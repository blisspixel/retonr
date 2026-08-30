use std::collections::HashSet;

use rewrite_model::{
    CandidateSelectionPolicyV1, GenerationCaseManifestV1, GenerationClusterRecordV1,
    GenerationDeterministicCaseContractV1, GenerationQualificationPlanV1,
    GenerationRepetitionRecordV1, GenerationSuiteManifestV1, PlannedCandidateAttemptV1,
    PlannedCandidateAttemptV1Relations,
};

use super::{
    GenerationQualificationPlanFoundationV1, GenerationQualificationPlanFoundationV1Input,
};
use crate::{StoreError, StoreResult};

pub(super) struct EncodedPlanFoundation {
    pub(super) clusters: Vec<Vec<u8>>,
    pub(super) deterministic_case_contracts: Vec<Vec<u8>>,
    pub(super) cases: Vec<Vec<u8>>,
    pub(super) suite: Vec<u8>,
    pub(super) repetitions: Vec<Vec<u8>>,
    pub(super) attempts: Vec<Vec<u8>>,
    pub(super) candidate_selection_policy: Vec<u8>,
    pub(super) plan: Vec<u8>,
}

pub(super) fn canonical_input(
    input: GenerationQualificationPlanFoundationV1Input<'_>,
) -> StoreResult<EncodedPlanFoundation> {
    validate_collection_bounds(input)?;
    validate_cluster_order(input.clusters, input.cases)?;
    let clusters = encode_and_check(input.clusters, |bytes, _| {
        GenerationClusterRecordV1::from_json_bytes(bytes).map_err(invalid)
    })?;
    let cases = encode_and_check(input.cases, |bytes, case| {
        let cluster = find_cluster(input.clusters, case.cluster_id())?;
        GenerationCaseManifestV1::from_json_bytes(bytes, cluster).map_err(invalid)
    })?;
    let deterministic_case_contracts = input
        .deterministic_case_contracts
        .iter()
        .zip(input.cases)
        .map(|(contract, case)| {
            let bytes = contract
                .to_canonical_json_bytes()
                .map_err(invalid_case_contract)?;
            if GenerationDeterministicCaseContractV1::from_json_bytes(&bytes, case)
                .map_err(invalid_case_contract)?
                == *contract
            {
                Ok(bytes)
            } else {
                Err(StoreError::CorruptRecord)
            }
        })
        .collect::<StoreResult<Vec<_>>>()?;
    let suite = encode_one(input.suite, |bytes| {
        GenerationSuiteManifestV1::from_json_bytes(bytes, input.cases).map_err(invalid)
    })?;
    let repetitions = encode_and_check(input.repetitions, |bytes, _| {
        GenerationRepetitionRecordV1::from_json_bytes(bytes, input.suite).map_err(invalid)
    })?;
    let attempts = encode_and_check(input.planned_attempts, |bytes, attempt| {
        PlannedCandidateAttemptV1::from_json_bytes(bytes, attempt_relations(input, attempt)?)
            .map_err(invalid)
    })?;
    let plan = encode_one(input.plan, |bytes| {
        GenerationQualificationPlanV1::from_json_bytes(
            bytes,
            input.suite,
            input.repetitions,
            input.generation_systems,
            input.planned_attempts,
        )
        .map_err(invalid)
    })?;
    let candidate_selection_policy = encode_one(input.candidate_selection_policy, |bytes| {
        let policy =
            CandidateSelectionPolicyV1::from_json_bytes(bytes, input.suite).map_err(invalid)?;
        policy
            .validate_against_plan(input.plan, input.planned_attempts)
            .map_err(invalid)?;
        Ok(policy)
    })?;
    Ok(EncodedPlanFoundation {
        clusters,
        deterministic_case_contracts,
        cases,
        suite,
        repetitions,
        attempts,
        candidate_selection_policy,
        plan,
    })
}

pub(super) fn matches_input(
    stored: &GenerationQualificationPlanFoundationV1,
    input: GenerationQualificationPlanFoundationV1Input<'_>,
) -> bool {
    stored.clusters == input.clusters
        && stored.deterministic_case_contracts == input.deterministic_case_contracts
        && stored.cases == input.cases
        && stored.suite == *input.suite
        && stored.repetitions == input.repetitions
        && stored.generation_systems == input.generation_systems
        && stored.planned_attempts == input.planned_attempts
        && stored.candidate_selection_policy == *input.candidate_selection_policy
        && stored.plan == *input.plan
}

fn validate_collection_bounds(
    input: GenerationQualificationPlanFoundationV1Input<'_>,
) -> StoreResult<()> {
    if input.cases.is_empty()
        || input.cases.len() > 256
        || input.clusters.is_empty()
        || input.clusters.len() > input.cases.len()
        || input.deterministic_case_contracts.len() != input.cases.len()
        || input.repetitions.is_empty()
        || input.repetitions.len() > 1_024
        || input.generation_systems.is_empty()
        || input.generation_systems.len() > 16
        || input.planned_attempts.is_empty()
        || input.planned_attempts.len() > 1_024
    {
        return Err(StoreError::InvalidGenerationQualificationPlan(
            rewrite_model::GenerationQualificationContractError::PlannedAttemptMismatch,
        ));
    }
    Ok(())
}

pub(super) fn attempt_relations<'a>(
    input: GenerationQualificationPlanFoundationV1Input<'a>,
    attempt: &PlannedCandidateAttemptV1,
) -> StoreResult<PlannedCandidateAttemptV1Relations<'a>> {
    let case = input
        .cases
        .iter()
        .find(|value| value.case_id() == attempt.case_id())
        .ok_or(StoreError::InvalidGenerationQualificationPlan(
            rewrite_model::GenerationQualificationContractError::PlannedAttemptMismatch,
        ))?;
    let cluster = find_cluster(input.clusters, attempt.cluster_id())?;
    let repetition = input
        .repetitions
        .iter()
        .find(|value| value.repetition_id() == attempt.repetition_id())
        .ok_or(StoreError::InvalidGenerationQualificationPlan(
            rewrite_model::GenerationQualificationContractError::PlannedAttemptMismatch,
        ))?;
    let generation_system = input
        .generation_systems
        .iter()
        .find(|value| value.generation_system_id() == attempt.generation_system_id())
        .ok_or(StoreError::InvalidGenerationQualificationPlan(
            rewrite_model::GenerationQualificationContractError::PlannedAttemptMismatch,
        ))?;
    Ok(PlannedCandidateAttemptV1Relations {
        suite: input.suite,
        case,
        cluster,
        repetition,
        generation_system,
    })
}

fn validate_cluster_order(
    clusters: &[GenerationClusterRecordV1],
    cases: &[GenerationCaseManifestV1],
) -> StoreResult<()> {
    let mut seen = HashSet::with_capacity(clusters.len());
    let expected = cases
        .iter()
        .filter_map(|case| {
            seen.insert(case.cluster_id().digest().as_str())
                .then_some(case.cluster_id())
        })
        .collect::<Vec<_>>();
    if expected.len() == clusters.len()
        && expected
            .iter()
            .zip(clusters)
            .all(|(id, cluster)| *id == cluster.cluster_id())
    {
        Ok(())
    } else {
        Err(StoreError::InvalidGenerationQualificationPlan(
            rewrite_model::GenerationQualificationContractError::ClusterMismatch,
        ))
    }
}

fn find_cluster<'a>(
    clusters: &'a [GenerationClusterRecordV1],
    id: &rewrite_model::GenerationClusterId,
) -> StoreResult<&'a GenerationClusterRecordV1> {
    clusters
        .iter()
        .find(|value| value.cluster_id() == id)
        .ok_or(StoreError::InvalidGenerationQualificationPlan(
            rewrite_model::GenerationQualificationContractError::ClusterMismatch,
        ))
}

fn encode_and_check<T: serde::Serialize + PartialEq>(
    values: &[T],
    mut decode: impl FnMut(&[u8], &T) -> StoreResult<T>,
) -> StoreResult<Vec<Vec<u8>>> {
    values
        .iter()
        .map(|value| {
            let bytes = serde_json::to_vec(value)?;
            if decode(&bytes, value)? == *value {
                Ok(bytes)
            } else {
                Err(StoreError::CorruptRecord)
            }
        })
        .collect()
}

fn encode_one<T: serde::Serialize + PartialEq>(
    value: &T,
    decode: impl FnOnce(&[u8]) -> StoreResult<T>,
) -> StoreResult<Vec<u8>> {
    let bytes = serde_json::to_vec(value)?;
    if decode(&bytes)? == *value {
        Ok(bytes)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn invalid(error: rewrite_model::GenerationQualificationContractError) -> StoreError {
    StoreError::InvalidGenerationQualificationPlan(error)
}

fn invalid_case_contract(
    error: rewrite_model::GenerationDeterministicCaseContractError,
) -> StoreError {
    StoreError::InvalidGenerationDeterministicCaseContract(error)
}
