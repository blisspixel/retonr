use rewrite_types::Digest;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::GenerationQualificationOperationContractError;
use super::common::{MAX_GENERATION_QUALIFICATION_POLICY_BYTES, append_u64};

/// Operation-policy identity domain.
pub const GENERATION_QUALIFICATION_OPERATION_POLICY_ID_DOMAIN: &[u8] =
    b"retonr:generation-qualification-operation-policy:v1\0";
/// Request-projection identity domain.
pub const GENERATION_QUALIFICATION_REQUEST_PROJECTION_ID_DOMAIN: &[u8] =
    b"retonr:generation-qualification-request-projection:v1\0";
/// Platform-evidence identity domain.
pub const GENERATION_QUALIFICATION_PLATFORM_EVIDENCE_ID_DOMAIN: &[u8] =
    b"retonr:generation-qualification-platform-evidence:v1\0";
/// License-evidence identity domain.
pub const GENERATION_QUALIFICATION_LICENSE_EVIDENCE_ID_DOMAIN: &[u8] =
    b"retonr:generation-qualification-license-evidence:v1\0";
/// Operation-receipt identity domain.
pub const GENERATION_QUALIFICATION_OPERATION_RECEIPT_ID_DOMAIN: &[u8] =
    b"retonr:generation-qualification-operation-receipt:v1\0";
/// Phase-interruption-record identity domain.
pub const GENERATION_QUALIFICATION_PHASE_INTERRUPTION_RECORD_ID_DOMAIN: &[u8] =
    b"retonr:generation-qualification-phase-interruption-record:v1\0";
/// Platform-assessment-policy identity domain.
pub const GENERATION_QUALIFICATION_PLATFORM_ASSESSMENT_POLICY_ID_DOMAIN: &[u8] =
    b"retonr:generation-qualification-platform-assessment-policy:v1\0";
/// License-assessment-policy identity domain.
pub const GENERATION_QUALIFICATION_LICENSE_ASSESSMENT_POLICY_ID_DOMAIN: &[u8] =
    b"retonr:generation-qualification-license-assessment-policy:v1\0";

macro_rules! content_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(Digest);

        impl $name {
            /// Returns the digest defining this portable identity.
            #[must_use]
            pub const fn digest(&self) -> &Digest {
                &self.0
            }

            pub(super) fn from_canonical_bytes(bytes: &[u8]) -> Self {
                Self(Digest::sha256(bytes))
            }
        }
    };
}

content_id!(
    GenerationQualificationOperationPolicyId,
    "Content identity of one qualification-operation policy."
);
content_id!(
    GenerationQualificationRequestProjectionId,
    "Content identity of one complete qualification request projection."
);
content_id!(
    GenerationQualificationPlatformEvidenceId,
    "Content identity of one qualification platform assessment."
);
content_id!(
    GenerationQualificationLicenseEvidenceId,
    "Content identity of one qualification license assessment."
);
content_id!(
    GenerationQualificationOperationReceiptId,
    "Content identity of one terminal qualification-operation receipt."
);
content_id!(
    GenerationQualificationPhaseInterruptionRecordId,
    "Content identity of one inert qualification phase-interruption record."
);

macro_rules! assessment_policy_id {
    ($name:ident, $domain:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(Digest);

        impl $name {
            /// Derives an inert ID from exact canonical bounded policy bytes.
            ///
            /// This method validates only the byte bound. The application-owned
            /// verifier remains responsible for canonical policy semantics and
            /// for issuing any nonserializable assessment authority.
            ///
            /// # Errors
            ///
            /// Returns an error for an empty or oversized policy encoding.
            pub fn from_canonical_policy_bytes(
                bytes: &[u8],
            ) -> Result<Self, GenerationQualificationOperationContractError> {
                if bytes.is_empty() || bytes.len() > MAX_GENERATION_QUALIFICATION_POLICY_BYTES {
                    return Err(
                        GenerationQualificationOperationContractError::InvalidPolicyEncoding,
                    );
                }
                let mut material = $domain.to_vec();
                append_u64(
                    &mut material,
                    u64::try_from(bytes.len()).map_err(|_| {
                        GenerationQualificationOperationContractError::EncodingOverflow
                    })?,
                );
                material.extend_from_slice(bytes);
                Ok(Self(Digest::sha256(&material)))
            }

            /// Returns the digest defining this inert policy identity.
            #[must_use]
            pub const fn digest(&self) -> &Digest {
                &self.0
            }
        }
    };
}

assessment_policy_id!(
    GenerationQualificationPlatformAssessmentPolicyId,
    GENERATION_QUALIFICATION_PLATFORM_ASSESSMENT_POLICY_ID_DOMAIN,
    "Identity of one canonical app-reviewed platform assessment policy."
);
assessment_policy_id!(
    GenerationQualificationLicenseAssessmentPolicyId,
    GENERATION_QUALIFICATION_LICENSE_ASSESSMENT_POLICY_ID_DOMAIN,
    "Identity of one canonical app-reviewed license assessment policy."
);
