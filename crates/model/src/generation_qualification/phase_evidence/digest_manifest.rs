use std::{collections::HashSet, fmt};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::super::{GenerationQualificationPlanId, GenerationSuiteManifestId, GenerationSystemId};
use super::common::{
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES,
    PhaseManifestFields, canonical_manifest_bytes, fields, root_prefix, validate_canonical_json,
    validate_item_bound,
};
use super::phase_id;
use crate::generation_qualification::codec::append_digest;

/// Resource-evidence manifest identity domain.
pub const GENERATION_RESOURCE_EVIDENCE_MANIFEST_ID_DOMAIN: &[u8] =
    b"retonr:generation-resource-evidence-manifest:v1\0";
/// Resource-evidence root domain.
pub const GENERATION_RESOURCE_EVIDENCE_ROOT_DOMAIN: &[u8] =
    b"retonr:generation-resource-evidence-root:v1\0";
/// Human-adjudication manifest identity domain.
pub const GENERATION_HUMAN_ADJUDICATION_EVIDENCE_MANIFEST_ID_DOMAIN: &[u8] =
    b"retonr:generation-human-adjudication-evidence-manifest:v1\0";
/// Human-adjudication root domain.
pub const GENERATION_HUMAN_ADJUDICATION_EVIDENCE_ROOT_DOMAIN: &[u8] =
    b"retonr:generation-human-adjudication-evidence-root:v1\0";

phase_id!(
    GenerationResourceEvidenceManifestId,
    "Content identity of one generation resource-evidence phase manifest."
);
phase_id!(
    GenerationHumanAdjudicationEvidenceManifestId,
    "Content identity of one generation human-adjudication phase manifest."
);

macro_rules! digest_manifest {
    (
        $record:ident,
        $id:ident,
        $relations:ident,
        $manifest_domain:ident,
        $root_domain:ident,
        $id_accessor:ident,
        $doc:literal
    ) => {
        #[doc = $doc]
        #[derive(Clone, Copy)]
        pub struct $relations<'a> {
            /// Exact target, plan, and suite scope.
            pub scope: GenerationQualificationPhaseScopeV1<'a>,
            /// Exact preregistered phase policy.
            pub phase_policy_digest: &'a Digest,
            /// Semantic-order subordinate content-free record digests.
            pub evidence_record_digests: &'a [Digest],
            /// Closed phase outcome.
            pub status: GenerationQualificationPhaseStatusV1,
        }

        #[doc = $doc]
        #[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
        #[serde(deny_unknown_fields)]
        pub struct $record {
            schema_version: u32,
            generation_system_id: GenerationSystemId,
            generation_qualification_plan_id: GenerationQualificationPlanId,
            suite_manifest_id: GenerationSuiteManifestId,
            phase_policy_digest: Digest,
            evidence_root_digest: Digest,
            evidence_item_count: u32,
            status: GenerationQualificationPhaseStatusV1,
            #[serde(skip)]
            id: $id,
        }

        impl $record {
            /// Derives one manifest from semantic-order content-free result digests.
            ///
            /// # Errors
            ///
            /// Returns a content-free error unless scope, count, status, and every
            /// bounded subordinate digest form the exact relationship.
            pub fn new(
                relations: $relations<'_>,
            ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
                Self::build(relations, None)
            }

            /// Decodes canonical bounded JSON and rederives every relationship.
            ///
            /// # Errors
            ///
            /// Returns a content-free error for oversized, malformed, noncanonical,
            /// unsupported, substituted, duplicated, or status-inconsistent input.
            pub fn from_json_bytes(
                bytes: &[u8],
                relations: $relations<'_>,
            ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
                if bytes.len() > MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES {
                    return Err(GenerationQualificationPhaseEvidenceError::EncodedRecordTooLarge);
                }
                let wire: Wire = serde_json::from_slice(bytes)
                    .map_err(|_| GenerationQualificationPhaseEvidenceError::InvalidEncoding)?;
                if wire.schema_version != super::super::GENERATION_QUALIFICATION_SCHEMA_VERSION {
                    return Err(GenerationQualificationPhaseEvidenceError::UnsupportedSchema);
                }
                let value = Self::build(relations, Some(&wire))?;
                validate_canonical_json(bytes, &value)?;
                Ok(value)
            }

            fn build(
                relations: $relations<'_>,
                wire: Option<&Wire>,
            ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
                validate_item_bound(relations.evidence_record_digests.len())?;
                validate_unique(relations.evidence_record_digests)?;
                let root = derive_digest_root(
                    $root_domain,
                    relations.scope,
                    relations.phase_policy_digest,
                    relations.evidence_record_digests,
                )?;
                let common = fields(
                    relations.scope,
                    relations.phase_policy_digest.clone(),
                    root,
                    relations.evidence_record_digests.len(),
                    relations.status,
                )?;
                let canonical = canonical_manifest_bytes($manifest_domain, &common)?;
                let value = Self::from_common(common, $id::from_canonical_bytes(&canonical));
                if wire.is_some_and(|expected| {
                    !expected.matches(
                        value.schema_version,
                        &value.generation_system_id,
                        &value.generation_qualification_plan_id,
                        &value.suite_manifest_id,
                        &value.phase_policy_digest,
                        &value.evidence_root_digest,
                        value.evidence_item_count,
                        value.status,
                    )
                }) {
                    return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
                }
                Ok(value)
            }

            fn from_common(value: PhaseManifestFields, id: $id) -> Self {
                Self {
                    schema_version: value.schema_version,
                    generation_system_id: value.generation_system_id,
                    generation_qualification_plan_id: value.generation_qualification_plan_id,
                    suite_manifest_id: value.suite_manifest_id,
                    phase_policy_digest: value.phase_policy_digest,
                    evidence_root_digest: value.evidence_root_digest,
                    evidence_item_count: value.evidence_item_count,
                    status: value.status,
                    id,
                }
            }

            /// Returns the target generation system.
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
            /// Returns the exact phase policy.
            #[must_use]
            pub const fn phase_policy_digest(&self) -> &Digest {
                &self.phase_policy_digest
            }
            /// Returns the rederived content-free evidence root.
            #[must_use]
            pub const fn evidence_root_digest(&self) -> &Digest {
                &self.evidence_root_digest
            }
            /// Returns the exact represented item count.
            #[must_use]
            pub const fn evidence_item_count(&self) -> u32 {
                self.evidence_item_count
            }
            /// Returns the closed phase status.
            #[must_use]
            pub const fn status(&self) -> GenerationQualificationPhaseStatusV1 {
                self.status
            }
            /// Returns the content-derived manifest identity.
            #[must_use]
            pub const fn $id_accessor(&self) -> &$id {
                &self.id
            }
        }

        impl fmt::Debug for $record {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($record))
                    .field("manifest_id", &self.id)
                    .field("evidence_item_count", &self.evidence_item_count)
                    .field("status", &self.status)
                    .finish_non_exhaustive()
            }
        }
    };
}

digest_manifest!(
    GenerationResourceEvidenceManifestV1,
    GenerationResourceEvidenceManifestId,
    GenerationResourceEvidenceManifestV1Relations,
    GENERATION_RESOURCE_EVIDENCE_MANIFEST_ID_DOMAIN,
    GENERATION_RESOURCE_EVIDENCE_ROOT_DOMAIN,
    resource_evidence_manifest_id,
    "Exact inert resource-evidence phase manifest."
);
digest_manifest!(
    GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestId,
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GENERATION_HUMAN_ADJUDICATION_EVIDENCE_MANIFEST_ID_DOMAIN,
    GENERATION_HUMAN_ADJUDICATION_EVIDENCE_ROOT_DOMAIN,
    human_adjudication_evidence_manifest_id,
    "Exact inert human-adjudication phase manifest."
);

fn validate_unique(values: &[Digest]) -> Result<(), GenerationQualificationPhaseEvidenceError> {
    let mut seen = HashSet::with_capacity(values.len());
    if values.iter().all(|value| seen.insert(value.as_str())) {
        Ok(())
    } else {
        Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
    }
}

fn derive_digest_root(
    domain: &[u8],
    scope: GenerationQualificationPhaseScopeV1<'_>,
    policy: &Digest,
    values: &[Digest],
) -> Result<Digest, GenerationQualificationPhaseEvidenceError> {
    let mut bytes = root_prefix(domain, scope, policy, values.len())?;
    for value in values {
        append_digest(&mut bytes, value);
    }
    Ok(Digest::sha256(&bytes))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    generation_system_id: GenerationSystemId,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    phase_policy_digest: Digest,
    evidence_root_digest: Digest,
    evidence_item_count: u32,
    status: GenerationQualificationPhaseStatusV1,
}

impl Wire {
    #[expect(
        clippy::too_many_arguments,
        reason = "the exact flat manifest wire relationship remains explicit"
    )]
    fn matches(
        &self,
        schema_version: u32,
        system: &GenerationSystemId,
        plan: &GenerationQualificationPlanId,
        suite: &GenerationSuiteManifestId,
        policy: &Digest,
        root: &Digest,
        count: u32,
        status: GenerationQualificationPhaseStatusV1,
    ) -> bool {
        self.schema_version == schema_version
            && &self.generation_system_id == system
            && &self.generation_qualification_plan_id == plan
            && &self.suite_manifest_id == suite
            && &self.phase_policy_digest == policy
            && &self.evidence_root_digest == root
            && self.evidence_item_count == count
            && self.status == status
    }
}
