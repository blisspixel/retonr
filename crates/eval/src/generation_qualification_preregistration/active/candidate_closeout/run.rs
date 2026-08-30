//! Candidate closeout execution inside a freshly validated Prepared view.

use rewrite_app::{
    CandidateGenerationEvidenceBundlePublicationPlan,
    CandidateGenerationEvidenceBundlePublicationPlanInput,
    CandidateGenerationEvidenceBundleReadbackLease, CandidateGenerationReceiptCompilationInput,
    CandidateGenerationReceiptCompiler, RetainedStructuredResponseArtifactV1,
};
use rewrite_model::GenerationQualificationPhaseScopeV1;
use rewrite_model_store::{
    CandidateGenerationEvidenceBundleStorageV1, CandidateGenerationEvidenceStorageV1Limits,
};
use rewrite_types::CancellationToken;

use super::{
    ActiveGenerationQualificationCandidateCloseoutInput,
    failure::{
        CloseoutPrimaryFailure, batch_failure, publication_failure, receipt_failure,
        repository_failure,
    },
};
use crate::generation_qualification_preregistration::{
    active::attempt_ledger::ActiveCandidateAttemptScope,
    prepared::PreparedGenerationQualificationValidationView,
};
use crate::{
    VerifiedCandidateBatch, VerifiedCandidateBatchResourceInput,
    VerifiedCompletedManagedCandidateAttempt,
};

pub(super) struct CandidateCloseoutProduct {
    pub(super) batch: VerifiedCandidateBatch,
    pub(super) storage: CandidateGenerationEvidenceBundleStorageV1,
}

pub(super) fn run_closeout(
    input: &ActiveGenerationQualificationCandidateCloseoutInput<'_>,
    completed_attempt: Box<VerifiedCompletedManagedCandidateAttempt>,
    pending: &ActiveCandidateAttemptScope,
    view: &PreparedGenerationQualificationValidationView<'_>,
    cancellation: &CancellationToken,
) -> Result<CandidateCloseoutProduct, CloseoutPrimaryFailure> {
    let case = input.case;
    let cluster = input.cluster;
    let repetition = input.repetition;
    let auxiliary_artifacts = input.auxiliary_artifacts;
    let manifest = input.manifest;
    let limits = input.limits;
    if case.case_id() != pending.planned_attempt.case_id()
        || cluster.cluster_id() != pending.planned_attempt.cluster_id()
        || repetition.repetition_id() != pending.planned_attempt.repetition_id()
        || input.resource_policy.is_some() != pending.target
    {
        return Err(CloseoutPrimaryFailure::operation_scope());
    }
    let structured_response = RetainedStructuredResponseArtifactV1::from_retained_response(
        completed_attempt.response(),
        completed_attempt.residency_receipt().execution(),
    )
    .map_err(publication_failure)?;
    let plan = CandidateGenerationEvidenceBundlePublicationPlan::compile(
        &CandidateGenerationEvidenceBundlePublicationPlanInput {
            qualification_plan: view.operation_policy_relations.plan,
            planned_attempt: completed_attempt.planned_attempt(),
            case_manifest: case,
            precursor: completed_attempt.precursor(),
            managed_evidence: completed_attempt.managed_evidence(),
            cleanup: completed_attempt.cleanup(),
            structured_response: &structured_response,
            structured_request: completed_attempt.structured_request(),
            auxiliary_artifacts,
            manifest,
        },
        limits,
        cancellation,
    )
    .map_err(publication_failure)?;
    let storage = input
        .evidence_repository
        .bundle_storage_reference(
            view.operation_policy_relations
                .plan
                .qualification_plan_id()
                .clone(),
            completed_attempt
                .planned_attempt()
                .planned_attempt_id()
                .clone(),
            manifest.evidence_bundle_id().clone(),
            storage_limits(limits)?,
        )
        .map_err(repository_failure)?;
    let (storage, readback_lease) = publish_bundle(input, storage, plan, cancellation)?;
    let receipt_compilation = CandidateGenerationReceiptCompiler::compile(
        CandidateGenerationReceiptCompilationInput {
            qualification_plan: view.operation_policy_relations.plan,
            suite: view.operation_policy_relations.suite,
            case,
            cluster,
            repetition,
            planned_attempt: completed_attempt.planned_attempt(),
            generation_system: completed_attempt.generation_system(),
            effective_package: completed_attempt.effective_package().evidence(),
            precursor: completed_attempt.precursor(),
            managed_evidence: completed_attempt.managed_evidence(),
            cleanup: completed_attempt.cleanup(),
            structured_request: completed_attempt.structured_request(),
            readback_lease,
        },
        cancellation,
    )
    .map_err(receipt_failure)?;
    verify_batch(
        completed_attempt,
        receipt_compilation,
        storage,
        pending,
        view,
        input,
        cancellation,
    )
}

fn publish_bundle(
    input: &ActiveGenerationQualificationCandidateCloseoutInput<'_>,
    storage: CandidateGenerationEvidenceBundleStorageV1,
    plan: CandidateGenerationEvidenceBundlePublicationPlan,
    cancellation: &CancellationToken,
) -> Result<
    (
        CandidateGenerationEvidenceBundleStorageV1,
        CandidateGenerationEvidenceBundleReadbackLease,
    ),
    CloseoutPrimaryFailure,
> {
    input
        .evidence_repository
        .publish_bundle(storage, plan, cancellation)
        .map_err(repository_failure)
        .map(rewrite_app::CandidateGenerationEvidenceRepositoryPublication::into_parts)
}

fn storage_limits(
    limits: rewrite_app::CandidateGenerationEvidenceBundleLimits,
) -> Result<CandidateGenerationEvidenceStorageV1Limits, CloseoutPrimaryFailure> {
    CandidateGenerationEvidenceStorageV1Limits::new(
        u32::try_from(limits.maximum_tree_entries).map_err(publication_failure)?,
        u16::try_from(limits.maximum_tree_depth).map_err(publication_failure)?,
        limits.maximum_total_bytes,
    )
    .map_err(publication_failure)
}

fn verify_batch(
    completed_attempt: Box<VerifiedCompletedManagedCandidateAttempt>,
    receipt_compilation: rewrite_app::CandidateGenerationReceiptCompilation,
    storage: CandidateGenerationEvidenceBundleStorageV1,
    pending: &ActiveCandidateAttemptScope,
    view: &PreparedGenerationQualificationValidationView<'_>,
    input: &ActiveGenerationQualificationCandidateCloseoutInput<'_>,
    cancellation: &CancellationToken,
) -> Result<CandidateCloseoutProduct, CloseoutPrimaryFailure> {
    if pending.target {
        VerifiedCandidateBatch::verify_resource_observed(
            *completed_attempt,
            receipt_compilation,
            VerifiedCandidateBatchResourceInput {
                scope: GenerationQualificationPhaseScopeV1 {
                    generation_system: view
                        .operation_policy_relations
                        .target_system
                        .generation_system,
                    qualification_plan: view.operation_policy_relations.plan,
                    suite: view.operation_policy_relations.suite,
                },
                operation_policy: view.operation_policy,
                resource_policy: input
                    .resource_policy
                    .ok_or_else(CloseoutPrimaryFailure::operation_scope)?,
                case: input.case,
                repetition: input.repetition,
            },
            cancellation,
        )
        .map(|batch| CandidateCloseoutProduct { batch, storage })
        .map_err(batch_failure)
    } else {
        VerifiedCandidateBatch::verify(*completed_attempt, receipt_compilation, cancellation)
            .map(|batch| CandidateCloseoutProduct { batch, storage })
            .map_err(batch_failure)
    }
}
