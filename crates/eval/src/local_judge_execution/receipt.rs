use std::fmt;

use rewrite_ollama::OllamaSessionExecutionReceipt;
use rewrite_types::Digest;

use crate::JudgeObservationBatch;

use super::LocalJudgeExecutionError;

const BATCH_DOMAIN: &[u8] = b"retonr:local-judge-execution-batch:v1\0";
const REQUESTS_DOMAIN: &[u8] = b"retonr:local-judge-execution-requests:v1\0";
const RESPONSES_DOMAIN: &[u8] = b"retonr:local-judge-execution-responses:v1\0";
const RECEIPT_DOMAIN: &[u8] = b"retonr:local-judge-execution-receipt:v1\0";
const RESPONSES_PER_ATTEMPT: usize = 7;

/// Explicitly limited authority of a local-judge execution receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalJudgeExecutionEvidenceClass {
    /// Binds one preflighted retained transport and its request/response receipts.
    ///
    /// This does not prove handler execution, model load or use, effective
    /// runtime identity, candidate generation, semantic correctness, or release
    /// qualification.
    RetainedTransportBindingOnly,
}

/// Content-free binding receipt for one locked local-judge run.
///
/// Digests are equality bindings, not anonymization. Predictable inputs and
/// outputs can be recovered by dictionary attack. This type deliberately has no
/// serialization implementation and carries no qualification authority.
#[derive(Clone, Eq, PartialEq)]
pub struct LocalJudgeExecutionReceipt {
    plan_digest: Digest,
    rubric_digest: Digest,
    observation_batch_digest: Digest,
    retained_session_preflight_digest: Digest,
    request_receipts_digest: Digest,
    response_receipts_digest: Digest,
    first_response_ordinal: usize,
    last_response_ordinal: usize,
    attempt_count: usize,
    evidence_class: LocalJudgeExecutionEvidenceClass,
    binding_digest: Digest,
}

impl LocalJudgeExecutionReceipt {
    pub(super) fn new<E>(
        plan_digest: Digest,
        rubric_digest: Digest,
        batch: &JudgeObservationBatch,
        attempts: &[OllamaSessionExecutionReceipt],
    ) -> Result<Self, LocalJudgeExecutionError<E>> {
        let first = attempts
            .first()
            .ok_or(LocalJudgeExecutionError::ReceiptInvariant)?;
        let last = attempts
            .last()
            .ok_or(LocalJudgeExecutionError::ReceiptInvariant)?;
        if batch.observations.len() != attempts.len()
            || attempts.iter().any(|receipt| {
                receipt.preflight_digest() != first.preflight_digest()
                    || receipt
                        .last_response_ordinal()
                        .checked_sub(receipt.first_response_ordinal())
                        .and_then(|span| span.checked_add(1))
                        != Some(RESPONSES_PER_ATTEMPT)
            })
            || !attempts.windows(2).all(|pair| {
                pair[0].last_response_ordinal().checked_add(1)
                    == Some(pair[1].first_response_ordinal())
            })
        {
            return Err(LocalJudgeExecutionError::ReceiptInvariant);
        }
        let observation_batch_digest =
            digest_json(BATCH_DOMAIN, batch).ok_or(LocalJudgeExecutionError::ReceiptInvariant)?;
        let request_receipts_digest = digest_receipts(REQUESTS_DOMAIN, attempts, |receipt| {
            receipt.request_digest()
        });
        let response_receipts_digest = digest_receipts(RESPONSES_DOMAIN, attempts, |receipt| {
            receipt.response_digest()
        });
        let evidence_class = LocalJudgeExecutionEvidenceClass::RetainedTransportBindingOnly;
        let binding_digest = digest_fields(
            RECEIPT_DOMAIN,
            &[
                plan_digest.as_str().as_bytes(),
                rubric_digest.as_str().as_bytes(),
                observation_batch_digest.as_str().as_bytes(),
                first.preflight_digest().as_str().as_bytes(),
                request_receipts_digest.as_str().as_bytes(),
                response_receipts_digest.as_str().as_bytes(),
                &(first.first_response_ordinal() as u64).to_be_bytes(),
                &(last.last_response_ordinal() as u64).to_be_bytes(),
                &(attempts.len() as u64).to_be_bytes(),
                &[0],
            ],
        );
        Ok(Self {
            plan_digest,
            rubric_digest,
            observation_batch_digest,
            retained_session_preflight_digest: first.preflight_digest().clone(),
            request_receipts_digest,
            response_receipts_digest,
            first_response_ordinal: first.first_response_ordinal(),
            last_response_ordinal: last.last_response_ordinal(),
            attempt_count: attempts.len(),
            evidence_class,
            binding_digest,
        })
    }

    /// Returns the exact validated scorecard-plan digest.
    #[must_use]
    pub const fn plan_digest(&self) -> &Digest {
        &self.plan_digest
    }

    /// Returns the exact canonical rubric digest.
    #[must_use]
    pub const fn rubric_digest(&self) -> &Digest {
        &self.rubric_digest
    }

    /// Returns the digest of the exact plan-bound observation batch.
    #[must_use]
    pub const fn observation_batch_digest(&self) -> &Digest {
        &self.observation_batch_digest
    }

    /// Returns the digest of the retained session preflight evidence.
    #[must_use]
    pub const fn retained_session_preflight_digest(&self) -> &Digest {
        &self.retained_session_preflight_digest
    }

    /// Returns the ordered aggregate of exact request receipt digests.
    #[must_use]
    pub const fn request_receipts_digest(&self) -> &Digest {
        &self.request_receipts_digest
    }

    /// Returns the ordered aggregate of exact response receipt digests.
    #[must_use]
    pub const fn response_receipts_digest(&self) -> &Digest {
        &self.response_receipts_digest
    }

    /// Returns the first response ordinal consumed by judge execution.
    #[must_use]
    pub const fn first_response_ordinal(&self) -> usize {
        self.first_response_ordinal
    }

    /// Returns the final response ordinal consumed by judge execution.
    #[must_use]
    pub const fn last_response_ordinal(&self) -> usize {
        self.last_response_ordinal
    }

    /// Returns the exact number of one-shot judge attempts.
    #[must_use]
    pub const fn attempt_count(&self) -> usize {
        self.attempt_count
    }

    /// Returns the deliberately limited receipt authority.
    #[must_use]
    pub const fn evidence_class(&self) -> LocalJudgeExecutionEvidenceClass {
        self.evidence_class
    }

    /// Returns the digest binding every content-free receipt field.
    #[must_use]
    pub const fn binding_digest(&self) -> &Digest {
        &self.binding_digest
    }
}

impl fmt::Debug for LocalJudgeExecutionReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalJudgeExecutionReceipt")
            .field("plan_digest", &self.plan_digest)
            .field("rubric_digest", &self.rubric_digest)
            .field("observation_batch_digest", &self.observation_batch_digest)
            .field(
                "retained_session_preflight_digest",
                &self.retained_session_preflight_digest,
            )
            .field("request_receipts_digest", &self.request_receipts_digest)
            .field("response_receipts_digest", &self.response_receipts_digest)
            .field("first_response_ordinal", &self.first_response_ordinal)
            .field("last_response_ordinal", &self.last_response_ordinal)
            .field("attempt_count", &self.attempt_count)
            .field("evidence_class", &self.evidence_class)
            .field("binding_digest", &self.binding_digest)
            .finish()
    }
}

fn digest_json<T: serde::Serialize>(domain: &[u8], value: &T) -> Option<Digest> {
    serde_json::to_vec(value)
        .ok()
        .map(|encoded| digest_fields(domain, &[&encoded]))
}

fn digest_receipts(
    domain: &[u8],
    receipts: &[OllamaSessionExecutionReceipt],
    select: impl Fn(&OllamaSessionExecutionReceipt) -> &Digest,
) -> Digest {
    let fields = receipts
        .iter()
        .map(|receipt| select(receipt).as_str().as_bytes())
        .collect::<Vec<_>>();
    digest_fields(domain, &fields)
}

fn digest_fields(domain: &[u8], fields: &[&[u8]]) -> Digest {
    let mut material = Vec::new();
    material.extend_from_slice(domain);
    material.extend_from_slice(&(fields.len() as u64).to_be_bytes());
    for field in fields {
        material.extend_from_slice(&(field.len() as u64).to_be_bytes());
        material.extend_from_slice(field);
    }
    Digest::sha256(&material)
}
