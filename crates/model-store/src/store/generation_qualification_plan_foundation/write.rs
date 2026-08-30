use rewrite_model::{
    CandidateSelectionPolicyV1, GenerationCaseManifestV1, GenerationClusterRecordV1,
    GenerationDeterministicCaseContractV1, GenerationRepetitionRecordV1, GenerationSuiteManifestV1,
    MAX_CANDIDATE_SELECTION_POLICY_JSON_BYTES, MAX_GENERATION_CASE_JSON_BYTES,
    MAX_GENERATION_CLUSTER_JSON_BYTES, MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES,
    MAX_GENERATION_PLAN_JSON_BYTES, MAX_GENERATION_REPETITION_JSON_BYTES,
    MAX_GENERATION_SUITE_JSON_BYTES, MAX_PLANNED_CANDIDATE_ATTEMPT_JSON_BYTES,
    PlannedCandidateAttemptV1,
};
use rusqlite::{Connection, OptionalExtension as _, params};

use super::GenerationQualificationPlanFoundationV1Input;
use super::codec::EncodedPlanFoundation;
use crate::store::generation_system_foundation::load_foundation as load_system_foundation;
use crate::{StoreError, StoreResult, WriteDisposition};

#[path = "write/exact.rs"]
mod exact;
use exact::{existing_blob, insert_association, require_record_result};

pub(super) fn insert_foundation(
    connection: &Connection,
    input: GenerationQualificationPlanFoundationV1Input<'_>,
    encoded: &EncodedPlanFoundation,
) -> StoreResult<WriteDisposition> {
    verify_systems(connection, input)?;
    for (cluster, bytes) in input.clusters.iter().zip(&encoded.clusters) {
        insert_cluster(connection, cluster, bytes)?;
    }
    for (contract, bytes) in input
        .deterministic_case_contracts
        .iter()
        .zip(&encoded.deterministic_case_contracts)
    {
        insert_case_contract(connection, contract, bytes)?;
    }
    for (case, bytes) in input.cases.iter().zip(&encoded.cases) {
        insert_case(connection, case, bytes)?;
    }
    insert_suite(connection, input.suite, input.cases.len(), &encoded.suite)?;
    for (ordinal, case) in input.cases.iter().enumerate() {
        insert_association(
            connection,
            "generation_suite_cases",
            "generation_suite_manifest_id",
            input.suite.suite_manifest_id().digest().as_str(),
            "semantic_ordinal",
            ordinal,
            "generation_case_id",
            case.case_id().digest().as_str(),
        )?;
    }
    for (repetition, bytes) in input.repetitions.iter().zip(&encoded.repetitions) {
        insert_repetition(connection, repetition, bytes)?;
    }
    for (attempt, bytes) in input.planned_attempts.iter().zip(&encoded.attempts) {
        insert_attempt(connection, attempt, bytes)?;
    }
    insert_selection_policy(
        connection,
        input.candidate_selection_policy,
        &encoded.candidate_selection_policy,
    )?;
    let disposition = insert_plan(connection, input, &encoded.plan)?;
    let plan_key = input.plan.qualification_plan_id().digest().as_str();
    for (ordinal, repetition) in input.repetitions.iter().enumerate() {
        insert_association(
            connection,
            "generation_qualification_plan_repetitions",
            "generation_qualification_plan_id",
            plan_key,
            "plan_ordinal",
            ordinal,
            "generation_repetition_id",
            repetition.repetition_id().digest().as_str(),
        )?;
    }
    for (ordinal, system) in input.generation_systems.iter().enumerate() {
        insert_association(
            connection,
            "generation_qualification_plan_systems",
            "generation_qualification_plan_id",
            plan_key,
            "plan_ordinal",
            ordinal,
            "generation_system_id",
            system.generation_system_id().digest().as_str(),
        )?;
    }
    for (ordinal, attempt) in input.planned_attempts.iter().enumerate() {
        insert_association(
            connection,
            "generation_qualification_plan_attempts",
            "generation_qualification_plan_id",
            plan_key,
            "plan_ordinal",
            ordinal,
            "planned_candidate_attempt_id",
            attempt.planned_attempt_id().digest().as_str(),
        )?;
    }
    Ok(disposition)
}

fn insert_case_contract(
    connection: &Connection,
    value: &GenerationDeterministicCaseContractV1,
    bytes: &[u8],
) -> StoreResult<()> {
    let key = value.contract_digest().as_str();
    let changed = connection.execute(
        "INSERT INTO generation_deterministic_case_contracts
             (contract_digest, canonical_json)
         VALUES (?1, ?2) ON CONFLICT(contract_digest) DO NOTHING",
        params![key, bytes],
    )?;
    require_record_result(
        connection,
        changed,
        "generation_deterministic_case_contracts",
        "contract_digest",
        key,
        bytes,
        MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES,
    )
}

fn verify_systems(
    connection: &Connection,
    input: GenerationQualificationPlanFoundationV1Input<'_>,
) -> StoreResult<()> {
    for expected in input.generation_systems {
        let stored = load_system_foundation(connection, expected.generation_system_id())?
            .ok_or(StoreError::MissingRecord)?;
        if stored.generation_system() != expected {
            return Err(StoreError::ImmutableConflict);
        }
    }
    Ok(())
}

fn insert_cluster(
    connection: &Connection,
    value: &GenerationClusterRecordV1,
    bytes: &[u8],
) -> StoreResult<()> {
    let key = value.cluster_id().digest().as_str();
    let changed = connection.execute(
        "INSERT INTO generation_cluster_records (generation_cluster_id, canonical_json)
         VALUES (?1, ?2) ON CONFLICT(generation_cluster_id) DO NOTHING",
        params![key, bytes],
    )?;
    require_record_result(
        connection,
        changed,
        "generation_cluster_records",
        "generation_cluster_id",
        key,
        bytes,
        MAX_GENERATION_CLUSTER_JSON_BYTES,
    )
}

fn insert_case(
    connection: &Connection,
    value: &GenerationCaseManifestV1,
    bytes: &[u8],
) -> StoreResult<()> {
    let key = value.case_id().digest().as_str();
    let cluster = value.cluster_id().digest().as_str();
    let contract = value.case_contract_digest().as_str();
    let changed = connection.execute(
        "INSERT INTO generation_case_manifests
             (generation_case_id, generation_cluster_id,
              deterministic_case_contract_digest, canonical_json)
         VALUES (?1, ?2, ?3, ?4) ON CONFLICT(generation_case_id) DO NOTHING",
        params![key, cluster, contract, bytes],
    )?;
    if changed == 1 {
        return Ok(());
    }
    let actual = connection
        .query_row(
            "SELECT generation_cluster_id, deterministic_case_contract_digest
             FROM generation_case_manifests WHERE generation_case_id = ?1",
            [key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    let actual_bytes = existing_blob(
        connection,
        "generation_case_manifests",
        "generation_case_id",
        key,
        MAX_GENERATION_CASE_JSON_BYTES,
    )?;
    if actual == (cluster.to_owned(), contract.to_owned()) && actual_bytes == bytes {
        Ok(())
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

fn insert_selection_policy(
    connection: &Connection,
    value: &CandidateSelectionPolicyV1,
    bytes: &[u8],
) -> StoreResult<()> {
    let key = value.selection_policy_id().digest().as_str();
    let suite = value.suite_manifest_id().digest().as_str();
    let count = i64::from(value.entry_count());
    let changed = connection.execute(
        "INSERT INTO candidate_selection_policies
             (candidate_selection_policy_id, generation_suite_manifest_id,
              entry_count, canonical_json)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(candidate_selection_policy_id) DO NOTHING",
        params![key, suite, count, bytes],
    )?;
    if changed == 1 {
        return Ok(());
    }
    let actual = connection
        .query_row(
            "SELECT generation_suite_manifest_id, entry_count
             FROM candidate_selection_policies WHERE candidate_selection_policy_id = ?1",
            [key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    let actual_bytes = existing_blob(
        connection,
        "candidate_selection_policies",
        "candidate_selection_policy_id",
        key,
        MAX_CANDIDATE_SELECTION_POLICY_JSON_BYTES,
    )?;
    if actual == (suite.to_owned(), count) && actual_bytes == bytes {
        Ok(())
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

fn insert_suite(
    connection: &Connection,
    value: &GenerationSuiteManifestV1,
    case_count: usize,
    bytes: &[u8],
) -> StoreResult<()> {
    let key = value.suite_manifest_id().digest().as_str();
    let count = i64::try_from(case_count).map_err(|_| StoreError::RecordTooLarge)?;
    let changed = connection.execute(
        "INSERT INTO generation_suite_manifests
             (generation_suite_manifest_id, case_count, canonical_json)
         VALUES (?1, ?2, ?3) ON CONFLICT(generation_suite_manifest_id) DO NOTHING",
        params![key, count, bytes],
    )?;
    if changed == 1 {
        return Ok(());
    }
    let actual = connection
        .query_row(
            "SELECT case_count FROM generation_suite_manifests
             WHERE generation_suite_manifest_id = ?1",
            [key],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    let actual_bytes = existing_blob(
        connection,
        "generation_suite_manifests",
        "generation_suite_manifest_id",
        key,
        MAX_GENERATION_SUITE_JSON_BYTES,
    )?;
    if actual == count && actual_bytes == bytes {
        Ok(())
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

fn insert_repetition(
    connection: &Connection,
    value: &GenerationRepetitionRecordV1,
    bytes: &[u8],
) -> StoreResult<()> {
    let key = value.repetition_id().digest().as_str();
    let suite = value.suite_manifest_id().digest().as_str();
    let changed = connection.execute(
        "INSERT INTO generation_repetition_records
             (generation_repetition_id, generation_suite_manifest_id,
              repetition_ordinal, canonical_json)
         VALUES (?1, ?2, ?3, ?4) ON CONFLICT(generation_repetition_id) DO NOTHING",
        params![key, suite, i64::from(value.repetition_ordinal()), bytes],
    )?;
    if changed == 1 {
        return Ok(());
    }
    let actual = connection
        .query_row(
            "SELECT generation_suite_manifest_id, repetition_ordinal
             FROM generation_repetition_records WHERE generation_repetition_id = ?1",
            [key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    let actual_bytes = existing_blob(
        connection,
        "generation_repetition_records",
        "generation_repetition_id",
        key,
        MAX_GENERATION_REPETITION_JSON_BYTES,
    )?;
    if actual == (suite.to_owned(), i64::from(value.repetition_ordinal())) && actual_bytes == bytes
    {
        Ok(())
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

fn insert_attempt(
    connection: &Connection,
    value: &PlannedCandidateAttemptV1,
    bytes: &[u8],
) -> StoreResult<()> {
    let key = value.planned_attempt_id().digest().as_str();
    let expected = (
        value.suite_manifest_id().digest().as_str(),
        value.case_id().digest().as_str(),
        value.cluster_id().digest().as_str(),
        value.repetition_id().digest().as_str(),
        value.generation_system_id().digest().as_str(),
        i64::from(value.attempt_ordinal()),
    );
    let changed = connection.execute(
        "INSERT INTO planned_candidate_attempts
             (planned_candidate_attempt_id, generation_suite_manifest_id, generation_case_id,
              generation_cluster_id, generation_repetition_id, generation_system_id,
              attempt_ordinal, canonical_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(planned_candidate_attempt_id) DO NOTHING",
        params![
            key, expected.0, expected.1, expected.2, expected.3, expected.4, expected.5, bytes,
        ],
    )?;
    if changed == 1 {
        return Ok(());
    }
    let actual = connection
        .query_row(
            "SELECT generation_suite_manifest_id, generation_case_id,
                    generation_cluster_id, generation_repetition_id, generation_system_id,
                    attempt_ordinal
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
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    let actual_bytes = existing_blob(
        connection,
        "planned_candidate_attempts",
        "planned_candidate_attempt_id",
        key,
        MAX_PLANNED_CANDIDATE_ATTEMPT_JSON_BYTES,
    )?;
    if actual
        == (
            expected.0.to_owned(),
            expected.1.to_owned(),
            expected.2.to_owned(),
            expected.3.to_owned(),
            expected.4.to_owned(),
            expected.5,
        )
        && actual_bytes == bytes
    {
        Ok(())
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

fn insert_plan(
    connection: &Connection,
    input: GenerationQualificationPlanFoundationV1Input<'_>,
    bytes: &[u8],
) -> StoreResult<WriteDisposition> {
    let key = input.plan.qualification_plan_id().digest().as_str();
    let expected = (
        input.suite.suite_manifest_id().digest().as_str(),
        input
            .candidate_selection_policy
            .selection_policy_id()
            .digest()
            .as_str(),
        i64::try_from(input.repetitions.len()).map_err(|_| StoreError::RecordTooLarge)?,
        i64::try_from(input.generation_systems.len()).map_err(|_| StoreError::RecordTooLarge)?,
        i64::try_from(input.planned_attempts.len()).map_err(|_| StoreError::RecordTooLarge)?,
    );
    let changed = connection.execute(
        "INSERT INTO generation_qualification_plans
             (generation_qualification_plan_id, generation_suite_manifest_id,
              candidate_selection_policy_id,
              repetition_count, generation_system_count, planned_attempt_count, canonical_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(generation_qualification_plan_id) DO NOTHING",
        params![
            key, expected.0, expected.1, expected.2, expected.3, expected.4, bytes
        ],
    )?;
    if changed == 1 {
        return Ok(WriteDisposition::Inserted);
    }
    let actual = connection
        .query_row(
            "SELECT generation_suite_manifest_id, candidate_selection_policy_id,
                    repetition_count,
                    generation_system_count, planned_attempt_count
             FROM generation_qualification_plans WHERE generation_qualification_plan_id = ?1",
            [key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::CorruptRecord)?;
    let actual_bytes = existing_blob(
        connection,
        "generation_qualification_plans",
        "generation_qualification_plan_id",
        key,
        MAX_GENERATION_PLAN_JSON_BYTES,
    )?;
    if actual
        == (
            expected.0.to_owned(),
            expected.1.to_owned(),
            expected.2,
            expected.3,
            expected.4,
        )
        && actual_bytes == bytes
    {
        Ok(WriteDisposition::AlreadyPresent)
    } else {
        Err(StoreError::ImmutableConflict)
    }
}
