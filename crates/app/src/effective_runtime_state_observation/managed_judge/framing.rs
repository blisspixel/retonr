use rewrite_model::CandidateJudgeScheduleId;
use rewrite_runtime_attestor::RetainedTcpConnectionEvidence;
use rewrite_types::Digest;

const CONNECTION_SPAN_DOMAIN: &[u8] = b"retonr:managed-judge-connection-span:v1";

pub(super) fn connection_span_digest(
    schedule_id: &CandidateJudgeScheduleId,
    span_kind: u8,
    schedule_cursor: u32,
    first: u64,
    last: u64,
    initial: &RetainedTcpConnectionEvidence,
    responses: &[RetainedTcpConnectionEvidence],
) -> Digest {
    let mut bytes = Vec::with_capacity(256 + responses.len() * 72);
    push_field(&mut bytes, CONNECTION_SPAN_DOMAIN);
    push_field(&mut bytes, schedule_id.digest().as_str().as_bytes());
    bytes.push(span_kind);
    bytes.extend_from_slice(&schedule_cursor.to_be_bytes());
    bytes.extend_from_slice(&first.to_be_bytes());
    bytes.extend_from_slice(&last.to_be_bytes());
    bytes.extend_from_slice(&(responses.len() as u64).to_be_bytes());
    push_field(&mut bytes, initial.evidence_digest().as_str().as_bytes());
    for evidence in responses {
        push_field(&mut bytes, evidence.evidence_digest().as_str().as_bytes());
    }
    Digest::sha256(&bytes)
}

pub(super) fn push_field(bytes: &mut Vec<u8>, field: &[u8]) {
    bytes.extend_from_slice(&(field.len() as u64).to_be_bytes());
    bytes.extend_from_slice(field);
}
