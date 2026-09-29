use rewrite_model::{
    GenerationHumanAdjudicationPolicyDenialRecordV1,
    GenerationHumanAdjudicationPolicyDenialRecordV1Relations,
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseScopeV1,
    GenerationResourcePolicyDenialRecordV1, GenerationResourcePolicyDenialRecordV1Relations,
};
use rewrite_types::Digest;

use crate::StoreError;

pub(super) const REASON: &str = "policy_source_denied";

#[derive(Clone, Copy)]
pub(super) struct DenialScope<'a> {
    pub(super) system_id: &'a str,
    pub(super) plan_id: &'a str,
    pub(super) suite_id: &'a str,
    pub(super) policy_digest: &'a str,
}

pub(super) trait DenialRecord: PartialEq + serde::Serialize + Sized {
    type Relations<'a>: Copy;

    const TABLE: &'static str;
    const ID_COLUMN: &'static str;
    const KIND: &'static str;

    fn identity(&self) -> &str;
    fn schema_version(&self) -> u32;
    fn system_id(&self) -> &str;
    fn plan_id(&self) -> &str;
    fn suite_id(&self) -> &str;
    fn policy_digest(&self) -> &str;
    fn validate(
        &self,
        relations: Self::Relations<'_>,
    ) -> Result<(), GenerationQualificationPhaseEvidenceError>;
    fn decode(
        bytes: &[u8],
        relations: Self::Relations<'_>,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError>;
    fn scope(relations: Self::Relations<'_>) -> DenialScope<'_>;
}

pub(super) fn scope_from<'a>(
    scope: GenerationQualificationPhaseScopeV1<'a>,
    policy: &'a Digest,
) -> DenialScope<'a> {
    DenialScope {
        system_id: scope
            .generation_system
            .generation_system_id()
            .digest()
            .as_str(),
        plan_id: scope
            .qualification_plan
            .qualification_plan_id()
            .digest()
            .as_str(),
        suite_id: scope.suite.suite_manifest_id().digest().as_str(),
        policy_digest: policy.as_str(),
    }
}

pub(super) fn map_caller(error: GenerationQualificationPhaseEvidenceError) -> StoreError {
    match error {
        GenerationQualificationPhaseEvidenceError::EncodedRecordTooLarge
        | GenerationQualificationPhaseEvidenceError::CanonicalEncodingTooLarge => {
            StoreError::RecordTooLarge
        }
        GenerationQualificationPhaseEvidenceError::InvalidEncoding
        | GenerationQualificationPhaseEvidenceError::NonCanonicalEncoding
        | GenerationQualificationPhaseEvidenceError::UnsupportedSchema
        | GenerationQualificationPhaseEvidenceError::ScopeMismatch
        | GenerationQualificationPhaseEvidenceError::InvalidCount
        | GenerationQualificationPhaseEvidenceError::RelationshipMismatch
        | GenerationQualificationPhaseEvidenceError::StatusMismatch
        | GenerationQualificationPhaseEvidenceError::CountOverflow
        | GenerationQualificationPhaseEvidenceError::InvalidResourceObservation
        | GenerationQualificationPhaseEvidenceError::InvalidResourceLimitList => {
            StoreError::ImmutableConflict
        }
    }
}

impl DenialRecord for GenerationResourcePolicyDenialRecordV1 {
    type Relations<'a> = GenerationResourcePolicyDenialRecordV1Relations<'a>;

    const TABLE: &'static str = "generation_resource_policy_denial_records";
    const ID_COLUMN: &'static str = "generation_resource_policy_denial_record_id";
    const KIND: &'static str = "resource_policy_denial";

    fn identity(&self) -> &str {
        GenerationResourcePolicyDenialRecordV1::resource_policy_denial_record_id(self)
            .digest()
            .as_str()
    }

    fn schema_version(&self) -> u32 {
        GenerationResourcePolicyDenialRecordV1::schema_version(self)
    }

    fn system_id(&self) -> &str {
        GenerationResourcePolicyDenialRecordV1::generation_system_id(self)
            .digest()
            .as_str()
    }

    fn plan_id(&self) -> &str {
        GenerationResourcePolicyDenialRecordV1::generation_qualification_plan_id(self)
            .digest()
            .as_str()
    }

    fn suite_id(&self) -> &str {
        GenerationResourcePolicyDenialRecordV1::suite_manifest_id(self)
            .digest()
            .as_str()
    }

    fn policy_digest(&self) -> &str {
        GenerationResourcePolicyDenialRecordV1::phase_policy_digest(self).as_str()
    }

    fn validate(
        &self,
        relations: Self::Relations<'_>,
    ) -> Result<(), GenerationQualificationPhaseEvidenceError> {
        GenerationResourcePolicyDenialRecordV1::validate_against(self, relations)
    }

    fn decode(
        bytes: &[u8],
        relations: Self::Relations<'_>,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        Self::from_json_bytes(bytes, relations)
    }

    fn scope(relations: Self::Relations<'_>) -> DenialScope<'_> {
        scope_from(relations.scope, relations.phase_policy_digest)
    }
}

impl DenialRecord for GenerationHumanAdjudicationPolicyDenialRecordV1 {
    type Relations<'a> = GenerationHumanAdjudicationPolicyDenialRecordV1Relations<'a>;

    const TABLE: &'static str = "generation_human_adjudication_policy_denial_records";
    const ID_COLUMN: &'static str = "generation_human_adjudication_policy_denial_record_id";
    const KIND: &'static str = "human_adjudication_policy_denial";

    fn identity(&self) -> &str {
        GenerationHumanAdjudicationPolicyDenialRecordV1::human_adjudication_policy_denial_record_id(
            self,
        )
        .digest()
        .as_str()
    }

    fn schema_version(&self) -> u32 {
        GenerationHumanAdjudicationPolicyDenialRecordV1::schema_version(self)
    }

    fn system_id(&self) -> &str {
        GenerationHumanAdjudicationPolicyDenialRecordV1::generation_system_id(self)
            .digest()
            .as_str()
    }

    fn plan_id(&self) -> &str {
        GenerationHumanAdjudicationPolicyDenialRecordV1::generation_qualification_plan_id(self)
            .digest()
            .as_str()
    }

    fn suite_id(&self) -> &str {
        GenerationHumanAdjudicationPolicyDenialRecordV1::suite_manifest_id(self)
            .digest()
            .as_str()
    }

    fn policy_digest(&self) -> &str {
        GenerationHumanAdjudicationPolicyDenialRecordV1::phase_policy_digest(self).as_str()
    }

    fn validate(
        &self,
        relations: Self::Relations<'_>,
    ) -> Result<(), GenerationQualificationPhaseEvidenceError> {
        GenerationHumanAdjudicationPolicyDenialRecordV1::validate_against(self, relations)
    }

    fn decode(
        bytes: &[u8],
        relations: Self::Relations<'_>,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        Self::from_json_bytes(bytes, relations)
    }

    fn scope(relations: Self::Relations<'_>) -> DenialScope<'_> {
        scope_from(relations.scope, relations.phase_policy_digest)
    }
}
