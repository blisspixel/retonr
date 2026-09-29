use super::codec::CohortRecords;
use crate::{StoreError, StoreResult};

pub(super) struct OrdinalSpan {
    pub(super) first: u64,
    pub(super) last: u64,
}

pub(super) struct Closed {
    pub(super) case_count: i64,
    pub(super) seed: String,
    pub(super) entry_count: i64,
    pub(super) attempt_count: i64,
    pub(super) first_ordinal: i64,
    pub(super) last_ordinal: i64,
}

pub(super) fn presentation_seed(seed: u64) -> StoreResult<String> {
    let text = seed.to_string();
    let canonical = text == "0" || !text.starts_with('0');
    if (1..=20).contains(&text.len()) && canonical && text.bytes().all(|byte| byte.is_ascii_digit())
    {
        Ok(text)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(super) fn ordinal_span(attempt_count: u32) -> StoreResult<OrdinalSpan> {
    if !(2..=512).contains(&attempt_count) || !attempt_count.is_multiple_of(2) {
        return Err(StoreError::CorruptRecord);
    }
    let attempts = u64::from(attempt_count);
    let last = attempts
        .checked_mul(9)
        .and_then(|value| value.checked_add(7))
        .ok_or(StoreError::CorruptRecord)?;
    Ok(OrdinalSpan { first: 8, last })
}

pub(super) fn sql_u64(value: u64) -> StoreResult<i64> {
    i64::try_from(value).map_err(|_| StoreError::CorruptRecord)
}

pub(super) fn close(records: &CohortRecords) -> StoreResult<Closed> {
    let case_count =
        i64::try_from(records.plan.cases().len()).map_err(|_| StoreError::CorruptRecord)?;
    if !(1..=256).contains(&case_count) {
        return Err(StoreError::CorruptRecord);
    }
    let entry_count = case_count.checked_mul(2).ok_or(StoreError::CorruptRecord)?;
    if !(2..=512).contains(&entry_count) || entry_count % 2 != 0 {
        return Err(StoreError::CorruptRecord);
    }
    let attempt_count = i64::from(records.schedule.entry_count());
    if entry_count != attempt_count
        || i64::from(records.requests.entry_count()) != entry_count
        || i64::from(records.responses.entry_count()) != entry_count
        || i64::from(records.observations.entry_count()) != entry_count
        || i64::from(records.receipt.attempt_count()) != entry_count
        || i64::from(records.join.attempt_count()) != entry_count
    {
        return Err(StoreError::CorruptRecord);
    }
    let span = ordinal_span(records.schedule.entry_count())?;
    if records.receipt.first_response_ordinal() != span.first
        || records.receipt.last_response_ordinal() != span.last
        || records.plan.presentation_seed() != records.schedule.presentation_seed()
        || records.plan.schema_version() != 1
        || records.schedule.schema_version() != 1
        || records.requests.schema_version() != 1
        || records.responses.schema_version() != 1
        || records.receipt.schema_version() != 1
        || records.join.schema_version() != 1
        || records.plan.attempts_per_order() != 1
        || records.join.candidate_semantics_proven()
        || records.join.judge_correctness_proven()
        || records.join.qualified()
        || records.receipt.judge_runtime_installation_generation()
            != records.join.judge_runtime_installation_generation()
        || records.receipt.judge_model_installation_generation()
            != records.join.judge_model_installation_generation()
    {
        return Err(StoreError::CorruptRecord);
    }
    sql_u64(records.receipt.judge_runtime_installation_generation())?;
    sql_u64(records.receipt.judge_model_installation_generation())?;
    Ok(Closed {
        case_count,
        seed: presentation_seed(records.plan.presentation_seed())?,
        entry_count,
        attempt_count,
        first_ordinal: sql_u64(span.first)?,
        last_ordinal: sql_u64(span.last)?,
    })
}
