//! Offline tests use a private retained-batch adapter. They exercise the set
//! join but do not represent live managed-execution authority.

use rewrite_model::{
    CandidateSelectionPolicyV1, GenerationCaseManifestV1, GenerationQualificationPlanV1,
    GenerationRepetitionRecordV1, GenerationSuiteManifestV1, GenerationSystemRecordV1,
    PlannedCandidateAttemptV1,
};
use rewrite_types::{CancellationToken, Digest};

use super::{
    CoreError, RetainedCandidateBatch, VerifiedCandidateBatchSet, VerifiedCandidateBatchSetCore,
    VerifiedCandidateBatchSetError, VerifiedCandidateBatchSetRelationship as Relationship,
};

#[path = "tests/support.rs"]
pub(crate) mod support;

#[path = "tests/active_subject.rs"]
mod active_subject;

use support::{OfflineBatchError, scenario};

#[test]
fn exact_set_retains_semantic_order_and_gates_both_outputs() {
    let fixture = scenario("exact");
    let log = fixture.log;
    let set = VerifiedCandidateBatchSetCore::verify(
        fixture.input,
        fixture.batches,
        &CancellationToken::new(),
    )
    .expect("exact batch set");
    assert_eq!(&*log.borrow(), &[0, 1, 0, 1]);
    assert_eq!(set.receipt_set.entry_count(), 2);
    assert_eq!(
        set.receipt_set.entries()[0].case_id(),
        &set.suite.case_ids()[0]
    );
    assert_eq!(
        set.receipt_set.entries()[1].case_id(),
        &set.suite.case_ids()[1]
    );
    let set = offline_authority(set);
    assert!(format!("{set:?}").contains("batch_count: 2"));
    log.borrow_mut().clear();
    set.revalidate(&CancellationToken::new())
        .expect("gated set revalidation");
    assert_eq!(&*log.borrow(), &[0, 1, 0, 1]);

    log.borrow_mut().clear();
    let receipt_set = set
        .receipt_set(&CancellationToken::new())
        .expect("gated receipt set");
    assert_eq!(receipt_set.entry_count(), 2);
    assert_eq!(&*log.borrow(), &[0, 1, 0, 1]);

    log.borrow_mut().clear();
    let selected = set
        .selected_candidates(&CancellationToken::new())
        .expect("gated selected candidates");
    assert_eq!(selected.len(), 2);
    assert!(selected[0].text.contains("selected candidate exact 0"));
    assert!(selected[1].text.contains("selected candidate exact 1"));
    assert_eq!(&*log.borrow(), &[0, 1, 0, 1, 0, 1, 0, 1]);

    log.borrow_mut().clear();
    assert!(
        set.resource_results(&CancellationToken::new())
            .expect("gated resource mode")
            .is_none()
    );
    assert_eq!(&*log.borrow(), &[0, 1, 0, 1, 0, 1, 0, 1]);
}

#[test]
fn strict_set_rejects_silent_resource_omission() {
    let fixture = scenario("strict-missing");
    assert_relationship(
        &VerifiedCandidateBatchSetCore::verify_resource_observed(
            fixture.input,
            fixture.batches,
            &CancellationToken::new(),
        ),
        Relationship::MissingResourceResult,
    );
}

#[test]
fn missing_reordered_and_substituted_batches_are_rejected() {
    let mut missing = scenario("missing");
    missing.batches.pop();
    assert_relationship(
        &VerifiedCandidateBatchSetCore::verify(
            missing.input,
            missing.batches,
            &CancellationToken::new(),
        ),
        Relationship::BatchCount,
    );

    let mut reordered = scenario("reordered");
    reordered.batches.swap(0, 1);
    assert_relationship(
        &VerifiedCandidateBatchSetCore::verify(
            reordered.input,
            reordered.batches,
            &CancellationToken::new(),
        ),
        Relationship::SemanticCaseOrder,
    );

    let mut local = scenario("local");
    let mut foreign = scenario("foreign");
    local.batches[1] = foreign.batches.remove(0);
    assert_relationship(
        &VerifiedCandidateBatchSetCore::verify(
            local.input,
            local.batches,
            &CancellationToken::new(),
        ),
        Relationship::SemanticCaseOrder,
    );
}

#[test]
fn reordered_plan_and_failed_live_selection_are_rejected() {
    let mut reordered = scenario("plan-order");
    reordered.input.planned_attempts.swap(0, 1);
    assert_relationship(
        &VerifiedCandidateBatchSetCore::verify(
            reordered.input,
            reordered.batches,
            &CancellationToken::new(),
        ),
        Relationship::PlannedAttemptClosure,
    );

    let mut selection = scenario("selection");
    selection.batches[1].remove_selected_candidate();
    assert_relationship(
        &VerifiedCandidateBatchSetCore::verify(
            selection.input,
            selection.batches,
            &CancellationToken::new(),
        ),
        Relationship::SelectionClosure,
    );
}

#[test]
fn cancellation_and_batch_revalidation_failure_stop_at_exact_boundary() {
    let cancelled = scenario("cancelled");
    let log = cancelled.log;
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        VerifiedCandidateBatchSetCore::verify(cancelled.input, cancelled.batches, &cancellation,),
        Err(CoreError::Cancelled)
    ));
    assert!(log.borrow().is_empty());

    let failed = scenario("failed-revalidation");
    let log = failed.log;
    failed.batches[1].fail_next_revalidation();
    assert!(matches!(
        VerifiedCandidateBatchSetCore::verify(
            failed.input,
            failed.batches,
            &CancellationToken::new(),
        ),
        Err(CoreError::Batch {
            index: 1,
            source: OfflineBatchError::ForcedRevalidation,
        })
    ));
    assert_eq!(&*log.borrow(), &[0, 1]);
}

#[test]
fn gated_apis_reject_later_drift_and_cancellation() {
    let fixture = scenario("later-drift");
    let set = VerifiedCandidateBatchSetCore::verify(
        fixture.input,
        fixture.batches,
        &CancellationToken::new(),
    )
    .expect("initial set");
    set.batches[0].fail_next_revalidation();
    assert!(matches!(
        set.revalidate(&CancellationToken::new()),
        Err(CoreError::Batch {
            index: 0,
            source: OfflineBatchError::ForcedRevalidation,
        })
    ));

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        set.receipt_set(&cancellation),
        Err(CoreError::Cancelled)
    ));
    assert!(matches!(
        set.selected_candidates(&cancellation),
        Err(CoreError::Cancelled)
    ));
}

#[test]
fn stored_portable_record_and_receipt_set_drift_are_rejected() {
    let local = scenario("portable-local");
    let foreign = scenario("portable-foreign");
    let mut set = VerifiedCandidateBatchSetCore::verify(
        local.input,
        local.batches,
        &CancellationToken::new(),
    )
    .expect("local set");
    set.attempt_records[0] = foreign.batches[0].attempt_record().clone();
    assert!(matches!(
        set.revalidate(&CancellationToken::new()),
        Err(CoreError::Relationship(Relationship::PortableRecordClosure))
    ));

    let local = scenario("set-local");
    let foreign = scenario("set-foreign");
    let mut set = VerifiedCandidateBatchSetCore::verify(
        local.input,
        local.batches,
        &CancellationToken::new(),
    )
    .expect("local set");
    let foreign = VerifiedCandidateBatchSetCore::verify(
        foreign.input,
        foreign.batches,
        &CancellationToken::new(),
    )
    .expect("foreign set");
    set.receipt_set = foreign.receipt_set;
    assert!(matches!(
        set.revalidate(&CancellationToken::new()),
        Err(CoreError::Relationship(Relationship::PortableRecordClosure))
    ));
}

#[test]
fn fixed_batch_bound_is_checked_before_any_revalidation() {
    let mut fixture = scenario("bound-0");
    for index in 1..=128 {
        let mut more = scenario(&format!("bound-{index}"));
        fixture.batches.append(&mut more.batches);
    }
    fixture
        .batches
        .truncate(rewrite_model::MAX_GENERATION_SUITE_CASES + 1);
    assert_relationship(
        &VerifiedCandidateBatchSetCore::verify(
            fixture.input,
            fixture.batches,
            &CancellationToken::new(),
        ),
        Relationship::FixedBound,
    );
    assert!(fixture.log.borrow().is_empty());
}

#[test]
fn public_error_debug_omits_typed_source_internals() {
    let error = VerifiedCandidateBatchSetError::Relationship(Relationship::SelectionClosure);
    let debug = format!("{error:?}");
    assert!(debug.contains("SelectionClosure"));
    assert!(!debug.contains("candidate text"));
}

#[test]
fn production_error_mapping_preserves_closed_kinds_and_redacts_sources() {
    use crate::{VerifiedCandidateBatchError, VerifiedCandidateBatchRelationship};
    use rewrite_model::GenerationQualificationContractError;

    let errors = [
        super::map_core_error(CoreError::Cancelled),
        super::map_core_error(CoreError::Batch {
            index: 3,
            source: VerifiedCandidateBatchError::Relationship(
                VerifiedCandidateBatchRelationship::CandidateBytes,
            ),
        }),
        super::map_core_error(CoreError::Relationship(Relationship::SelectionClosure)),
        super::map_core_error(CoreError::Contract(
            GenerationQualificationContractError::InvalidEncoding,
        )),
    ];
    assert!(matches!(
        errors[0],
        VerifiedCandidateBatchSetError::Cancelled
    ));
    assert!(matches!(
        errors[1],
        VerifiedCandidateBatchSetError::Batch { index: 3, .. }
    ));
    assert!(matches!(
        errors[2],
        VerifiedCandidateBatchSetError::Relationship(Relationship::SelectionClosure)
    ));
    assert!(matches!(
        errors[3],
        VerifiedCandidateBatchSetError::Contract(_)
    ));
    for error in errors {
        assert!(!format!("{error:?}").contains("CandidateBytes"));
    }
}

fn offline_authority(
    core: VerifiedCandidateBatchSetCore<support::OfflineBatch>,
) -> VerifiedCandidateBatchSet {
    super::erase_core(core, map_offline_error)
}

pub(crate) struct OfflinePairedAuthorityFixture {
    pub(crate) qualification_plan: GenerationQualificationPlanV1,
    pub(crate) suite: GenerationSuiteManifestV1,
    pub(crate) repetition: GenerationRepetitionRecordV1,
    pub(crate) selection_policy: CandidateSelectionPolicyV1,
    pub(crate) planned_attempts: Vec<PlannedCandidateAttemptV1>,
    pub(crate) candidate_a_system: GenerationSystemRecordV1,
    pub(crate) candidate_b_system: GenerationSystemRecordV1,
    pub(crate) operation_policy: Option<rewrite_model::GenerationQualificationOperationPolicyV1>,
    pub(crate) candidate_a: VerifiedCandidateBatchSet,
    pub(crate) candidate_b: VerifiedCandidateBatchSet,
    pub(crate) candidate_a_control: support::OfflineBatchFailureControl,
    pub(crate) candidate_b_control: support::OfflineBatchFailureControl,
}

pub(crate) fn offline_paired_authorities(
    suite: &GenerationSuiteManifestV1,
    cases: &[GenerationCaseManifestV1],
    suffix: &str,
) -> OfflinePairedAuthorityFixture {
    let paired = support::paired_scenario(suite, cases, suffix);
    let qualification_plan = paired.candidate_a.input.qualification_plan.clone();
    let suite = paired.candidate_a.input.suite.clone();
    let repetition = paired.candidate_a.input.repetition.clone();
    let selection_policy = paired.candidate_a.input.selection_policy.clone();
    let planned_attempts = paired.candidate_a.input.planned_attempts.clone();
    let first_system = paired.candidate_a.input.generation_system.clone();
    let second_system = paired.candidate_b.input.generation_system.clone();
    let first_control = paired.candidate_a.batches[0].failure_control();
    let second_control = paired.candidate_b.batches[0].failure_control();
    let resource_observed = paired.operation_policy.is_some();
    let operation_policy = paired.operation_policy;
    let candidate_a = offline_authority(
        if resource_observed {
            VerifiedCandidateBatchSetCore::verify_resource_observed(
                paired.candidate_a.input,
                paired.candidate_a.batches,
                &CancellationToken::new(),
            )
        } else {
            VerifiedCandidateBatchSetCore::verify(
                paired.candidate_a.input,
                paired.candidate_a.batches,
                &CancellationToken::new(),
            )
        }
        .expect("candidate A authority"),
    );
    let candidate_b = offline_authority(
        VerifiedCandidateBatchSetCore::verify(
            paired.candidate_b.input,
            paired.candidate_b.batches,
            &CancellationToken::new(),
        )
        .expect("candidate B authority"),
    );
    OfflinePairedAuthorityFixture {
        qualification_plan,
        suite,
        repetition,
        selection_policy,
        planned_attempts,
        candidate_a_system: first_system,
        candidate_b_system: second_system,
        operation_policy,
        candidate_a,
        candidate_b,
        candidate_a_control: first_control,
        candidate_b_control: second_control,
    }
}

pub(crate) fn offline_judge_system(
    identity_label: &str,
    prompt_digest: Digest,
    output_schema_digest: Digest,
) -> GenerationSystemRecordV1 {
    support::judge_system(identity_label, prompt_digest, output_schema_digest)
}

pub(crate) fn offline_judge_system_with_model(
    identity_label: &str,
    prompt_digest: Digest,
    output_schema_digest: Digest,
    model_label: &str,
) -> GenerationSystemRecordV1 {
    support::judge_system_with_model(
        identity_label,
        prompt_digest,
        output_schema_digest,
        model_label,
    )
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "the erased authority error-mapper contract consumes its generic error"
)]
fn map_offline_error(error: CoreError<OfflineBatchError>) -> VerifiedCandidateBatchSetError {
    match error {
        CoreError::Cancelled => VerifiedCandidateBatchSetError::Cancelled,
        CoreError::Batch { .. } => {
            VerifiedCandidateBatchSetError::Relationship(Relationship::PortableRecordClosure)
        }
        CoreError::Relationship(relationship) => {
            VerifiedCandidateBatchSetError::Relationship(relationship)
        }
        CoreError::Contract(source) => VerifiedCandidateBatchSetError::Contract(Box::new(source)),
    }
}

fn assert_relationship(
    result: &Result<
        VerifiedCandidateBatchSetCore<support::OfflineBatch>,
        CoreError<OfflineBatchError>,
    >,
    expected: Relationship,
) {
    assert!(matches!(
        result,
        Err(CoreError::Relationship(observed)) if *observed == expected
    ));
}

#[path = "tests/compiler.rs"]
mod compiler;
