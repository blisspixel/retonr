//! Inert records for phase policies denied before phase execution.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::common::{
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseScopeV1,
    validate_canonical_json, validate_scope,
};
use super::phase_id;
use crate::generation_qualification::codec::{append_digest, append_u32};
use crate::generation_qualification::{
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationQualificationPlanId,
    GenerationSuiteManifestId, GenerationSystemId,
};

/// Maximum canonical JSON bytes for one phase-policy denial record.
pub const MAX_GENERATION_PHASE_POLICY_DENIAL_RECORD_JSON_BYTES: usize = 4_096;
const MAX_GENERATION_PHASE_POLICY_DENIAL_RECORD_CANONICAL_BYTES: usize = 512;

/// Resource-policy denial record identity domain.
pub const GENERATION_RESOURCE_POLICY_DENIAL_RECORD_ID_DOMAIN: &[u8] =
    b"retonr:generation-resource-policy-denial-record:v1\0";
/// Human-adjudication-policy denial record identity domain.
pub const GENERATION_HUMAN_ADJUDICATION_POLICY_DENIAL_RECORD_ID_DOMAIN: &[u8] =
    b"retonr:generation-human-adjudication-policy-denial-record:v1\0";

phase_id!(
    GenerationResourcePolicyDenialRecordId,
    "Content identity of one inert resource-policy denial record."
);
phase_id!(
    GenerationHumanAdjudicationPolicyDenialRecordId,
    "Content identity of one inert human-adjudication-policy denial record."
);

/// Closed reason why a phase policy could not permit phase execution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationPhasePolicyDenialReasonV1 {
    /// The configured policy source denied the phase before execution.
    PolicySourceDenied,
}

#[derive(Clone, Copy, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ResourcePolicyDenialRecordKindV1 {
    ResourcePolicyDenial,
}

#[derive(Clone, Copy, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum HumanAdjudicationPolicyDenialRecordKindV1 {
    HumanAdjudicationPolicyDenial,
}

macro_rules! denial_record {
    (
        $record:ident,
        $id:ident,
        $relations:ident,
        $domain:ident,
        $kind:ident,
        $kind_variant:ident,
        $id_accessor:ident,
        $debug_name:literal,
        $doc:literal
    ) => {
        #[doc = $doc]
        #[derive(Clone, Copy)]
        pub struct $relations<'a> {
            /// Exact target, plan, and suite scope.
            pub scope: GenerationQualificationPhaseScopeV1<'a>,
            /// Exact preregistered phase-policy digest.
            pub phase_policy_digest: &'a Digest,
        }

        #[doc = $doc]
        ///
        /// This record is inert. It contains no application authority and cannot
        /// grant permission to execute, persist, activate, or serve a model.
        #[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
        #[serde(deny_unknown_fields)]
        pub struct $record {
            schema_version: u32,
            record_kind: $kind,
            generation_system_id: GenerationSystemId,
            generation_qualification_plan_id: GenerationQualificationPlanId,
            suite_manifest_id: GenerationSuiteManifestId,
            phase_policy_digest: Digest,
            reason: GenerationPhasePolicyDenialReasonV1,
            #[serde(skip)]
            id: $id,
        }

        impl $record {
            /// Derives a source-denied record from exact portable relationships.
            ///
            /// # Errors
            ///
            /// Returns a content-free error unless the target, plan, and suite
            /// form one exact valid phase scope.
            pub fn new(
                relations: $relations<'_>,
            ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
                Self::build(relations, None)
            }

            /// Decodes canonical bounded JSON and rederives every relationship.
            ///
            /// # Errors
            ///
            /// Returns a content-free error for oversized, malformed, duplicated,
            /// trailing, noncanonical, unsupported, cross-phase, or substituted input.
            pub fn from_json_bytes(
                bytes: &[u8],
                relations: $relations<'_>,
            ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
                if bytes.len() > MAX_GENERATION_PHASE_POLICY_DENIAL_RECORD_JSON_BYTES {
                    return Err(GenerationQualificationPhaseEvidenceError::EncodedRecordTooLarge);
                }
                let wire: DenialWire<$kind> = serde_json::from_slice(bytes)
                    .map_err(|_| GenerationQualificationPhaseEvidenceError::InvalidEncoding)?;
                if wire.schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
                    return Err(GenerationQualificationPhaseEvidenceError::UnsupportedSchema);
                }
                let value = Self::build(relations, Some(&wire))?;
                validate_canonical_json(bytes, &value)?;
                Ok(value)
            }

            /// Revalidates this record against exact portable relationships.
            ///
            /// # Errors
            ///
            /// Returns a content-free error if any relationship differs.
            pub fn validate_against(
                &self,
                relations: $relations<'_>,
            ) -> Result<(), GenerationQualificationPhaseEvidenceError> {
                let expected = Self::new(relations)?;
                if self == &expected {
                    Ok(())
                } else {
                    Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
                }
            }

            fn build(
                relations: $relations<'_>,
                wire: Option<&DenialWire<$kind>>,
            ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
                validate_scope(relations.scope)?;
                let canonical = canonical_denial_bytes(
                    $domain,
                    relations.scope,
                    relations.phase_policy_digest,
                )?;
                let value = Self {
                    schema_version: GENERATION_QUALIFICATION_SCHEMA_VERSION,
                    record_kind: $kind::$kind_variant,
                    generation_system_id: relations
                        .scope
                        .generation_system
                        .generation_system_id()
                        .clone(),
                    generation_qualification_plan_id: relations
                        .scope
                        .qualification_plan
                        .qualification_plan_id()
                        .clone(),
                    suite_manifest_id: relations.scope.suite.suite_manifest_id().clone(),
                    phase_policy_digest: relations.phase_policy_digest.clone(),
                    reason: GenerationPhasePolicyDenialReasonV1::PolicySourceDenied,
                    id: $id::from_canonical_bytes(&canonical),
                };
                if wire.is_some_and(|candidate| !candidate.matches(&value)) {
                    return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
                }
                Ok(value)
            }

            /// Returns the record schema version.
            #[must_use]
            pub const fn schema_version(&self) -> u32 {
                self.schema_version
            }

            /// Returns the exact target generation system.
            #[must_use]
            pub const fn generation_system_id(&self) -> &GenerationSystemId {
                &self.generation_system_id
            }

            /// Returns the exact qualification plan.
            #[must_use]
            pub const fn generation_qualification_plan_id(&self) -> &GenerationQualificationPlanId {
                &self.generation_qualification_plan_id
            }

            /// Returns the exact suite.
            #[must_use]
            pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
                &self.suite_manifest_id
            }

            /// Returns the exact phase-policy digest.
            #[must_use]
            pub const fn phase_policy_digest(&self) -> &Digest {
                &self.phase_policy_digest
            }

            /// Returns the closed source-denial reason.
            #[must_use]
            pub const fn reason(&self) -> GenerationPhasePolicyDenialReasonV1 {
                self.reason
            }

            /// Returns the domain-derived content identity.
            #[must_use]
            pub const fn $id_accessor(&self) -> &$id {
                &self.id
            }
        }

        impl fmt::Debug for $record {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct($debug_name)
                    .field("record_id", &self.id)
                    .field("reason", &self.reason)
                    .finish_non_exhaustive()
            }
        }
    };
}

denial_record!(
    GenerationResourcePolicyDenialRecordV1,
    GenerationResourcePolicyDenialRecordId,
    GenerationResourcePolicyDenialRecordV1Relations,
    GENERATION_RESOURCE_POLICY_DENIAL_RECORD_ID_DOMAIN,
    ResourcePolicyDenialRecordKindV1,
    ResourcePolicyDenial,
    resource_policy_denial_record_id,
    "GenerationResourcePolicyDenialRecordV1",
    "Exact inert record that a resource-phase policy source denied execution."
);

denial_record!(
    GenerationHumanAdjudicationPolicyDenialRecordV1,
    GenerationHumanAdjudicationPolicyDenialRecordId,
    GenerationHumanAdjudicationPolicyDenialRecordV1Relations,
    GENERATION_HUMAN_ADJUDICATION_POLICY_DENIAL_RECORD_ID_DOMAIN,
    HumanAdjudicationPolicyDenialRecordKindV1,
    HumanAdjudicationPolicyDenial,
    human_adjudication_policy_denial_record_id,
    "GenerationHumanAdjudicationPolicyDenialRecordV1",
    "Exact inert record that a human-adjudication policy source denied execution."
);

fn canonical_denial_bytes(
    domain: &[u8],
    scope: GenerationQualificationPhaseScopeV1<'_>,
    phase_policy_digest: &Digest,
) -> Result<Vec<u8>, GenerationQualificationPhaseEvidenceError> {
    let mut output = domain.to_vec();
    append_u32(&mut output, GENERATION_QUALIFICATION_SCHEMA_VERSION);
    for digest in [
        scope.generation_system.generation_system_id().digest(),
        scope.qualification_plan.qualification_plan_id().digest(),
        scope.suite.suite_manifest_id().digest(),
        phase_policy_digest,
    ] {
        append_digest(&mut output, digest);
    }
    output.push(0); // GenerationPhasePolicyDenialReasonV1::PolicySourceDenied
    if output.len() > MAX_GENERATION_PHASE_POLICY_DENIAL_RECORD_CANONICAL_BYTES {
        Err(GenerationQualificationPhaseEvidenceError::CanonicalEncodingTooLarge)
    } else {
        Ok(output)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DenialWire<K> {
    schema_version: u32,
    record_kind: K,
    generation_system_id: GenerationSystemId,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    phase_policy_digest: Digest,
    reason: GenerationPhasePolicyDenialReasonV1,
}

impl<K: PartialEq> DenialWire<K> {
    fn matches<R: DenialRecordFields<K>>(&self, value: &R) -> bool {
        self.schema_version == value.schema_version()
            && &self.record_kind == value.record_kind()
            && &self.generation_system_id == value.generation_system_id()
            && &self.generation_qualification_plan_id == value.generation_qualification_plan_id()
            && &self.suite_manifest_id == value.suite_manifest_id()
            && &self.phase_policy_digest == value.phase_policy_digest()
            && self.reason == value.reason()
    }
}

trait DenialRecordFields<K> {
    fn schema_version(&self) -> u32;
    fn record_kind(&self) -> &K;
    fn generation_system_id(&self) -> &GenerationSystemId;
    fn generation_qualification_plan_id(&self) -> &GenerationQualificationPlanId;
    fn suite_manifest_id(&self) -> &GenerationSuiteManifestId;
    fn phase_policy_digest(&self) -> &Digest;
    fn reason(&self) -> GenerationPhasePolicyDenialReasonV1;
}

macro_rules! impl_denial_fields {
    ($record:ident, $kind:ident) => {
        impl DenialRecordFields<$kind> for $record {
            fn schema_version(&self) -> u32 {
                self.schema_version
            }

            fn record_kind(&self) -> &$kind {
                &self.record_kind
            }

            fn generation_system_id(&self) -> &GenerationSystemId {
                &self.generation_system_id
            }

            fn generation_qualification_plan_id(&self) -> &GenerationQualificationPlanId {
                &self.generation_qualification_plan_id
            }

            fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
                &self.suite_manifest_id
            }

            fn phase_policy_digest(&self) -> &Digest {
                &self.phase_policy_digest
            }

            fn reason(&self) -> GenerationPhasePolicyDenialReasonV1 {
                self.reason
            }
        }
    };
}

impl_denial_fields!(
    GenerationResourcePolicyDenialRecordV1,
    ResourcePolicyDenialRecordKindV1
);
impl_denial_fields!(
    GenerationHumanAdjudicationPolicyDenialRecordV1,
    HumanAdjudicationPolicyDenialRecordKindV1
);
