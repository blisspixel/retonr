use rewrite_model::{
    GenerationResourceAttemptResultRecordV1Input, GenerationResourceExceededLimitV1,
};

use super::{
    ResourceCompilationKernelChecks, ResourceObservationView, reviewed_generate_ordinal,
    validate_resource_mode,
};
use crate::{VerifiedCandidateBatchError, VerifiedCandidateBatchRelationship};

struct KernelView {
    input: GenerationResourceAttemptResultRecordV1Input,
}

impl ResourceObservationView for KernelView {
    fn revalidates(&self) -> bool {
        true
    }

    fn exact_bindings_match(
        &self,
        _input: &crate::VerifiedCandidateBatchResourceInput<'_>,
        _attempt: &crate::VerifiedCompletedManagedCandidateAttempt,
        _receipt: &rewrite_model::CandidateGenerationReceiptV1,
    ) -> bool {
        true
    }

    fn record_input(&self) -> GenerationResourceAttemptResultRecordV1Input {
        self.input.clone()
    }
}

fn passing_checks() -> ResourceCompilationKernelChecks {
    ResourceCompilationKernelChecks {
        source_approved: true,
        operation_policy_matches: true,
        observation_revalidates_before: true,
        exact_bindings_match: true,
    }
}

#[test]
fn private_kernel_accepts_exact_positive_observations() {
    passing_checks().validate().expect("all exact checks pass");
    let input = GenerationResourceAttemptResultRecordV1Input {
        prompt_token_count: 10,
        generated_token_count: 20,
        total_duration_nanoseconds: 30_000,
        load_duration_nanoseconds: 1_000,
        prompt_evaluation_duration_nanoseconds: 2_000,
        evaluation_duration_nanoseconds: 20_000,
        attempt_elapsed_nanoseconds: 50_000,
        first_response_elapsed_nanoseconds: 25_000,
        cleanup_elapsed_nanoseconds: 4_000,
        worker_high_water_resident_bytes: 8_192,
        runtime_installed_payload_bytes: 100,
        model_installed_payload_bytes: 200,
        installed_footprint_bytes: 300,
        exceeded_limits: vec![GenerationResourceExceededLimitV1::FirstResponse],
    };
    assert_eq!(
        KernelView {
            input: input.clone()
        }
        .record_input(),
        input
    );
}

#[test]
fn private_kernel_rejects_each_omission_or_substitution() {
    for change in [
        |checks: &mut ResourceCompilationKernelChecks| checks.source_approved = false,
        |checks: &mut ResourceCompilationKernelChecks| {
            checks.operation_policy_matches = false;
        },
        |checks: &mut ResourceCompilationKernelChecks| {
            checks.observation_revalidates_before = false;
        },
        |checks: &mut ResourceCompilationKernelChecks| checks.exact_bindings_match = false,
    ] {
        let mut checks = passing_checks();
        change(&mut checks);
        assert!(matches!(
            checks.validate(),
            Err(VerifiedCandidateBatchError::Relationship(
                VerifiedCandidateBatchRelationship::ResourceObservation
            ))
        ));
    }
}

#[test]
fn reviewed_generate_ordinal_is_exact_and_checked() {
    assert_eq!(reviewed_generate_ordinal(7), Some(10));
    assert_eq!(reviewed_generate_ordinal(usize::MAX), None);
}

#[test]
fn compatibility_and_resource_compilation_modes_fail_closed() {
    assert!(validate_resource_mode(false, false).is_ok());
    assert!(validate_resource_mode(true, true).is_ok());
    assert!(matches!(
        validate_resource_mode(true, false),
        Err(VerifiedCandidateBatchError::Relationship(
            VerifiedCandidateBatchRelationship::UncompiledResourceObservation
        ))
    ));
    assert!(matches!(
        validate_resource_mode(false, true),
        Err(VerifiedCandidateBatchError::Relationship(
            VerifiedCandidateBatchRelationship::MissingResourceObservation
        ))
    ));
}
