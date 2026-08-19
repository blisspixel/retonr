use rewrite_types::{
    ClaimEvidence, ClaimExtractionStatus, ClaimModality, ClaimPolarity, Digest, RewriteUnitId,
    SourceSpan,
};
use serde::Deserialize;

use super::ClaimExtractionError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ClaimPayload {
    pub(super) status: ClaimExtractionStatus,
    pub(super) unit_id: String,
    pub(super) text_digest: Digest,
    pub(super) claims: Vec<ClaimPayloadItem>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ClaimPayloadItem {
    claim_id: Digest,
    subject_id: Option<Digest>,
    predicate_id: Digest,
    object_id: Option<Digest>,
    polarity: ClaimPolarity,
    modality: ClaimModality,
    condition_count: u16,
    attributed: bool,
    evidence_spans: Vec<SpanPayload>,
    confidence_ppm: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpanPayload {
    start: usize,
    end: usize,
}

pub(super) fn parse_payload(json: &str) -> Result<ClaimPayload, ClaimExtractionError> {
    serde_json::from_str(json).map_err(|_error| ClaimExtractionError::InvalidPayload)
}

pub(super) fn claims_from_payload(
    payload: ClaimPayload,
    expected_unit: &RewriteUnitId,
    text: &str,
) -> Result<(ClaimExtractionStatus, Vec<ClaimEvidence>), ClaimExtractionError> {
    if payload.unit_id != expected_unit.as_str() {
        return Err(ClaimExtractionError::PayloadMismatch);
    }
    if payload.text_digest != Digest::sha256(text.as_bytes()) {
        return Err(ClaimExtractionError::PayloadMismatch);
    }
    let mut claims = Vec::with_capacity(payload.claims.len());
    for item in payload.claims {
        claims.push(item.into_claim()?);
    }
    Ok((payload.status, claims))
}

impl ClaimPayloadItem {
    fn into_claim(self) -> Result<ClaimEvidence, ClaimExtractionError> {
        let spans = self
            .evidence_spans
            .into_iter()
            .map(|span| {
                SourceSpan::new(span.start, span.end)
                    .map_err(|_error| ClaimExtractionError::InvalidPayload)
            })
            .collect::<Result<Vec<_>, _>>()?;
        ClaimEvidence::from_canonical(
            self.claim_id,
            self.subject_id,
            self.predicate_id,
            self.object_id,
            self.polarity,
            self.modality,
            self.condition_count,
            self.attributed,
            spans,
            self.confidence_ppm,
        )
        .map_err(ClaimExtractionError::InvalidEvidence)
    }
}
