use std::{collections::BTreeSet, fs::File, io::Seek as _};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    MAX_ARTIFACT_SET_MEMBERS,
};
use rewrite_ollama_package::{ReconstructedRuntimePackage, RuntimeSourceBuildAttempt};
use rewrite_types::{CancellationToken, Digest};

use super::contract::{
    RuntimeSourceBuildEvidenceBundleError, RuntimeSourceBuildEvidenceBundleLimits, ensure_active,
};
use crate::{
    ExecutableRuntimeSourceBuildBundleLease, RuntimeSourceBuildExecution,
    RuntimeSourceBuildManagedEvidence, RuntimeSourceBuildReportCompilation,
};

const INPUT_PREFIX: &str = "inputs/";
const BUILD_EVIDENCE_FILES: [&str; 4] = [
    "runtime-layout.json",
    "sbom.json",
    "provenance.json",
    "transformation.json",
];

pub(super) enum PlannedSource {
    Memory(Vec<u8>),
    Retained {
        file: File,
        byte_size: u64,
        expected_digest: Option<Digest>,
    },
}

pub(super) struct PlannedFile {
    pub(super) path: ArtifactSetRelativePath,
    pub(super) source: PlannedSource,
}

impl PlannedFile {
    pub(super) fn byte_size(&self) -> Result<u64, RuntimeSourceBuildEvidenceBundleError> {
        match &self.source {
            PlannedSource::Memory(bytes) => u64::try_from(bytes.len())
                .map_err(|_| RuntimeSourceBuildEvidenceBundleError::LimitExceeded),
            PlannedSource::Retained { byte_size, .. } => Ok(*byte_size),
        }
    }
}

pub(super) fn plan_files(
    bundle: &ExecutableRuntimeSourceBuildBundleLease,
    primary: &RuntimeSourceBuildExecution,
    rebuild: &RuntimeSourceBuildExecution,
    compilation: &RuntimeSourceBuildReportCompilation,
    cancellation: &CancellationToken,
) -> Result<Vec<PlannedFile>, RuntimeSourceBuildEvidenceBundleError> {
    let mut files = vec![
        memory_plan(
            super::RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH,
            bundle.bundle().manifest_bytes().to_vec(),
        )?,
        memory_plan(
            super::RUNTIME_SOURCE_BUILD_REPORT_PATH,
            compilation.canonical_report_bytes().to_vec(),
        )?,
    ];
    if let Some(binding) = super::plan_binding::canonical_bytes(bundle)? {
        files.push(memory_plan(
            super::RUNTIME_SOURCE_BUILD_PLAN_BINDING_PATH,
            binding,
        )?);
    }
    for component in bundle.bundle().inputs().manifest().components() {
        ensure_active(cancellation)?;
        files.push(retained_plan(
            &format!("{INPUT_PREFIX}{}", component.relative_path().as_str()),
            bundle
                .bundle()
                .clone_component_for_evidence(component.relative_path())?,
            component.byte_size(),
            Some(component.digest().clone()),
        )?);
    }
    plan_attempt(
        &mut files,
        RuntimeSourceBuildAttempt::Primary,
        primary,
        compilation.compiled().primary(),
        compilation.managed_evidence(RuntimeSourceBuildAttempt::Primary),
        cancellation,
    )?;
    plan_attempt(
        &mut files,
        RuntimeSourceBuildAttempt::Rebuild,
        rebuild,
        compilation.compiled().rebuild(),
        compilation.managed_evidence(RuntimeSourceBuildAttempt::Rebuild),
        cancellation,
    )?;
    Ok(files)
}

fn plan_attempt(
    files: &mut Vec<PlannedFile>,
    attempt: RuntimeSourceBuildAttempt,
    execution: &RuntimeSourceBuildExecution,
    runtime: &ReconstructedRuntimePackage,
    managed: &RuntimeSourceBuildManagedEvidence,
    cancellation: &CancellationToken,
) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
    let prefix = attempt_prefix(attempt);
    for name in BUILD_EVIDENCE_FILES {
        ensure_active(cancellation)?;
        let relative = ArtifactSetRelativePath::new(name.to_owned())
            .expect("fixed build evidence path is portable");
        let (opened, byte_size, digest) = execution
            .output
            .open_committed_regular_file(&relative, cancellation)?;
        files.push(retained_plan(
            &format!("{prefix}{name}"),
            opened,
            byte_size,
            Some(digest),
        )?);
    }
    for member in runtime.artifact_set().members() {
        ensure_active(cancellation)?;
        files.push(retained_plan(
            &format!("{prefix}{}", member.relative_path().as_str()),
            execution
                .output
                .open_regular_file(member.relative_path(), cancellation)?,
            member.byte_size(),
            Some(member.artifact_id().digest().clone()),
        )?);
    }
    files.push(memory_plan(
        &format!("{prefix}output-tree.json"),
        managed.output_tree_bytes().to_vec(),
    )?);
    files.push(memory_plan(
        &format!("{prefix}execution-receipt.json"),
        managed.execution_receipt_bytes().to_vec(),
    )?);
    files.push(memory_plan(
        &format!("{prefix}stdout.bin"),
        managed.standard_output().to_vec(),
    )?);
    files.push(memory_plan(
        &format!("{prefix}stderr.bin"),
        managed.standard_error().to_vec(),
    )?);
    Ok(())
}

fn memory_plan(
    path: &str,
    bytes: Vec<u8>,
) -> Result<PlannedFile, RuntimeSourceBuildEvidenceBundleError> {
    Ok(PlannedFile {
        path: ArtifactSetRelativePath::new(path.to_owned())
            .map_err(|_| RuntimeSourceBuildEvidenceBundleError::TreeMismatch)?,
        source: PlannedSource::Memory(bytes),
    })
}

fn retained_plan(
    path: &str,
    mut file: File,
    byte_size: u64,
    expected_digest: Option<Digest>,
) -> Result<PlannedFile, RuntimeSourceBuildEvidenceBundleError> {
    file.seek(std::io::SeekFrom::Start(0))
        .map_err(RuntimeSourceBuildEvidenceBundleError::StorageIo)?;
    Ok(PlannedFile {
        path: ArtifactSetRelativePath::new(path.to_owned())
            .map_err(|_| RuntimeSourceBuildEvidenceBundleError::TreeMismatch)?,
        source: PlannedSource::Retained {
            file,
            byte_size,
            expected_digest,
        },
    })
}

pub(super) fn validate_plan(
    files: &[PlannedFile],
    limits: RuntimeSourceBuildEvidenceBundleLimits,
) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
    if files.len() > MAX_ARTIFACT_SET_MEMBERS {
        return Err(RuntimeSourceBuildEvidenceBundleError::LimitExceeded);
    }
    let mut total_bytes = 0_u64;
    let mut directories = BTreeSet::new();
    let mut placeholders = Vec::with_capacity(files.len());
    for file in files {
        let byte_size = file.byte_size()?;
        total_bytes = total_bytes
            .checked_add(byte_size)
            .ok_or(RuntimeSourceBuildEvidenceBundleError::LimitExceeded)?;
        if total_bytes > limits.maximum_total_bytes {
            return Err(RuntimeSourceBuildEvidenceBundleError::LimitExceeded);
        }
        collect_directories(&file.path, &mut directories)?;
        placeholders.push(ArtifactSetMember::new(
            ArtifactId::from_digest(Digest::sha256(&[])),
            byte_size,
            file.path.clone(),
        ));
    }
    placeholders.sort_unstable_by(|left, right| {
        left.relative_path()
            .as_str()
            .as_bytes()
            .cmp(right.relative_path().as_str().as_bytes())
    });
    ArtifactSetManifest::new(placeholders)
        .map_err(RuntimeSourceBuildEvidenceBundleError::Manifest)?;
    let tree_entries = files
        .len()
        .checked_add(directories.len())
        .and_then(|entries| entries.checked_add(1))
        .ok_or(RuntimeSourceBuildEvidenceBundleError::LimitExceeded)?;
    if tree_entries > limits.maximum_tree_entries {
        Err(RuntimeSourceBuildEvidenceBundleError::LimitExceeded)
    } else {
        Ok(())
    }
}

fn collect_directories(
    path: &ArtifactSetRelativePath,
    directories: &mut BTreeSet<ArtifactSetRelativePath>,
) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
    let mut prefix = String::new();
    let components = path.as_str().split('/').collect::<Vec<_>>();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        directories.insert(
            ArtifactSetRelativePath::new(prefix.clone())
                .map_err(|_| RuntimeSourceBuildEvidenceBundleError::TreeMismatch)?,
        );
    }
    Ok(())
}

pub(super) const fn attempt_prefix(attempt: RuntimeSourceBuildAttempt) -> &'static str {
    match attempt {
        RuntimeSourceBuildAttempt::Primary => "attempts/primary/",
        RuntimeSourceBuildAttempt::Rebuild => "attempts/rebuild/",
    }
}
