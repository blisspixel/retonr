use std::{
    cell::Cell,
    path::{Path, PathBuf},
};

use rewrite_model::ArtifactSetId;

use super::*;
use crate::RuntimeAdmissionEvidenceFoundationInput;

pub(in crate::runtime_admission_evidence) struct FakeSource {
    pub(super) foundation: RuntimeAdmissionEvidenceFoundation,
    pub(in crate::runtime_admission_evidence) root: PathBuf,
    pub(in crate::runtime_admission_evidence) checks: Cell<usize>,
    pub(in crate::runtime_admission_evidence) fail_at: Cell<Option<usize>>,
    pub(in crate::runtime_admission_evidence) fail_all: Cell<bool>,
    pub(in crate::runtime_admission_evidence) cancel_at: Cell<Option<usize>>,
    pub(in crate::runtime_admission_evidence) cancellation: CancellationToken,
    pub(in crate::runtime_admission_evidence) inject_at: Cell<Option<usize>>,
    pub(in crate::runtime_admission_evidence) injection: Option<PathBuf>,
}

impl FakeSource {
    pub(in crate::runtime_admission_evidence) fn new(root: PathBuf) -> Self {
        let digest = Digest::sha256(b"source-bound assembly fixture");
        let foundation = RuntimeAdmissionEvidenceFoundation::compile(
            RuntimeAdmissionEvidenceFoundationInput::new(
                ArtifactSetId::from_digest(digest.clone()),
                ArtifactSetId::from_digest(digest.clone()),
                digest.clone(),
                digest.clone(),
                digest.clone(),
                serde_json::from_value(serde_json::json!(digest))
                    .expect("fixture runtime identity"),
            ),
        )
        .expect("fixture foundation");
        Self {
            foundation,
            root,
            checks: Cell::new(0),
            fail_at: Cell::new(None),
            fail_all: Cell::new(false),
            cancel_at: Cell::new(None),
            cancellation: CancellationToken::new(),
            inject_at: Cell::new(None),
            injection: None,
        }
    }
}

impl AssemblySourceEvidence for FakeSource {
    fn verify_foundation(
        &self,
        foundation: &RuntimeAdmissionEvidenceFoundation,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionEvidenceAssemblyError> {
        super::super::verify::active(cancellation)?;
        let index = self.checks.get() + 1;
        self.checks.set(index);
        if self.cancel_at.get() == Some(index) {
            self.cancellation.cancel();
        }
        if self.inject_at.get() == Some(index) {
            let path = self.injection.as_ref().expect("fixture injection path");
            std::fs::create_dir(path).expect("inject competing destination");
            std::fs::write(path.join("keep"), b"independent existing bytes")
                .expect("write competitor");
        }
        if self.fail_all.get()
            || self.fail_at.get() == Some(index)
            || foundation != &self.foundation
        {
            return Err(RuntimeAdmissionFoundationBindingError::InvalidBinding.into());
        }
        Ok(())
    }
    fn overlaps_path(&self, path: &Path) -> bool {
        path.starts_with(&self.root) || self.root.starts_with(path)
    }
}

pub(in crate::runtime_admission_evidence) fn members()
-> Vec<RuntimeAdmissionEvidenceAssemblyMember<'static>> {
    RuntimeAdmissionEvidenceMember::ALL
        .into_iter()
        .skip(1)
        .map(|member| {
            RuntimeAdmissionEvidenceAssemblyMember::new(
                member,
                b"opaque evidence awaiting semantic adjudication",
            )
        })
        .collect()
}

pub(in crate::runtime_admission_evidence) fn compilation(
    source: &FakeSource,
) -> CompiledRuntimeAdmissionEvidenceAssembly {
    let compiled = compile_view(
        &source.foundation,
        source,
        &members(),
        RuntimeAdmissionEvidenceBundleLimits::default(),
        &source.cancellation,
    )
    .expect("compile inert fixture");
    source.checks.set(0);
    compiled
}

#[test]
fn compiler_retains_exact_inert_bytes_and_redacts_debug() {
    let source = FakeSource::new(PathBuf::from("source-build-fixture"));
    let compiled = compilation(&source);
    assert_eq!(compiled.manifest().members().len(), 12);
    assert_eq!(compiled.tree_plan().members().len(), 12);
    assert_eq!(compiled.foundation(), &source.foundation);
    assert_eq!(
        compiled.bytes[RuntimeAdmissionEvidenceMember::NativeReview.relative_path()],
        b"opaque evidence awaiting semantic adjudication"
    );
    let debug = format!("{compiled:?}");
    assert!(!debug.contains("opaque evidence"));
    assert!(!debug.contains("source-build-fixture"));
}

#[test]
fn missing_extra_duplicate_reordered_and_foundation_members_fail_before_source_reads() {
    let source = FakeSource::new(PathBuf::from("source-build-fixture"));
    for mutation in 0..5 {
        let mut selected = members();
        match mutation {
            0 => {
                selected.pop();
            }
            1 => selected.push(selected[0]),
            2 => selected[1] = selected[0],
            3 => selected.swap(0, 1),
            _ => {
                selected[0] = RuntimeAdmissionEvidenceAssemblyMember::new(
                    RuntimeAdmissionEvidenceMember::Foundation,
                    b"supplied foundation",
                );
            }
        }
        assert!(matches!(
            compile_view(
                &source.foundation,
                &source,
                &selected,
                RuntimeAdmissionEvidenceBundleLimits::default(),
                &source.cancellation
            ),
            Err(RuntimeAdmissionEvidenceAssemblyError::InvalidMembers)
        ));
        assert_eq!(source.checks.get(), 0);
    }
}

#[test]
fn empty_oversized_and_aggregate_member_bounds_fail_before_source_reads() {
    let source = FakeSource::new(PathBuf::from("source-build-fixture"));
    let large = vec![0; 262_145];
    for bytes in [b"".as_slice(), large.as_slice()] {
        let mut selected = members();
        selected[0] = RuntimeAdmissionEvidenceAssemblyMember::new(
            RuntimeAdmissionEvidenceMember::RuntimePackageReviewV2,
            bytes,
        );
        assert!(
            compile_view(
                &source.foundation,
                &source,
                &selected,
                RuntimeAdmissionEvidenceBundleLimits::default(),
                &source.cancellation
            )
            .is_err()
        );
        assert_eq!(source.checks.get(), 0);
    }
    let limits = RuntimeAdmissionEvidenceBundleLimits {
        maximum_total_bytes: 1_048_576,
        ..RuntimeAdmissionEvidenceBundleLimits::default()
    };
    assert!(
        compile_view(
            &source.foundation,
            &source,
            &members(),
            limits,
            &source.cancellation
        )
        .is_err()
    );
    assert_eq!(source.checks.get(), 0);
}

#[test]
fn source_mismatch_drift_and_cancellation_prevent_compilation() {
    let source = FakeSource::new(PathBuf::from("source-build-fixture"));
    for check in [1, 2] {
        source.checks.set(0);
        source.fail_at.set(Some(check));
        assert!(matches!(
            compile_view(
                &source.foundation,
                &source,
                &members(),
                RuntimeAdmissionEvidenceBundleLimits::default(),
                &source.cancellation
            ),
            Err(RuntimeAdmissionEvidenceAssemblyError::Foundation(_))
        ));
    }
    source.fail_at.set(None);
    source.cancellation.cancel();
    assert!(matches!(
        compile_view(
            &source.foundation,
            &source,
            &members(),
            RuntimeAdmissionEvidenceBundleLimits::default(),
            &source.cancellation
        ),
        Err(RuntimeAdmissionEvidenceAssemblyError::Bundle(
            RuntimeAdmissionEvidenceBundleError::Cancelled
        ))
    ));
}

#[test]
fn diagnostics_do_not_expose_nested_private_source_messages() {
    let private = "C:/private/source-fixture raw evidence fixture";
    let error = RuntimeAdmissionEvidenceAssemblyError::from(
        RuntimeAdmissionEvidenceBundleError::StorageIo(std::io::Error::other(private)),
    );
    assert!(!format!("{error:?}").contains(private));
    let bundle = RuntimeAdmissionEvidenceBundleError::StorageIo(std::io::Error::other(private));
    assert!(!format!("{bundle:?}").contains(private));
    let error = RuntimeAdmissionEvidenceAssemblyError::CleanupAfterFailure {
        operation: Box::new(error),
        cleanup: Box::new(bundle),
    };
    assert!(!format!("{error:?}").contains(private));
}

#[test]
fn cancellation_at_both_source_checks_prevents_snapshot_release() {
    for checkpoint in [1, 2] {
        let source = FakeSource::new(PathBuf::from("source-build-fixture"));
        source.cancel_at.set(Some(checkpoint));
        let error = compile_view(
            &source.foundation,
            &source,
            &members(),
            RuntimeAdmissionEvidenceBundleLimits::default(),
            &source.cancellation,
        )
        .expect_err("cancellation before or after snapshotting prevents release");
        assert!(matches!(
            error,
            RuntimeAdmissionEvidenceAssemblyError::Bundle(
                RuntimeAdmissionEvidenceBundleError::Cancelled
            )
        ));
        assert_eq!(source.checks.get(), checkpoint);
    }
}

#[test]
fn compiled_snapshot_owns_bytes_after_caller_mutates_and_drops_input() {
    let source = FakeSource::new(PathBuf::from("source-build-fixture"));
    let mut caller_bytes = b"caller-owned opaque source evidence".to_vec();
    let compiled = {
        let selected = RuntimeAdmissionEvidenceMember::ALL
            .into_iter()
            .skip(1)
            .map(|member| RuntimeAdmissionEvidenceAssemblyMember::new(member, &caller_bytes))
            .collect::<Vec<_>>();
        compile_view(
            &source.foundation,
            &source,
            &selected,
            RuntimeAdmissionEvidenceBundleLimits::default(),
            &source.cancellation,
        )
        .expect("snapshot caller-owned bytes")
    };
    let expected_manifest = compiled.manifest().clone();
    caller_bytes.fill(b'x');
    drop(caller_bytes);
    for member in RuntimeAdmissionEvidenceMember::ALL.into_iter().skip(1) {
        assert_eq!(
            compiled.bytes[member.relative_path()],
            b"caller-owned opaque source evidence"
        );
    }
    assert_eq!(compiled.manifest(), &expected_manifest);
}

#[test]
fn substituted_foundation_subject_is_rejected_without_private_diagnostics() {
    let source = FakeSource::new(PathBuf::from("source-build-fixture"));
    let mut wire: serde_json::Value = serde_json::from_slice(source.foundation.canonical_bytes())
        .expect("fixture foundation JSON");
    wire["source_report_digest"] = serde_json::json!(Digest::sha256(b"substituted report"));
    let substitute = RuntimeAdmissionEvidenceFoundation::from_canonical_bytes(
        &serde_json::to_vec(&wire).expect("canonical substituted foundation"),
    )
    .expect("structurally valid substituted foundation");
    let error = compile_view(
        &substitute,
        &source,
        &members(),
        RuntimeAdmissionEvidenceBundleLimits::default(),
        &source.cancellation,
    )
    .expect_err("another source report cannot bind the retained source");
    assert!(matches!(
        error,
        RuntimeAdmissionEvidenceAssemblyError::Foundation(_)
    ));
    assert_eq!(source.checks.get(), 1);
    assert!(!format!("{error:?}").contains("source-build-fixture"));
    assert!(!format!("{error:?}").contains("substituted report"));
}
