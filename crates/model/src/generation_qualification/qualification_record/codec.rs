use serde::Deserialize;

use super::super::codec::{append_digest, append_u32};
use super::{
    GENERATION_QUALIFICATION_RECORD_ID_DOMAIN, GenerationQualificationLicenseEvidenceId,
    GenerationQualificationOperationPolicyId, GenerationQualificationOperationReceiptId,
    GenerationQualificationPlatformEvidenceId, GenerationQualificationRecordV1,
    GenerationQualificationRecordV1Error, GenerationQualificationRecordV1Relations,
    GenerationQualificationRequestProjectionId, GenerationQualificationStatusV1,
    GenerationSystemId, MAX_GENERATION_QUALIFICATION_RECORD_JSON_BYTES,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QualificationRecordWire {
    schema_version: u32,
    target_generation_system_id: GenerationSystemId,
    baseline_generation_system_id: GenerationSystemId,
    operation_policy_id: GenerationQualificationOperationPolicyId,
    request_projection_id: GenerationQualificationRequestProjectionId,
    platform_evidence_id: GenerationQualificationPlatformEvidenceId,
    license_evidence_id: GenerationQualificationLicenseEvidenceId,
    operation_receipt_id: GenerationQualificationOperationReceiptId,
    status: GenerationQualificationStatusV1,
}

impl QualificationRecordWire {
    pub(super) fn matches(&self, value: &GenerationQualificationRecordV1) -> bool {
        self.schema_version == value.schema_version
            && self.target_generation_system_id == value.target_generation_system_id
            && self.baseline_generation_system_id == value.baseline_generation_system_id
            && self.operation_policy_id == value.operation_policy_id
            && self.request_projection_id == value.request_projection_id
            && self.platform_evidence_id == value.platform_evidence_id
            && self.license_evidence_id == value.license_evidence_id
            && self.operation_receipt_id == value.operation_receipt_id
            && self.status == value.status
    }
}

impl GenerationQualificationRecordV1 {
    /// Decodes bounded canonical JSON using fresh independent expected facts.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for oversized, malformed, noncanonical,
    /// unsupported, substituted, stale, incomplete, or ineligible evidence.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: &GenerationQualificationRecordV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationRecordV1Error> {
        if bytes.len() > MAX_GENERATION_QUALIFICATION_RECORD_JSON_BYTES {
            return Err(GenerationQualificationRecordV1Error::EncodedRecordTooLarge);
        }
        let wire: QualificationRecordWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationRecordV1Error::InvalidEncoding)?;
        if wire.schema_version != super::super::GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationRecordV1Error::UnsupportedSchema);
        }
        let value = Self::build(relations, Some(&wire))?;
        let canonical = serde_json::to_vec(&value)
            .map_err(|_| GenerationQualificationRecordV1Error::InvalidEncoding)?;
        if canonical != bytes {
            return Err(GenerationQualificationRecordV1Error::NonCanonicalEncoding);
        }
        Ok(value)
    }

    pub(super) fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = GENERATION_QUALIFICATION_RECORD_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in [
            self.target_generation_system_id.digest(),
            self.baseline_generation_system_id.digest(),
            self.operation_policy_id.digest(),
            self.request_projection_id.digest(),
            self.platform_evidence_id.digest(),
            self.license_evidence_id.digest(),
            self.operation_receipt_id.digest(),
        ] {
            append_digest(&mut output, digest);
        }
        output.push(status_tag(self.status));
        output
    }
}

const fn status_tag(status: GenerationQualificationStatusV1) -> u8 {
    match status {
        GenerationQualificationStatusV1::Qualified => 0,
        GenerationQualificationStatusV1::Rejected => 1,
    }
}
