use rewrite_model::{
    CandidateSelectionPolicyV1, GenerationCaseManifestV1, GenerationClusterRecordV1,
    GenerationDeterministicCaseContractV1, GenerationQualificationPlanId,
    GenerationQualificationPlanV1, GenerationRepetitionRecordV1, GenerationSuiteManifestV1,
    GenerationSystemRecordV1, MAX_CANDIDATE_SELECTION_POLICY_JSON_BYTES,
    MAX_GENERATION_CASE_JSON_BYTES, MAX_GENERATION_CLUSTER_JSON_BYTES,
    MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES, MAX_GENERATION_PLAN_JSON_BYTES,
    MAX_GENERATION_REPETITION_JSON_BYTES, MAX_GENERATION_SUITE_JSON_BYTES,
    MAX_PLANNED_CANDIDATE_ATTEMPT_JSON_BYTES, PlannedCandidateAttemptV1,
    PlannedCandidateAttemptV1Relations,
};
use rusqlite::{Connection, OptionalExtension as _};

use super::GenerationQualificationPlanFoundationV1;
use crate::store::generation_system_foundation::{load_foundation as load_system_foundation, read};
use crate::{StoreError, StoreResult};

#[path = "read/bounds.rs"]
pub(super) mod bounds;
use bounds::{bounded_count, load_ordered_ids, load_record_blob, require_digest};

struct PlanRow {
    suite_id: String,
    selection_policy_id: String,
    repetition_count: usize,
    system_count: usize,
    attempt_count: usize,
    canonical_json: Vec<u8>,
}

struct CaseRow {
    cluster_id: String,
    contract_digest: String,
    canonical_json: Vec<u8>,
}

struct AttemptRow {
    suite_id: String,
    case_id: String,
    cluster_id: String,
    repetition_id: String,
    system_id: String,
    attempt_ordinal: u32,
    canonical_json: Vec<u8>,
}

#[expect(
    clippy::too_many_lines,
    reason = "complete recursive typed plan readback"
)]
pub(crate) fn load_foundation(
    connection: &Connection,
    plan_id: &GenerationQualificationPlanId,
) -> StoreResult<Option<GenerationQualificationPlanFoundationV1>> {
    let key = plan_id.digest().as_str();
    let Some(plan_row) = load_plan_row(connection, key)? else {
        return Ok(None);
    };
    let case_ids = load_ordered_ids(
        connection,
        "generation_suite_cases",
        "generation_suite_manifest_id",
        &plan_row.suite_id,
        "semantic_ordinal",
        "generation_case_id",
        load_suite_count(connection, &plan_row.suite_id)?,
    )?;
    let mut clusters = Vec::new();
    let mut deterministic_case_contracts = Vec::with_capacity(case_ids.len());
    let mut cases = Vec::with_capacity(case_ids.len());
    for case_id in &case_ids {
        let row = load_case_row(connection, case_id)?;
        let cluster_index = clusters
            .iter()
            .position(|value: &GenerationClusterRecordV1| {
                value.cluster_id().digest().as_str() == row.cluster_id
            });
        let cluster = if let Some(index) = cluster_index {
            &clusters[index]
        } else {
            let bytes = load_record_blob(
                connection,
                "generation_cluster_records",
                "generation_cluster_id",
                &row.cluster_id,
                MAX_GENERATION_CLUSTER_JSON_BYTES,
            )?
            .ok_or(StoreError::CorruptRecord)?;
            let cluster = GenerationClusterRecordV1::from_json_bytes(&bytes)
                .map_err(|_| StoreError::CorruptRecord)?;
            if cluster.cluster_id().digest().as_str() != row.cluster_id {
                return Err(StoreError::CorruptRecord);
            }
            clusters.push(cluster);
            clusters.last().ok_or(StoreError::CorruptRecord)?
        };
        let case = GenerationCaseManifestV1::from_json_bytes(&row.canonical_json, cluster)
            .map_err(|_| StoreError::CorruptRecord)?;
        if case.case_id().digest().as_str() != case_id
            || case.cluster_id().digest().as_str() != row.cluster_id
            || case.case_contract_digest().as_str() != row.contract_digest
        {
            return Err(StoreError::CorruptRecord);
        }
        let contract_bytes = load_record_blob(
            connection,
            "generation_deterministic_case_contracts",
            "contract_digest",
            &row.contract_digest,
            MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES,
        )?
        .ok_or(StoreError::CorruptRecord)?;
        let contract =
            GenerationDeterministicCaseContractV1::from_json_bytes(&contract_bytes, &case)
                .map_err(|_| StoreError::CorruptRecord)?;
        if contract.contract_digest().as_str() != row.contract_digest
            || contract
                .to_canonical_json_bytes()
                .map_err(|_| StoreError::CorruptRecord)?
                != contract_bytes
        {
            return Err(StoreError::CorruptRecord);
        }
        deterministic_case_contracts.push(contract);
        cases.push(case);
    }
    let suite_bytes = load_record_blob(
        connection,
        "generation_suite_manifests",
        "generation_suite_manifest_id",
        &plan_row.suite_id,
        MAX_GENERATION_SUITE_JSON_BYTES,
    )?
    .ok_or(StoreError::CorruptRecord)?;
    let suite = GenerationSuiteManifestV1::from_json_bytes(&suite_bytes, &cases)
        .map_err(|_| StoreError::CorruptRecord)?;
    if suite.suite_manifest_id().digest().as_str() != plan_row.suite_id {
        return Err(StoreError::CorruptRecord);
    }
    let repetition_ids = load_ordered_ids(
        connection,
        "generation_qualification_plan_repetitions",
        "generation_qualification_plan_id",
        key,
        "plan_ordinal",
        "generation_repetition_id",
        plan_row.repetition_count,
    )?;
    let repetitions = repetition_ids
        .iter()
        .map(|id| load_repetition(connection, id, &suite))
        .collect::<StoreResult<Vec<_>>>()?;
    let system_ids = load_ordered_ids(
        connection,
        "generation_qualification_plan_systems",
        "generation_qualification_plan_id",
        key,
        "plan_ordinal",
        "generation_system_id",
        plan_row.system_count,
    )?;
    let generation_systems = system_ids
        .iter()
        .map(|id| load_system(connection, id))
        .collect::<StoreResult<Vec<_>>>()?;
    let attempt_ids = load_ordered_ids(
        connection,
        "generation_qualification_plan_attempts",
        "generation_qualification_plan_id",
        key,
        "plan_ordinal",
        "planned_candidate_attempt_id",
        plan_row.attempt_count,
    )?;
    let planned_attempts = attempt_ids
        .iter()
        .map(|id| {
            load_attempt(
                connection,
                id,
                &suite,
                &cases,
                &clusters,
                &repetitions,
                &generation_systems,
            )
        })
        .collect::<StoreResult<Vec<_>>>()?;
    let plan = GenerationQualificationPlanV1::from_json_bytes(
        &plan_row.canonical_json,
        &suite,
        &repetitions,
        &generation_systems,
        &planned_attempts,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    if plan.qualification_plan_id() != plan_id
        || plan.suite_manifest_id().digest().as_str() != plan_row.suite_id
        || repetitions.len() != plan_row.repetition_count
        || plan.generation_system_ids().len() != plan_row.system_count
        || plan.planned_attempt_ids().len() != plan_row.attempt_count
        || serde_json::to_vec(&plan)? != plan_row.canonical_json
    {
        return Err(StoreError::CorruptRecord);
    }
    let candidate_selection_policy =
        load_selection_policy(connection, &plan_row.selection_policy_id, &suite)?;
    candidate_selection_policy
        .validate_against_plan(&plan, &planned_attempts)
        .map_err(|_| StoreError::CorruptRecord)?;
    Ok(Some(GenerationQualificationPlanFoundationV1 {
        clusters,
        deterministic_case_contracts,
        cases,
        suite,
        repetitions,
        generation_systems,
        planned_attempts,
        candidate_selection_policy,
        plan,
    }))
}

fn load_plan_row(connection: &Connection, key: &str) -> StoreResult<Option<PlanRow>> {
    let row = connection
        .query_row(
            "SELECT generation_suite_manifest_id, candidate_selection_policy_id,
                    repetition_count, generation_system_count, planned_attempt_count,
                    typeof(canonical_json), length(canonical_json)
             FROM generation_qualification_plans
             WHERE generation_qualification_plan_id = ?1",
            [key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()?;
    let Some((suite_id, selection_policy_id, repetitions, systems, attempts, kind, length)) = row
    else {
        return Ok(None);
    };
    require_digest(&suite_id)?;
    require_digest(&selection_policy_id)?;
    let canonical_json = read::read_bounded_blob(
        connection,
        "generation_qualification_plans",
        "generation_qualification_plan_id",
        key,
        &kind,
        length,
        MAX_GENERATION_PLAN_JSON_BYTES,
    )?;
    Ok(Some(PlanRow {
        suite_id,
        selection_policy_id,
        repetition_count: bounded_count(repetitions, 1_024)?,
        system_count: bounded_count(systems, 16)?,
        attempt_count: bounded_count(attempts, 1_024)?,
        canonical_json,
    }))
}

fn load_suite_count(connection: &Connection, suite_id: &str) -> StoreResult<usize> {
    let count = connection
        .query_row(
            "SELECT case_count FROM generation_suite_manifests
             WHERE generation_suite_manifest_id = ?1",
            [suite_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    bounded_count(count, 256)
}

fn load_case_row(connection: &Connection, key: &str) -> StoreResult<CaseRow> {
    let (cluster_id, contract_digest, kind, length) = connection
        .query_row(
            "SELECT generation_cluster_id, deterministic_case_contract_digest,
                    typeof(canonical_json), length(canonical_json)
             FROM generation_case_manifests WHERE generation_case_id = ?1",
            [key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    require_digest(&cluster_id)?;
    require_digest(&contract_digest)?;
    let bytes = read::read_bounded_blob(
        connection,
        "generation_case_manifests",
        "generation_case_id",
        key,
        &kind,
        length,
        MAX_GENERATION_CASE_JSON_BYTES,
    )?;
    Ok(CaseRow {
        cluster_id,
        contract_digest,
        canonical_json: bytes,
    })
}

fn load_selection_policy(
    connection: &Connection,
    key: &str,
    suite: &GenerationSuiteManifestV1,
) -> StoreResult<CandidateSelectionPolicyV1> {
    let (suite_id, entry_count, kind, length) = connection
        .query_row(
            "SELECT generation_suite_manifest_id, entry_count,
                    typeof(canonical_json), length(canonical_json)
             FROM candidate_selection_policies WHERE candidate_selection_policy_id = ?1",
            [key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    require_digest(&suite_id)?;
    let bytes = read::read_bounded_blob(
        connection,
        "candidate_selection_policies",
        "candidate_selection_policy_id",
        key,
        &kind,
        length,
        MAX_CANDIDATE_SELECTION_POLICY_JSON_BYTES,
    )?;
    let value = CandidateSelectionPolicyV1::from_json_bytes(&bytes, suite)
        .map_err(|_| StoreError::CorruptRecord)?;
    if value.selection_policy_id().digest().as_str() == key
        && value.suite_manifest_id().digest().as_str() == suite_id
        && i64::from(value.entry_count()) == entry_count
        && serde_json::to_vec(&value)? == bytes
    {
        Ok(value)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn load_repetition(
    connection: &Connection,
    key: &str,
    suite: &GenerationSuiteManifestV1,
) -> StoreResult<GenerationRepetitionRecordV1> {
    let (suite_id, ordinal, kind, length) = connection
        .query_row(
            "SELECT generation_suite_manifest_id, repetition_ordinal,
                    typeof(canonical_json), length(canonical_json)
             FROM generation_repetition_records WHERE generation_repetition_id = ?1",
            [key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    require_digest(&suite_id)?;
    let bytes = read::read_bounded_blob(
        connection,
        "generation_repetition_records",
        "generation_repetition_id",
        key,
        &kind,
        length,
        MAX_GENERATION_REPETITION_JSON_BYTES,
    )?;
    let value = GenerationRepetitionRecordV1::from_json_bytes(&bytes, suite)
        .map_err(|_| StoreError::CorruptRecord)?;
    if value.repetition_id().digest().as_str() == key
        && value.suite_manifest_id().digest().as_str() == suite_id
        && i64::from(value.repetition_ordinal()) == ordinal
        && serde_json::to_vec(&value)? == bytes
    {
        Ok(value)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn load_system(connection: &Connection, key: &str) -> StoreResult<GenerationSystemRecordV1> {
    let id: rewrite_model::GenerationSystemId =
        serde_json::from_value(serde_json::Value::String(key.to_owned()))
            .map_err(|_| StoreError::CorruptRecord)?;
    load_system_foundation(connection, &id)?
        .map(|foundation| foundation.generation_system().clone())
        .ok_or(StoreError::CorruptRecord)
}

fn load_attempt(
    connection: &Connection,
    key: &str,
    suite: &GenerationSuiteManifestV1,
    cases: &[GenerationCaseManifestV1],
    clusters: &[GenerationClusterRecordV1],
    repetitions: &[GenerationRepetitionRecordV1],
    systems: &[GenerationSystemRecordV1],
) -> StoreResult<PlannedCandidateAttemptV1> {
    let row = load_attempt_row(connection, key)?;
    let case = cases
        .iter()
        .find(|value| value.case_id().digest().as_str() == row.case_id.as_str())
        .ok_or(StoreError::CorruptRecord)?;
    let cluster = clusters
        .iter()
        .find(|value| value.cluster_id().digest().as_str() == row.cluster_id.as_str())
        .ok_or(StoreError::CorruptRecord)?;
    let repetition = repetitions
        .iter()
        .find(|value| value.repetition_id().digest().as_str() == row.repetition_id.as_str())
        .ok_or(StoreError::CorruptRecord)?;
    let system = systems
        .iter()
        .find(|value| value.generation_system_id().digest().as_str() == row.system_id.as_str())
        .ok_or(StoreError::CorruptRecord)?;
    let value = PlannedCandidateAttemptV1::from_json_bytes(
        &row.canonical_json,
        PlannedCandidateAttemptV1Relations {
            suite,
            case,
            cluster,
            repetition,
            generation_system: system,
        },
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    if value.planned_attempt_id().digest().as_str() == key
        && value.suite_manifest_id().digest().as_str() == row.suite_id
        && value.attempt_ordinal() == row.attempt_ordinal
        && serde_json::to_vec(&value)? == row.canonical_json
    {
        Ok(value)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn load_attempt_row(connection: &Connection, key: &str) -> StoreResult<AttemptRow> {
    let row = connection
        .query_row(
            "SELECT generation_suite_manifest_id, generation_case_id, generation_cluster_id,
                    generation_repetition_id, generation_system_id, attempt_ordinal,
                    typeof(canonical_json), length(canonical_json)
             FROM planned_candidate_attempts WHERE planned_candidate_attempt_id = ?1",
            [key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    for id in [&row.0, &row.1, &row.2, &row.3, &row.4] {
        require_digest(id)?;
    }
    let canonical_json = read::read_bounded_blob(
        connection,
        "planned_candidate_attempts",
        "planned_candidate_attempt_id",
        key,
        &row.6,
        row.7,
        MAX_PLANNED_CANDIDATE_ATTEMPT_JSON_BYTES,
    )?;
    Ok(AttemptRow {
        suite_id: row.0,
        case_id: row.1,
        cluster_id: row.2,
        repetition_id: row.3,
        system_id: row.4,
        attempt_ordinal: u32::try_from(row.5).map_err(|_| StoreError::CorruptRecord)?,
        canonical_json,
    })
}
