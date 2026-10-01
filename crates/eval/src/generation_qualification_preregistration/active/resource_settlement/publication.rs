//! Exact live-parent projection and plan-ordered durable publication.

use super::{
    ActiveGenerationQualificationResourceSettlementError as Error,
    GenerationQualificationPreregistrationRepository, JudgeSettlementContext,
    map_preparation_error,
};
use crate::generation_qualification_preregistration::prepared::PreparedGenerationQualificationValidationView;
use crate::{CompletePassedRepeatabilityRelations, VerifiedCandidateJudgeJoin};
use rewrite_model::{ManagedOllamaCandidateGenerationEvidenceV2Input, PlannedCandidateAttemptId};
use rewrite_model_store::{
    GenerationAttemptLedgerV1Input, GenerationQualificationPreregistrationReadInput,
};
use rewrite_types::CancellationToken;

type ManagedFacts = Vec<(
    PlannedCandidateAttemptId,
    ManagedOllamaCandidateGenerationEvidenceV2Input,
)>;

#[derive(Clone, Copy)]
pub(super) struct ResourceClosure<'a> {
    pub(super) repeatability_manifest: &'a rewrite_model::GenerationRepeatabilityEvidenceManifestV1,
    pub(super) resource_results: &'a [rewrite_model::GenerationResourceAttemptResultRecordV1],
    pub(super) manifest: &'a rewrite_model::GenerationResourceEvidenceManifestV1,
}

pub(super) fn publish(
    context: &JudgeSettlementContext<'_>,
    prepared: &PreparedGenerationQualificationValidationView<'_>,
    closure: CompletePassedRepeatabilityRelations<'_>,
    resource: ResourceClosure<'_>,
    joins: &mut [VerifiedCandidateJudgeJoin<'_, '_, '_, '_>],
    repository: &mut GenerationQualificationPreregistrationRepository,
    cancellation: &CancellationToken,
) -> Result<
    (
        rewrite_model::GenerationResourceEvidenceManifestV1,
        rewrite_model_store::GenerationResourcePhaseV1Disposition,
    ),
    Error,
> {
    let ResourceClosure {
        repeatability_manifest,
        resource_results,
        manifest,
    } = resource;
    validate_scope(context, prepared, closure, joins.len())?;
    let mut managed = Vec::new();
    let mut expected_parents = Vec::new();
    // Reconstruct both candidates and every deterministic/judge parent from
    // retained authority, rather than accepting a SQL identity as authority.
    for (ordinal, join) in joins.iter_mut().enumerate() {
        if !join.matches_active_subject(context.subject) {
            return Err(Error::OperationScope);
        }
        join.with_settlement_view(cancellation, |view| {
            if context
                .validate(prepared, &view, cancellation)
                .map_err(|_| Error::OperationScope)?
                != ordinal
            {
                return Err(Error::OperationScope);
            }
            let (judge_execution, _) = repository
                .persist_judge_execution(view.input, prepared.deadline, cancellation)
                .map_err(map_preparation_error)?;
            expected_parents.push(rewrite_model_store::GenerationRepeatabilityPhaseParentV1 {
                target_receipt_set: view.input.candidate_a_receipt_set.clone(),
                baseline_receipt_set: view.input.candidate_b_receipt_set.clone(),
                deterministic_evaluation: view.input.deterministic_evaluation.clone(),
                judge_execution,
            });
            managed.extend(
                view.target
                    .managed_evidence_inputs(cancellation)
                    .map_err(|_| Error::JoinAuthority)?,
            );
            Ok(())
        })
        .map_err(|()| Error::JoinAuthority)??;
    }
    let ordered = order_managed(prepared, managed)?;
    let ledger_input = GenerationAttemptLedgerV1Input {
        preregistration: GenerationQualificationPreregistrationReadInput {
            operation_policy_id: prepared.operation_policy.operation_policy_id(),
            request_projection_id: prepared.request_projection.request_projection_id(),
            operation_policy_relations: prepared.operation_policy_relations,
            operation_policy_input: prepared.operation_policy_input,
            request_projection_entry_inputs: prepared.request_projection_entry_inputs,
        },
        managed_evidence_inputs: &ordered,
        manifest: context.ledger.manifest(),
    };
    let parent = rewrite_model_store::GenerationRepeatabilityPhaseV1Input {
        ledger: &ledger_input,
        ordered_results: closure.ordered_results,
        expected_parents: &expected_parents,
        manifest: repeatability_manifest,
    };
    let disposition = repository
        .persist_resource_phase(
            rewrite_model_store::GenerationResourcePhaseV1Input {
                repeatability: &parent,
                ordered_results: resource_results,
                manifest,
            },
            prepared.deadline,
            cancellation,
        )
        .map_err(map_preparation_error)?;
    Ok((manifest.clone(), disposition))
}

fn validate_scope(
    context: &JudgeSettlementContext<'_>,
    prepared: &PreparedGenerationQualificationValidationView<'_>,
    closure: CompletePassedRepeatabilityRelations<'_>,
    join_count: usize,
) -> Result<(), Error> {
    let relations = prepared.operation_policy_relations;
    if context.executed_count != relations.repetitions.len()
        || context.settled_count != relations.repetitions.len()
    {
        return Err(Error::NotReady);
    }
    if context.executed_joins.len() != relations.repetitions.len()
        || closure.operation_policy != prepared.operation_policy
        || closure.scope.generation_system != relations.target_system.generation_system
        || closure.scope.qualification_plan != relations.plan
        || closure.scope.suite != relations.suite
        || closure.planned_attempts != relations.planned_attempts
        || closure.preregistered_repetitions != relations.repetitions
        || closure.attempt_ledger_manifest != context.ledger.manifest()
        || join_count != relations.repetitions.len()
    {
        return Err(Error::OperationScope);
    }
    Ok(())
}

fn order_managed(
    prepared: &PreparedGenerationQualificationValidationView<'_>,
    mut managed: ManagedFacts,
) -> Result<Vec<ManagedOllamaCandidateGenerationEvidenceV2Input>, Error> {
    let relations = prepared.operation_policy_relations;
    // Retained sets use semantic suite order. Reconstruct exact target plan
    // order while refusing missing, duplicate, or extraneous retained facts.
    let mut ordered = Vec::new();
    for planned in relations.planned_attempts.iter().filter(|planned| {
        planned.generation_system_id()
            == relations
                .target_system
                .generation_system
                .generation_system_id()
    }) {
        let position = managed
            .iter()
            .position(|(id, _)| id == planned.planned_attempt_id())
            .ok_or(Error::OperationScope)?;
        ordered.push(managed.remove(position).1);
    }
    if managed.is_empty() {
        Ok(ordered)
    } else {
        Err(Error::OperationScope)
    }
}
