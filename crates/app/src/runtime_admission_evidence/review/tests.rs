use std::{cell::Cell, collections::BTreeMap, io::Cursor};

use rewrite_ollama_package::{
    MemberOpenError, RuntimeLayoutLimits, reconstruct_runtime_package_with_limits,
};
use rewrite_types::Digest;

use super::super::{RuntimeAdmissionEvidenceFoundation, RuntimeAdmissionEvidenceFoundationInput};
use super::*;

mod runtime;

struct Fixture {
    source: SyntheticSource,
    lineage: VerifiedPassedRuntimeAdmissionSourceLineageControl,
    transformation: VerifiedPassedRuntimeAdmissionTransformationControl,
    license: VerifiedPassedRuntimeAdmissionLicenseControl,
    operation: RuntimeAdmissionFinalOperation,
}

struct SyntheticSource {
    foundation: VerifiedRuntimeAdmissionFoundationBinding,
    package: RuntimePackageManifest,
    evidence: BTreeMap<String, Vec<u8>>,
    source: BTreeMap<String, Vec<u8>>,
    members: BTreeMap<String, Vec<u8>>,
    calls: Cell<usize>,
    fail_from: Cell<Option<usize>>,
    cancel_at: Cell<Option<usize>>,
    caller: CancellationToken,
}

impl ReviewSource for SyntheticSource {
    fn validate(
        &self,
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionAllPassReviewError> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if self.cancel_at.get() == Some(call) {
            self.caller.cancel();
        }
        if cancellation.is_cancelled() {
            return Err(RuntimeAdmissionAllPassReviewError::Cancelled);
        }
        if self.fail_from.get().is_some_and(|from| call >= from) {
            return Err(RuntimeAdmissionAllPassReviewError::Source(
                RuntimeAdmissionFoundationBindingError::InvalidBinding,
            ));
        }
        if foundation != &self.foundation {
            return Err(RuntimeAdmissionAllPassReviewError::InvalidBinding);
        }
        Ok(())
    }
    fn verify_static(
        &self,
        material: &input::ReviewMaterial<'_>,
        _cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionAllPassReviewError> {
        // Private synthetic authorities exercise composition. Actual semantic
        // controls are independently verified by the production source adapter.
        if material.source_lineage_bytes != b"lineage fixture control"
            || material.transformation_bytes != b"transformation fixture control"
            || material.license_bytes != b"license fixture control"
        {
            return Err(RuntimeAdmissionAllPassReviewError::Static(
                RuntimeAdmissionStaticControlError::InvalidBinding,
            ));
        }
        Ok(())
    }
    fn package(&self) -> &RuntimePackageManifest {
        &self.package
    }
    fn open_evidence(
        &self,
        path: &ArtifactSetRelativePath,
        _cancellation: &CancellationToken,
    ) -> Result<Box<dyn Read>, rewrite_ollama_package::RuntimePackageReviewEvidenceOpenError> {
        self.evidence
            .get(path.as_str())
            .cloned()
            .map(|bytes| Box::new(Cursor::new(bytes)) as Box<dyn Read>)
            .ok_or(rewrite_ollama_package::RuntimePackageReviewEvidenceOpenError)
    }
    fn open_source(
        &self,
        path: &ArtifactSetRelativePath,
        _cancellation: &CancellationToken,
    ) -> Result<Box<dyn Read>, rewrite_ollama_package::RuntimeSourceBuildInputOpenError> {
        self.source
            .get(path.as_str())
            .cloned()
            .map(|bytes| Box::new(Cursor::new(bytes)) as Box<dyn Read>)
            .ok_or(rewrite_ollama_package::RuntimeSourceBuildInputOpenError)
    }
    fn open_member(
        &self,
        path: &ArtifactSetRelativePath,
        _cancellation: &CancellationToken,
    ) -> Result<Box<dyn Read>, MemberOpenError> {
        self.members
            .get(path.as_str())
            .cloned()
            .map(|bytes| Box::new(Cursor::new(bytes)) as Box<dyn Read>)
            .ok_or(MemberOpenError)
    }
}

impl Fixture {
    fn new() -> Self {
        let (source_id, source_manifest, source) = runtime::source_fixture();
        let (layout, members, package_id) = runtime::runtime_fixture(&source_id);
        let package = reconstruct_runtime_package_with_limits(
            &layout,
            &RuntimeLayoutLimits::default(),
            |path| {
                members
                    .get(path.as_str())
                    .cloned()
                    .map(Cursor::new)
                    .ok_or(MemberOpenError)
            },
            || false,
        )
        .expect("fixture package")
        .runtime_package()
        .clone();
        let source_report = b"synthetic retained source report".to_vec();
        let foundation = RuntimeAdmissionEvidenceFoundation::compile(
            RuntimeAdmissionEvidenceFoundationInput::new(
                source_id.clone(),
                source_id,
                Digest::sha256(&source_manifest),
                Digest::sha256(b"plan"),
                Digest::sha256(&source_report),
                package_id,
            ),
        )
        .expect("foundation");
        let binding = VerifiedRuntimeAdmissionFoundationBinding::test_fixture(&foundation);
        let operation = RuntimeAdmissionFinalOperation::test_review_fixture(&binding, &package);
        let lineage = VerifiedPassedRuntimeAdmissionSourceLineageControl::test_fixture(
            binding.foundation_id().clone(),
        );
        let transformation = VerifiedPassedRuntimeAdmissionTransformationControl::test_fixture(
            binding.foundation_id().clone(),
        );
        let license = VerifiedPassedRuntimeAdmissionLicenseControl::test_fixture(
            binding.foundation_id().clone(),
        );
        Self {
            source: SyntheticSource {
                foundation: binding,
                package,
                evidence: BTreeMap::from([
                    (
                        crate::RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH.to_owned(),
                        source_manifest,
                    ),
                    (
                        crate::RUNTIME_SOURCE_BUILD_REPORT_PATH.to_owned(),
                        source_report,
                    ),
                    ("attempts/primary/runtime-layout.json".to_owned(), layout),
                    (
                        "attempts/primary/provenance.json".to_owned(),
                        b"synthetic retained provenance".to_vec(),
                    ),
                ]),
                source,
                members,
                calls: Cell::new(0),
                fail_from: Cell::new(None),
                cancel_at: Cell::new(None),
                caller: CancellationToken::new(),
            },
            lineage,
            transformation,
            license,
            operation,
        }
    }
    fn material(&self) -> input::ReviewMaterial<'_> {
        input::ReviewMaterial {
            foundation: &self.source.foundation,
            source_lineage_bytes: b"lineage fixture control",
            source_lineage: &self.lineage,
            transformation_bytes: b"transformation fixture control",
            transformation: &self.transformation,
            license_bytes: b"license fixture control",
            license: &self.license,
            execution: &self.operation,
        }
    }
}

#[test]
fn six_private_passed_authorities_compile_an_inert_canonical_round_trip() {
    let fixture = Fixture::new();
    let compiled = compile_view(
        &fixture.material(),
        &fixture.source,
        RuntimePackageReviewV2Limits::default(),
        &fixture.source.caller,
    )
    .expect("all-pass inert material");
    assert_eq!(fixture.source.calls.get(), 3);
    assert_eq!(
        compiled
            .verified_review()
            .reconstructed_runtime()
            .expect("reconstructed")
            .runtime_package(),
        &fixture.source.package
    );
    let value: serde_json::Value =
        serde_json::from_slice(compiled.canonical_bytes()).expect("canonical review");
    assert_eq!(
        serde_json::to_vec(&value).expect("canonical encoding"),
        compiled.canonical_bytes()
    );
    assert_eq!(value["checks"].as_array().expect("checks").len(), 6);
    assert!(
        value["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .all(|check| check["status"] == "passed")
    );
}

#[test]
fn substituted_static_bytes_and_foreign_foundations_never_compile() {
    let fixture = Fixture::new();
    let mut material = fixture.material();
    material.license_bytes = b"substituted license material";
    assert!(matches!(
        compile_view(
            &material,
            &fixture.source,
            RuntimePackageReviewV2Limits::default(),
            &fixture.source.caller
        ),
        Err(RuntimeAdmissionAllPassReviewError::Static(_))
    ));
    assert_eq!(fixture.source.calls.get(), 2);
    let other = Fixture::new();
    let foreign = VerifiedPassedRuntimeAdmissionLicenseControl::test_fixture(
        RuntimeAdmissionEvidenceFoundation::compile(RuntimeAdmissionEvidenceFoundationInput::new(
            other.source.foundation.source_build_inputs_id().clone(),
            other.source.foundation.source_build_inputs_id().clone(),
            Digest::sha256(b"foreign manifest"),
            Digest::sha256(b"foreign plan"),
            Digest::sha256(b"foreign report"),
            other.source.package.runtime_package_manifest_id(),
        ))
        .expect("foreign foundation")
        .foundation_id()
        .clone(),
    );
    let mut material = other.material();
    material.license = &foreign;
    assert!(matches!(
        compile_view(
            &material,
            &other.source,
            RuntimePackageReviewV2Limits::default(),
            &other.source.caller
        ),
        Err(RuntimeAdmissionAllPassReviewError::InvalidBinding)
    ));
}

#[test]
fn cancellation_always_runs_fresh_terminal_validation_and_blocks_release() {
    for cancel_at in [0, 1, 2, 3] {
        let fixture = Fixture::new();
        if cancel_at == 0 {
            fixture.source.caller.cancel();
        } else {
            fixture.source.cancel_at.set(Some(cancel_at));
        }
        assert!(matches!(
            compile_view(
                &fixture.material(),
                &fixture.source,
                RuntimePackageReviewV2Limits::default(),
                &fixture.source.caller
            ),
            Err(RuntimeAdmissionAllPassReviewError::Cancelled
                | RuntimeAdmissionAllPassReviewError::Review(_))
        ));
        assert!(fixture.source.calls.get() >= 1);
    }
}

#[test]
fn pre_cancelled_compilation_retains_independent_terminal_failure() {
    let fixture = Fixture::new();
    fixture.source.caller.cancel();
    fixture.source.fail_from.set(Some(1));
    let error = compile_view(
        &fixture.material(),
        &fixture.source,
        RuntimePackageReviewV2Limits::default(),
        &fixture.source.caller,
    )
    .err()
    .expect("cancellation and finalization remain distinct");
    assert!(
        matches!(error, RuntimeAdmissionAllPassReviewError::FinalizationAfterFailure {
        primary, finalization,
    } if matches!(*primary, RuntimeAdmissionAllPassReviewError::Cancelled)
        && matches!(*finalization, RuntimeAdmissionAllPassReviewError::Source(_)))
    );
    assert_eq!(fixture.source.calls.get(), 1);
}

#[test]
fn independent_terminal_source_drift_is_retained_alongside_primary_failure() {
    let fixture = Fixture::new();
    fixture.source.fail_from.set(Some(2));
    let mut material = fixture.material();
    material.license_bytes = b"substituted";
    let error = compile_view(
        &material,
        &fixture.source,
        RuntimePackageReviewV2Limits::default(),
        &fixture.source.caller,
    )
    .err()
    .expect("both causes");
    assert!(
        matches!(error, RuntimeAdmissionAllPassReviewError::FinalizationAfterFailure { primary, finalization }
        if matches!(*primary, RuntimeAdmissionAllPassReviewError::Static(_)) && matches!(*finalization, RuntimeAdmissionAllPassReviewError::Source(_)))
    );
    assert_eq!(fixture.source.calls.get(), 2);
}

#[test]
fn successful_primary_cannot_suppress_terminal_source_drift() {
    let fixture = Fixture::new();
    fixture.source.fail_from.set(Some(3));
    assert!(matches!(
        compile_view(
            &fixture.material(),
            &fixture.source,
            RuntimePackageReviewV2Limits::default(),
            &fixture.source.caller
        ),
        Err(RuntimeAdmissionAllPassReviewError::Source(_))
    ));
    assert_eq!(fixture.source.calls.get(), 3);
}

#[test]
fn changed_source_and_runtime_member_bytes_fail_independent_reconstruction() {
    for change_source in [true, false] {
        let mut fixture = Fixture::new();
        if change_source {
            *fixture
                .source
                .source
                .values_mut()
                .next()
                .expect("source member") = b"changed".to_vec();
        } else {
            *fixture
                .source
                .members
                .values_mut()
                .next()
                .expect("runtime member") = b"changed".to_vec();
        }
        assert!(matches!(
            compile_view(
                &fixture.material(),
                &fixture.source,
                RuntimePackageReviewV2Limits::default(),
                &fixture.source.caller
            ),
            Err(RuntimeAdmissionAllPassReviewError::Review(_))
        ));
        assert_eq!(fixture.source.calls.get(), 2);
    }
}

#[test]
fn invalid_limits_and_empty_control_material_release_no_review() {
    let fixture = Fixture::new();
    let limits = RuntimePackageReviewV2Limits {
        review_bytes: 0,
        ..RuntimePackageReviewV2Limits::default()
    };
    assert!(matches!(
        compile_view(
            &fixture.material(),
            &fixture.source,
            limits,
            &fixture.source.caller
        ),
        Err(RuntimeAdmissionAllPassReviewError::Review(_))
    ));
    let mut material = fixture.material();
    material.source_lineage_bytes = b"";
    assert!(matches!(
        compile_view(
            &material,
            &fixture.source,
            RuntimePackageReviewV2Limits::default(),
            &fixture.source.caller
        ),
        Err(RuntimeAdmissionAllPassReviewError::InvalidBinding)
    ));
}
