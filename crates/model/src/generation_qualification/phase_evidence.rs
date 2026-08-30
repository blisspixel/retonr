//! Inert content-free evidence manifests for generation qualification phases.

mod attempt_ledger;
mod common;
mod denial_record;
mod digest_manifest;
mod repeatability;
mod resource_result;

pub use attempt_ledger::{
    GENERATION_ATTEMPT_LEDGER_MANIFEST_ID_DOMAIN, GENERATION_ATTEMPT_LEDGER_ROOT_DOMAIN,
    GenerationAttemptLedgerManifestId, GenerationAttemptLedgerManifestV1,
    GenerationAttemptLedgerManifestV1Relations,
};
pub use common::{
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, MAX_GENERATION_QUALIFICATION_PHASE_ITEMS,
    MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES,
};
pub use denial_record::{
    GENERATION_HUMAN_ADJUDICATION_POLICY_DENIAL_RECORD_ID_DOMAIN,
    GENERATION_RESOURCE_POLICY_DENIAL_RECORD_ID_DOMAIN,
    GenerationHumanAdjudicationPolicyDenialRecordId,
    GenerationHumanAdjudicationPolicyDenialRecordV1,
    GenerationHumanAdjudicationPolicyDenialRecordV1Relations, GenerationPhasePolicyDenialReasonV1,
    GenerationResourcePolicyDenialRecordId, GenerationResourcePolicyDenialRecordV1,
    GenerationResourcePolicyDenialRecordV1Relations,
    MAX_GENERATION_PHASE_POLICY_DENIAL_RECORD_JSON_BYTES,
};
pub use digest_manifest::{
    GENERATION_HUMAN_ADJUDICATION_EVIDENCE_MANIFEST_ID_DOMAIN,
    GENERATION_HUMAN_ADJUDICATION_EVIDENCE_ROOT_DOMAIN,
    GENERATION_RESOURCE_EVIDENCE_MANIFEST_ID_DOMAIN, GENERATION_RESOURCE_EVIDENCE_ROOT_DOMAIN,
    GenerationHumanAdjudicationEvidenceManifestId, GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestV1Relations, GenerationResourceEvidenceManifestId,
    GenerationResourceEvidenceManifestV1, GenerationResourceEvidenceManifestV1Relations,
};
pub use repeatability::{
    GENERATION_REPEATABILITY_EVIDENCE_MANIFEST_ID_DOMAIN,
    GENERATION_REPEATABILITY_EVIDENCE_ROOT_DOMAIN, GENERATION_REPEATABILITY_RESULT_ID_DOMAIN,
    GenerationRepeatabilityEvidenceManifestId, GenerationRepeatabilityEvidenceManifestV1,
    GenerationRepeatabilityEvidenceManifestV1Relations, GenerationRepeatabilityResultId,
    GenerationRepeatabilityResultRecordV1, GenerationRepeatabilityResultRecordV1Relations,
    GenerationRepeatabilityTerminalStageV1, MAX_GENERATION_REPEATABILITY_RESULT_JSON_BYTES,
};
pub use resource_result::{
    GENERATION_RESOURCE_ATTEMPT_RESULT_ID_DOMAIN, GenerationResourceAttemptResultId,
    GenerationResourceAttemptResultRecordV1, GenerationResourceAttemptResultRecordV1Input,
    GenerationResourceAttemptResultRecordV1Relations, GenerationResourceExceededLimitV1,
    GenerationResourceObservationProfileV1, MAX_GENERATION_RESOURCE_ATTEMPT_RESULT_JSON_BYTES,
};

macro_rules! phase_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Clone,
            Debug,
            serde::Deserialize,
            Eq,
            Hash,
            schemars::JsonSchema,
            PartialEq,
            serde::Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(rewrite_types::Digest);

        impl $name {
            /// Returns the digest that defines this inert portable identity.
            #[must_use]
            pub const fn digest(&self) -> &rewrite_types::Digest {
                &self.0
            }

            fn from_canonical_bytes(bytes: &[u8]) -> Self {
                Self(rewrite_types::Digest::sha256(bytes))
            }
        }
    };
}

pub(super) use phase_id;

#[cfg(test)]
mod tests;
