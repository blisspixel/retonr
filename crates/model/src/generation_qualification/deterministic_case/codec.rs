use rewrite_types::{Digest, ReasonCode, RewriteStatus};

use super::{
    GenerationDeterministicCaseContractError, GenerationDeterministicCaseContractV1Input,
    MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES,
};
use crate::{ExpectedOutput, ReferenceJudgment};

const GENERATION_DETERMINISTIC_CASE_CONTRACT_DIGEST_DOMAIN: &[u8] =
    b"retonr:generation-deterministic-case-contract:v1\0";

pub(super) fn canonical_identity_bytes(
    schema_version: u32,
    input: &GenerationDeterministicCaseContractV1Input,
) -> Result<Vec<u8>, GenerationDeterministicCaseContractError> {
    let length = canonical_identity_length(input)?;
    let mut output = Vec::with_capacity(length);
    output.extend_from_slice(GENERATION_DETERMINISTIC_CASE_CONTRACT_DIGEST_DOMAIN);
    append_u32(&mut output, schema_version);
    append_text(&mut output, &input.case_key);
    append_digest(&mut output, input.source_artifact_id.digest());
    append_digest(&mut output, &input.source_digest);
    append_u64(&mut output, input.source_byte_count);
    append_digest(&mut output, &input.language_digest);
    append_digest(&mut output, &input.mode_digest);
    append_digest(&mut output, &input.format_digest);
    append_text(&mut output, &input.evaluation_category);
    append_text_set(&mut output, &input.protected_terms)?;
    output.push(reference_judgment_tag(input.reference_judgment));
    output.push(rewrite_status_tag(input.expected_status));
    match input.expected_reason {
        None => output.push(0),
        Some(reason) => {
            output.push(1);
            output.push(reason_code_tag(reason));
        }
    }
    output.push(expected_output_tag(input.expected_output));
    append_text_set(&mut output, &input.rubric_clause_ids)?;
    debug_assert_eq!(output.len(), length);
    Ok(output)
}

fn canonical_identity_length(
    input: &GenerationDeterministicCaseContractV1Input,
) -> Result<usize, GenerationDeterministicCaseContractError> {
    let mut length = GENERATION_DETERMINISTIC_CASE_CONTRACT_DIGEST_DOMAIN.len();
    add_length(&mut length, 4)?;
    add_text_length(&mut length, &input.case_key)?;
    add_length(&mut length, 64 * 5 + 8)?;
    add_text_length(&mut length, &input.evaluation_category)?;
    add_text_set_length(&mut length, &input.protected_terms)?;
    add_length(&mut length, 4)?;
    if input.expected_reason.is_some() {
        add_length(&mut length, 1)?;
    }
    add_text_set_length(&mut length, &input.rubric_clause_ids)?;
    if length > MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES {
        return Err(GenerationDeterministicCaseContractError::CanonicalEncodingTooLarge);
    }
    Ok(length)
}

fn add_text_set_length(
    length: &mut usize,
    values: &[String],
) -> Result<(), GenerationDeterministicCaseContractError> {
    add_length(length, 8)?;
    for value in values {
        add_text_length(length, value)?;
    }
    Ok(())
}

fn add_text_length(
    length: &mut usize,
    value: &str,
) -> Result<(), GenerationDeterministicCaseContractError> {
    add_length(length, 8)?;
    add_length(length, value.len())
}

fn add_length(
    length: &mut usize,
    addition: usize,
) -> Result<(), GenerationDeterministicCaseContractError> {
    *length = length
        .checked_add(addition)
        .ok_or(GenerationDeterministicCaseContractError::CanonicalEncodingTooLarge)?;
    if *length > MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES {
        return Err(GenerationDeterministicCaseContractError::CanonicalEncodingTooLarge);
    }
    Ok(())
}

fn append_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn append_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn append_digest(output: &mut Vec<u8>, value: &Digest) {
    output.extend_from_slice(value.as_str().as_bytes());
}

fn append_text(output: &mut Vec<u8>, value: &str) {
    append_u64(output, value.len() as u64);
    output.extend_from_slice(value.as_bytes());
}

fn append_text_set(
    output: &mut Vec<u8>,
    values: &[String],
) -> Result<(), GenerationDeterministicCaseContractError> {
    append_u64(
        output,
        u64::try_from(values.len())
            .map_err(|_| GenerationDeterministicCaseContractError::CanonicalEncodingTooLarge)?,
    );
    for value in values {
        append_text(output, value);
    }
    Ok(())
}

const fn reference_judgment_tag(value: ReferenceJudgment) -> u8 {
    match value {
        ReferenceJudgment::Acceptable => 0,
        ReferenceJudgment::Unacceptable => 1,
        ReferenceJudgment::Identity => 2,
        ReferenceJudgment::NotApplicable => 3,
    }
}

const fn rewrite_status_tag(value: RewriteStatus) -> u8 {
    match value {
        RewriteStatus::Rewritten => 0,
        RewriteStatus::UnchangedNoEligibleContent => 1,
        RewriteStatus::Abstained => 2,
        RewriteStatus::Failed => 3,
    }
}

const fn reason_code_tag(value: ReasonCode) -> u8 {
    match value {
        ReasonCode::NoCandidate => 0,
        ReasonCode::InvalidCandidate => 1,
        ReasonCode::SentinelIntegrity => 2,
        ReasonCode::ProtectedValueChanged => 3,
        ReasonCode::StructureChanged => 4,
        ReasonCode::UnsafeText => 5,
        ReasonCode::SemanticMismatch => 6,
        ReasonCode::SemanticUncertain => 7,
        ReasonCode::ReassemblyVerification => 8,
        ReasonCode::Cancelled => 9,
        ReasonCode::UnsupportedAtomicity => 10,
    }
}

const fn expected_output_tag(value: ExpectedOutput) -> u8 {
    match value {
        ExpectedOutput::Source => 0,
        ExpectedOutput::Candidate => 1,
    }
}
