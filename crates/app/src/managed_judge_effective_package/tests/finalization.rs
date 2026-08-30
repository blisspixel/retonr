use std::{error::Error as _, fs};

use rewrite_types::CancellationToken;

use crate::managed_judge_precursor::JudgeFixture;

use super::super::*;
use super::{authority_and_requests, managed_bracket};

#[test]
fn release_preserves_real_cleanup_model_and_runtime_failures() {
    let mut fixture = JudgeFixture::new();
    let model_drift = fixture.live.model_drift_path();
    let runtime_drift = fixture.live.runtime_drift_path();
    let managed = managed_bracket(
        &fixture.live.model_lease,
        &fixture.live.runtime.runtime_manifest,
        &fixture.live.runtime.runtime_package,
        &fixture.live.runtime.admitted,
        &fixture.live.runtime.prepared_isolation,
    );
    let (authority, requests) = authority_and_requests(&fixture, &managed);
    let runtime_generation = fixture
        .live
        .runtime
        .runtime_package
        .installation_key()
        .installation_generation();
    let model_generation = fixture
        .live
        .model_lease
        .private_view()
        .installation_generation();
    let plan = VerifiedManagedJudgeEffectivePackagePlan::prepare(
        ManagedJudgeEffectivePackagePlanInput {
            observation_authority: authority,
            request_aggregate: requests,
            judge_system: fixture.judge_system.clone(),
            expected_runtime_state: fixture.live.runtime.runtime_state.clone(),
            runtime_manifest: &fixture.live.runtime.runtime_manifest,
            runtime_package: &mut fixture.live.runtime.runtime_package,
            runtime_build: &fixture.live.runtime.runtime_build,
            admitted_runtime: &fixture.live.runtime.admitted,
            generation_path: &fixture.live.runtime.path,
            frozen_components: &fixture.live.runtime.frozen,
            prepared_isolation: &fixture.live.runtime.prepared_isolation,
            model_package: &fixture.live.model_lease,
            static_model: &fixture.static_model,
            characterized_package: &fixture.characterized,
            runtime_installation_generation: runtime_generation,
            model_installation_generation: model_generation,
            managed_ollama: managed,
        },
        &CancellationToken::new(),
    )
    .expect("pending batch package");
    fs::write(model_drift, b"model drift").expect("model drift");
    fs::write(runtime_drift, b"runtime drift").expect("runtime drift");
    let error = plan.release().expect_err("all package drift is retained");
    assert!(error.cleanup().is_some());
    assert!(error.model().is_some());
    assert!(error.runtime().is_some());
    assert!(error.evidence().is_none());
    assert!(error.source().is_some());
    assert_eq!(
        error.to_string(),
        "managed judge effective-package release failed"
    );
    let debug = format!("{error:?}");
    assert!(debug.contains("cleanup_failed: true"));
    assert!(!debug.contains("unexpected-member.bin"));
}

#[test]
fn redacted_error_variants_and_sources_are_complete() {
    let evidence_derivation = ManagedJudgeEffectivePackageDerivationError::Evidence(Box::new(
        crate::GenerationEffectivePackageDerivationError::Cancelled,
    ));
    assert!(format!("{evidence_derivation:?}").contains("evidence"));
    assert!(evidence_derivation.source().is_some());
    assert!(
        ManagedJudgeEffectivePackageDerivationError::Cancelled
            .source()
            .is_none()
    );

    let evidence = ManagedJudgeEffectivePackageFinalValidationError::EvidenceChanged;
    assert!(format!("{evidence:?}").contains("evidence_changed"));
    let release = ManagedJudgeEffectivePackageReleaseError {
        cleanup: None,
        model: None,
        runtime: None,
        evidence: Some(Box::new(evidence)),
    };
    assert!(release.cleanup().is_none());
    assert!(release.model().is_none());
    assert!(release.runtime().is_none());
    assert!(release.evidence().is_some());
    assert!(release.source().is_some());
    assert!(!format!("{release:?}").contains("evidence validation failed"));
}
