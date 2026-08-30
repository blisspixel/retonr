use std::collections::BTreeMap;

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_ollama_package::{
    RuntimeSourceBuildAttempt, VerifiedRuntimeSourceBuildInputs, VerifiedRuntimeSourceBuildReport,
};
use rewrite_types::Digest;

use super::{INPUT_PREFIX, attempt_prefix};
use crate::{
    RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_MANIFEST_PATH, RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH,
    RUNTIME_SOURCE_BUILD_PLAN_BINDING_PATH, RUNTIME_SOURCE_BUILD_REPORT_PATH,
    RuntimeSourceBuildEvidenceBundleError,
    artifact_storage::{ManagedTreeEntryKind, ManagedTreeSnapshot},
};

pub(super) fn expected_manifest(
    input_bytes: &[u8],
    plan_binding_bytes: Option<&[u8]>,
    report_bytes: &[u8],
    source_inputs: &VerifiedRuntimeSourceBuildInputs,
    report: &VerifiedRuntimeSourceBuildReport,
) -> Result<ArtifactSetManifest, RuntimeSourceBuildEvidenceBundleError> {
    let mut members = vec![
        memory_member(RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH, input_bytes)?,
        memory_member(RUNTIME_SOURCE_BUILD_REPORT_PATH, report_bytes)?,
    ];
    if let Some(bytes) = plan_binding_bytes {
        members.push(memory_member(
            RUNTIME_SOURCE_BUILD_PLAN_BINDING_PATH,
            bytes,
        )?);
    }
    for component in source_inputs.manifest().components() {
        members.push(declared_member(
            &format!("{INPUT_PREFIX}{}", component.relative_path().as_str()),
            component.byte_size(),
            component.digest().clone(),
        )?);
    }
    for (attempt, runtime) in [
        (RuntimeSourceBuildAttempt::Primary, report.primary()),
        (RuntimeSourceBuildAttempt::Rebuild, report.rebuild()),
    ] {
        for evidence in report.report().evidence(attempt) {
            members.push(declared_member(
                evidence.relative_path().as_str(),
                evidence.byte_size(),
                evidence.digest().clone(),
            )?);
        }
        for member in runtime.artifact_set().members() {
            members.push(declared_member(
                &format!(
                    "{}{}",
                    attempt_prefix(attempt),
                    member.relative_path().as_str()
                ),
                member.byte_size(),
                member.artifact_id().digest().clone(),
            )?);
        }
    }
    members.sort_unstable_by(|left, right| {
        left.relative_path()
            .as_str()
            .as_bytes()
            .cmp(right.relative_path().as_str().as_bytes())
    });
    ArtifactSetManifest::new(members).map_err(RuntimeSourceBuildEvidenceBundleError::Manifest)
}

pub(super) fn validate_tree(
    snapshot: &ManagedTreeSnapshot,
    manifest: &ArtifactSetManifest,
    manifest_bytes: usize,
) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
    let mut expected = BTreeMap::<String, (String, ManagedTreeEntryKind, u64)>::new();
    for member in manifest.members() {
        insert_expected(
            &mut expected,
            member.relative_path().as_str(),
            ManagedTreeEntryKind::RegularFile,
            member.byte_size(),
        )?;
    }
    insert_expected(
        &mut expected,
        RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_MANIFEST_PATH,
        ManagedTreeEntryKind::RegularFile,
        u64::try_from(manifest_bytes)
            .map_err(|_| RuntimeSourceBuildEvidenceBundleError::LimitExceeded)?,
    )?;
    if snapshot.entries().len() != expected.len() {
        return Err(RuntimeSourceBuildEvidenceBundleError::TreeMismatch);
    }
    for entry in snapshot.entries() {
        let key = entry.relative_path().as_str().to_ascii_lowercase();
        let matches = expected.get(&key).is_some_and(|(path, kind, byte_size)| {
            path == entry.relative_path().as_str()
                && *kind == entry.kind()
                && *byte_size == entry.byte_size()
                && (entry.kind() != ManagedTreeEntryKind::RegularFile || entry.has_single_link())
        });
        if !matches {
            return Err(RuntimeSourceBuildEvidenceBundleError::TreeMismatch);
        }
    }
    Ok(())
}

fn insert_expected(
    expected: &mut BTreeMap<String, (String, ManagedTreeEntryKind, u64)>,
    path: &str,
    kind: ManagedTreeEntryKind,
    byte_size: u64,
) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
    let components = path.split('/').collect::<Vec<_>>();
    let mut prefix = String::new();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        insert_one(expected, &prefix, ManagedTreeEntryKind::Directory, 0)?;
    }
    insert_one(expected, path, kind, byte_size)
}

fn insert_one(
    expected: &mut BTreeMap<String, (String, ManagedTreeEntryKind, u64)>,
    path: &str,
    kind: ManagedTreeEntryKind,
    byte_size: u64,
) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
    let key = path.to_ascii_lowercase();
    match expected.get(&key) {
        Some((prior_path, prior_kind, prior_size))
            if prior_path == path && *prior_kind == kind && *prior_size == byte_size =>
        {
            Ok(())
        }
        Some(_) => Err(RuntimeSourceBuildEvidenceBundleError::TreeMismatch),
        None => {
            expected.insert(key, (path.to_owned(), kind, byte_size));
            Ok(())
        }
    }
}

fn memory_member(
    path: &str,
    bytes: &[u8],
) -> Result<ArtifactSetMember, RuntimeSourceBuildEvidenceBundleError> {
    declared_member(
        path,
        u64::try_from(bytes.len())
            .map_err(|_| RuntimeSourceBuildEvidenceBundleError::LimitExceeded)?,
        Digest::sha256(bytes),
    )
}

fn declared_member(
    path: &str,
    byte_size: u64,
    digest: Digest,
) -> Result<ArtifactSetMember, RuntimeSourceBuildEvidenceBundleError> {
    Ok(ArtifactSetMember::new(
        ArtifactId::from_digest(digest),
        byte_size,
        ArtifactSetRelativePath::new(path.to_owned())
            .map_err(|_| RuntimeSourceBuildEvidenceBundleError::TreeMismatch)?,
    ))
}
