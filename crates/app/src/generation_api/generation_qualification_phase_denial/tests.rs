use super::*;

#[cfg(feature = "test-support")]
mod integration;
#[cfg(feature = "test-support")]
pub(crate) mod support;

#[test]
fn public_release_surface_is_minimal_inert_and_clock_free() {
    fn public_type<T>() {}
    public_type::<crate::GenerationQualificationResourcePolicyDeniedCompiler>();
    public_type::<crate::GenerationQualificationHumanAdjudicationPolicyDeniedCompiler>();
    public_type::<crate::VerifiedGenerationQualificationResourcePolicyDenied<'static>>();
    public_type::<crate::VerifiedGenerationQualificationHumanAdjudicationPolicyDenied<'static>>();

    let source = include_str!("../generation_qualification_phase_denial.rs");
    for forbidden in [
        "pub fn execute(",
        "pub fn launch(",
        "pub fn send(",
        "pub fn persist(",
        "pub fn approve(",
        "pub fn qualify(",
        "pub fn traffic(",
    ] {
        assert!(
            !source.contains(forbidden),
            "unexpected public authority path"
        );
    }
    assert!(!source.contains("CancellationToken"));
    assert!(!source.contains("std::time"));
    assert!(source.contains("outer\n//! evaluation state-machine bracket owns cancellation"));
}

#[test]
fn errors_are_content_redacted() {
    let secret = "e4479b4d9801d36ff8740a61376750c9f04bcedfeaa5e65b71b5de65dd301d2f";
    let errors = [
        GenerationQualificationPhaseDenialError::PolicyNotDenied,
        GenerationQualificationPhaseDenialError::Policy(
            GenerationQualificationPhasePolicyError::OperationPolicyMismatch,
        ),
        GenerationQualificationPhaseDenialError::ScopeMismatch,
        GenerationQualificationPhaseDenialError::Evidence(
            GenerationQualificationPhaseEvidenceError::RelationshipMismatch,
        ),
    ];
    for error in errors {
        assert!(!format!("{error:?}").contains(secret));
        assert!(!error.to_string().contains(secret));
    }
}
