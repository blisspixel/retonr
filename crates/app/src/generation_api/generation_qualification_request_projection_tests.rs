use std::time::Instant;

use rewrite_model::GenerationQualificationRequestProjectionEntryV1;
use rewrite_types::{CancellationToken, Digest};

use super::*;
use crate::generation_api::generation_qualification_request_builder::tests::with_projection_compiler_input;

#[test]
fn compiles_exact_complete_projection_and_revalidates() {
    with_projection_compiler_input(|input, _database| {
        let expected_attempt_ids = input
            .operation_policy_relations
            .planned_attempts
            .iter()
            .map(|attempt| attempt.planned_attempt_id().clone())
            .collect::<Vec<_>>();
        let mut authority = VerifiedGenerationQualificationRequestProjectionV1::compile(
            input,
            &CancellationToken::new(),
        )
        .expect("verified projection");
        assert!(authority.started() < authority.deadline());
        assert_eq!(authority.projection().entry_count(), 2);
        assert_eq!(authority.entry_inputs().len(), 2);
        assert_eq!(
            authority.operation_policy().operation_policy_id(),
            authority.projection().operation_policy_id()
        );
        assert_eq!(
            authority.operation_policy_input().limits,
            authority.operation_policy().limits()
        );
        assert_eq!(
            authority
                .operation_policy_relations()
                .plan
                .qualification_plan_id(),
            authority.projection().generation_qualification_plan_id()
        );
        assert!(
            authority
                .projection()
                .entries()
                .iter()
                .map(GenerationQualificationRequestProjectionEntryV1::planned_attempt_id)
                .eq(expected_attempt_ids.iter())
        );
        authority
            .revalidate(&CancellationToken::new())
            .expect("fresh reconstruction");
        let debug = format!("{authority:?}");
        assert!(!debug.contains("Retain Acme"));
        assert!(!debug.contains("PROTECTED"));
    });
}

#[test]
fn deadline_token_must_bind_the_exact_operation_limits() {
    with_projection_compiler_input(|mut input, _database| {
        let limits = input.operation_policy.limits();
        let shorter = GenerationQualificationOperationLimitsV1::new(
            limits.maximum_source_bytes(),
            limits.maximum_complete_input_bytes(),
            limits.maximum_context_tokens(),
            limits.maximum_output_tokens(),
            limits.maximum_output_bytes(),
            limits.maximum_candidates_per_completion(),
            limits.maximum_candidate_bytes(),
            limits.maximum_aggregate_candidate_bytes(),
            limits.maximum_predeclared_attempts(),
            limits.maximum_concurrent_attempts(),
            limits.maximum_elapsed_milliseconds() - 1,
        )
        .expect("shorter deadline limits");
        input.operation_deadline =
            GenerationQualificationOperationDeadlineV1::start(shorter, &CancellationToken::new())
                .expect("alternate deadline token");
        assert!(matches!(
            VerifiedGenerationQualificationRequestProjectionV1::compile(
                input,
                &CancellationToken::new(),
            ),
            Err(GenerationQualificationRequestProjectionError::OperationMismatch)
        ));
    });
}

#[test]
fn target_and_baseline_builder_substitution_fails_before_source_use() {
    with_projection_compiler_input(|mut input, _database| {
        let [case] = input.case_authorities.as_mut_slice() else {
            panic!("one fixture case");
        };
        std::mem::swap(&mut case.target_builder, &mut case.baseline_builder);
        assert!(matches!(
            VerifiedGenerationQualificationRequestProjectionV1::compile(
                input,
                &CancellationToken::new(),
            ),
            Err(GenerationQualificationRequestProjectionError::CaseAuthorityMismatch)
        ));
    });
}

#[test]
fn foreign_operation_input_fails_closed() {
    with_projection_compiler_input(|input, _database| {
        let mut foreign = input.operation_policy_input.clone();
        foreign.resource_policy_digest = Digest::sha256(b"foreign resource policy");
        let substituted = GenerationQualificationRequestProjectionCompilerV1Input {
            operation_deadline: input.operation_deadline,
            operation_policy: input.operation_policy,
            operation_policy_relations: input.operation_policy_relations,
            operation_policy_input: foreign,
            case_authorities: input.case_authorities,
        };
        assert!(matches!(
            VerifiedGenerationQualificationRequestProjectionV1::compile(
                substituted,
                &CancellationToken::new(),
            ),
            Err(GenerationQualificationRequestProjectionError::OperationMismatch)
        ));
    });
}

#[test]
fn cancellation_precedes_deadline_and_source_drift() {
    with_projection_compiler_input(|input, _database| {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            VerifiedGenerationQualificationRequestProjectionV1::compile(input, &cancellation),
            Err(GenerationQualificationRequestProjectionError::Cancelled)
        ));

        with_projection_compiler_input(|input, database| {
            let mut authority = VerifiedGenerationQualificationRequestProjectionV1::compile(
                input,
                &CancellationToken::new(),
            )
            .expect("verified projection");
            let connection = rusqlite::Connection::open(database).expect("state database");
            connection
                .execute(
                    "UPDATE installed_artifacts SET installation_epoch = installation_epoch + 1",
                    [],
                )
                .expect("advance installation epoch");
            assert_eq!(
                authority.revalidate(&CancellationToken::new()),
                Err(GenerationQualificationRequestProjectionError::SourceMismatch)
            );

            authority.operation_deadline.deadline = Instant::now();
            assert_eq!(
                authority.revalidate(&CancellationToken::new()),
                Err(GenerationQualificationRequestProjectionError::DeadlineExceeded)
            );
            let cancelled = CancellationToken::new();
            cancelled.cancel();
            assert_eq!(
                authority.revalidate(&cancelled),
                Err(GenerationQualificationRequestProjectionError::Cancelled)
            );
        });
    });
}

#[test]
fn mandatory_finalization_ignores_the_original_deadline_and_caller_cancellation() {
    with_projection_compiler_input(|input, _database| {
        let mut authority = VerifiedGenerationQualificationRequestProjectionV1::compile(
            input,
            &CancellationToken::new(),
        )
        .expect("verified projection");
        authority.operation_deadline.deadline = Instant::now();
        assert_eq!(
            authority.revalidate(&CancellationToken::new()),
            Err(GenerationQualificationRequestProjectionError::DeadlineExceeded)
        );
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert_eq!(
            authority.revalidate(&cancelled),
            Err(GenerationQualificationRequestProjectionError::Cancelled)
        );
        authority
            .revalidate_for_mandatory_finalization()
            .expect("fresh finalization reconstruction ignores caller operation state");
    });
}

#[test]
fn mandatory_finalization_detects_fresh_source_drift() {
    with_projection_compiler_input(|input, database| {
        let mut authority = VerifiedGenerationQualificationRequestProjectionV1::compile(
            input,
            &CancellationToken::new(),
        )
        .expect("verified projection");
        let connection = rusqlite::Connection::open(database).expect("state database");
        connection
            .execute(
                "UPDATE installed_artifacts SET installation_epoch = installation_epoch + 1",
                [],
            )
            .expect("advance installation epoch");
        assert_eq!(
            authority.revalidate_for_mandatory_finalization(),
            Err(GenerationQualificationRequestProjectionError::SourceMismatch)
        );
    });
}
