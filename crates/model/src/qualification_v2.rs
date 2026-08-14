use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

use rewrite_types::Digest;

use crate::{
    ArtifactRole, ArtifactSetId, ArtifactSetManifest, EffectivePackageEvidence,
    EffectivePackageEvidenceError, EffectivePackageEvidenceId, EffectiveRuntimeState,
    EffectiveRuntimeStateId, LicenseDecision, QualificationStatus, RuntimeBuildId,
    RuntimeBuildIdentity,
};

mod codec;

/// Current claim-extraction qualification evidence contract version.
pub const QUALIFICATION_V2_SCHEMA_VERSION: u32 = 2;
/// Maximum JSON bytes admitted by the qualification v2 decoder.
pub const MAX_QUALIFICATION_V2_JSON_BYTES: usize = 16_384;
/// Maximum canonical identity bytes for one qualification v2 record.
pub const MAX_QUALIFICATION_V2_CANONICAL_BYTES: usize = 1_024;

/// Content-derived identifier for one qualification v2 evidence record.
///
/// This type is intentionally distinct from the v1 [`crate::QualificationId`], so
/// existing activation APIs cannot consume it.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct QualificationV2Id(Digest);

impl QualificationV2Id {
    /// Returns the digest that defines this qualification evidence.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Caller-supplied policy and result evidence for qualification v2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualificationRecordV2Input {
    /// Maximum admitted UTF-8 source bytes for the tested policy.
    pub source_byte_limit: u64,
    /// Effective context-token ceiling used during qualification.
    pub context_token_limit: u32,
    /// Digest of the exact prompt or chat template.
    pub prompt_template_digest: Digest,
    /// Digest of the exact claim-extraction output schema.
    pub claim_output_contract_digest: Digest,
    /// Digest of parser, canonicalizer, claim schema, and confidence semantics.
    pub claim_operation_contract_digest: Digest,
    /// Digest of exact request, sampling, reasoning, stop, and resource policy.
    pub request_policy_digest: Digest,
    /// Digest of predeclared acceptance, abstention, and calibration thresholds.
    pub threshold_policy_digest: Digest,
    /// Digest of the tested language and locale support policy.
    pub language_policy_digest: Digest,
    /// Digest of the tested hardware and execution envelope.
    pub hardware_envelope_digest: Digest,
    /// Digest of the immutable qualification-suite manifest.
    pub qualification_suite_digest: Digest,
    /// Digest of bounded retained qualification-run results and attestations.
    pub qualification_result_evidence_digest: Digest,
    /// Reviewed permission for the qualified local use or redistribution.
    pub license_decision: LicenseDecision,
    /// Outcome of the predeclared qualification policy.
    pub status: QualificationStatus,
}

/// Inert qualification v2 evidence for one exact claim-extraction tuple.
///
/// The record can describe a passed or rejected qualification run. It cannot
/// authorize activation and has no `authorizes` operation. Persistence and live
/// attestation are deliberately outside this slice.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QualificationRecordV2 {
    schema_version: u32,
    role: ArtifactRole,
    artifact_set_id: ArtifactSetId,
    effective_package_evidence_id: EffectivePackageEvidenceId,
    runtime_build_id: RuntimeBuildId,
    effective_runtime_state_id: EffectiveRuntimeStateId,
    source_byte_limit: u64,
    context_token_limit: u32,
    prompt_template_digest: Digest,
    claim_output_contract_digest: Digest,
    claim_operation_contract_digest: Digest,
    request_policy_digest: Digest,
    threshold_policy_digest: Digest,
    language_policy_digest: Digest,
    hardware_envelope_digest: Digest,
    qualification_suite_digest: Digest,
    qualification_result_evidence_digest: Digest,
    license_decision: LicenseDecision,
    status: QualificationStatus,
}

impl QualificationRecordV2 {
    /// Creates a version 2 record from exact, relationship-checked evidence.
    ///
    /// # Errors
    ///
    /// Returns [`QualificationRecordV2Error`] for a stale or inconsistent reference,
    /// invalid policy, unsupported role or schema, or exceeded canonical bound.
    pub fn new(
        artifact_set: &ArtifactSetManifest,
        runtime_build: &RuntimeBuildIdentity,
        runtime_state: &EffectiveRuntimeState,
        package_evidence: &EffectivePackageEvidence,
        input: QualificationRecordV2Input,
    ) -> Result<Self, QualificationRecordV2Error> {
        package_evidence
            .validate_against(artifact_set, runtime_build, runtime_state)
            .map_err(QualificationRecordV2Error::InvalidPackageEvidence)?;
        let record = Self::from_wire(
            QUALIFICATION_V2_SCHEMA_VERSION,
            ArtifactRole::ClaimExtraction,
            artifact_set.artifact_set_id(),
            package_evidence.effective_package_evidence_id(),
            runtime_build.runtime_build_id(),
            runtime_state.effective_runtime_state_id(),
            input,
        )?;
        record.validate_against(artifact_set, runtime_build, runtime_state, package_evidence)?;
        Ok(record)
    }

    fn from_wire(
        schema_version: u32,
        role: ArtifactRole,
        artifact_set_id: ArtifactSetId,
        effective_package_evidence_id: EffectivePackageEvidenceId,
        runtime_build_id: RuntimeBuildId,
        effective_runtime_state_id: EffectiveRuntimeStateId,
        input: QualificationRecordV2Input,
    ) -> Result<Self, QualificationRecordV2Error> {
        if schema_version != QUALIFICATION_V2_SCHEMA_VERSION {
            return Err(QualificationRecordV2Error::UnsupportedSchema(
                schema_version,
            ));
        }
        if role != ArtifactRole::ClaimExtraction {
            return Err(QualificationRecordV2Error::UnsupportedRole);
        }
        if input.source_byte_limit == 0
            || input.context_token_limit == 0
            || (input.status == QualificationStatus::Qualified
                && input.license_decision == LicenseDecision::Rejected)
        {
            return Err(QualificationRecordV2Error::InvalidPolicy);
        }
        let record = Self {
            schema_version,
            role,
            artifact_set_id,
            effective_package_evidence_id,
            runtime_build_id,
            effective_runtime_state_id,
            source_byte_limit: input.source_byte_limit,
            context_token_limit: input.context_token_limit,
            prompt_template_digest: input.prompt_template_digest,
            claim_output_contract_digest: input.claim_output_contract_digest,
            claim_operation_contract_digest: input.claim_operation_contract_digest,
            request_policy_digest: input.request_policy_digest,
            threshold_policy_digest: input.threshold_policy_digest,
            language_policy_digest: input.language_policy_digest,
            hardware_envelope_digest: input.hardware_envelope_digest,
            qualification_suite_digest: input.qualification_suite_digest,
            qualification_result_evidence_digest: input.qualification_result_evidence_digest,
            license_decision: input.license_decision,
            status: input.status,
        };
        if record.canonical_bytes().len() > MAX_QUALIFICATION_V2_CANONICAL_BYTES {
            return Err(QualificationRecordV2Error::CanonicalEncodingTooLarge);
        }
        Ok(record)
    }

    /// Rechecks every referenced identity and the effective-package relationship.
    ///
    /// # Errors
    ///
    /// Returns [`QualificationRecordV2Error`] when any supplied object differs from
    /// the qualified tuple or package evidence no longer validates against it.
    pub fn validate_against(
        &self,
        artifact_set: &ArtifactSetManifest,
        runtime_build: &RuntimeBuildIdentity,
        runtime_state: &EffectiveRuntimeState,
        package_evidence: &EffectivePackageEvidence,
    ) -> Result<(), QualificationRecordV2Error> {
        if self.artifact_set_id != artifact_set.artifact_set_id() {
            return Err(QualificationRecordV2Error::ArtifactSetMismatch);
        }
        if self.effective_package_evidence_id != package_evidence.effective_package_evidence_id() {
            return Err(QualificationRecordV2Error::PackageEvidenceMismatch);
        }
        if self.runtime_build_id != runtime_build.runtime_build_id() {
            return Err(QualificationRecordV2Error::RuntimeBuildMismatch);
        }
        if self.effective_runtime_state_id != runtime_state.effective_runtime_state_id() {
            return Err(QualificationRecordV2Error::RuntimeStateMismatch);
        }
        package_evidence
            .validate_against(artifact_set, runtime_build, runtime_state)
            .map_err(QualificationRecordV2Error::InvalidPackageEvidence)
    }

    /// Returns the content-derived identity of this complete evidence record.
    #[must_use]
    pub fn qualification_v2_id(&self) -> QualificationV2Id {
        QualificationV2Id(Digest::sha256(&self.canonical_bytes()))
    }

    /// Returns the qualification contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the only role represented by qualification v2.
    #[must_use]
    pub const fn role(&self) -> ArtifactRole {
        self.role
    }

    /// Returns the exact artifact-set identity.
    #[must_use]
    pub const fn artifact_set_id(&self) -> &ArtifactSetId {
        &self.artifact_set_id
    }

    /// Returns the exact effective-package evidence identity.
    #[must_use]
    pub const fn effective_package_evidence_id(&self) -> &EffectivePackageEvidenceId {
        &self.effective_package_evidence_id
    }

    /// Returns the exact runtime-build identity.
    #[must_use]
    pub const fn runtime_build_id(&self) -> &RuntimeBuildId {
        &self.runtime_build_id
    }

    /// Returns the exact effective-runtime-state identity.
    #[must_use]
    pub const fn effective_runtime_state_id(&self) -> &EffectiveRuntimeStateId {
        &self.effective_runtime_state_id
    }

    /// Returns the tested source-byte ceiling.
    #[must_use]
    pub const fn source_byte_limit(&self) -> u64 {
        self.source_byte_limit
    }

    /// Returns the tested effective context-token ceiling.
    #[must_use]
    pub const fn context_token_limit(&self) -> u32 {
        self.context_token_limit
    }

    /// Returns the exact prompt-template identity.
    #[must_use]
    pub const fn prompt_template_digest(&self) -> &Digest {
        &self.prompt_template_digest
    }

    /// Returns the exact claim-output schema identity.
    #[must_use]
    pub const fn claim_output_contract_digest(&self) -> &Digest {
        &self.claim_output_contract_digest
    }

    /// Returns the exact claim-operation contract identity.
    #[must_use]
    pub const fn claim_operation_contract_digest(&self) -> &Digest {
        &self.claim_operation_contract_digest
    }

    /// Returns the exact request-policy identity.
    #[must_use]
    pub const fn request_policy_digest(&self) -> &Digest {
        &self.request_policy_digest
    }

    /// Returns the exact threshold and calibration policy identity.
    #[must_use]
    pub const fn threshold_policy_digest(&self) -> &Digest {
        &self.threshold_policy_digest
    }

    /// Returns the exact language-policy identity.
    #[must_use]
    pub const fn language_policy_digest(&self) -> &Digest {
        &self.language_policy_digest
    }

    /// Returns the exact hardware-envelope identity.
    #[must_use]
    pub const fn hardware_envelope_digest(&self) -> &Digest {
        &self.hardware_envelope_digest
    }

    /// Returns the immutable qualification-suite identity.
    #[must_use]
    pub const fn qualification_suite_digest(&self) -> &Digest {
        &self.qualification_suite_digest
    }

    /// Returns the retained qualification-result evidence identity.
    #[must_use]
    pub const fn qualification_result_evidence_digest(&self) -> &Digest {
        &self.qualification_result_evidence_digest
    }

    /// Returns the reviewed license decision.
    #[must_use]
    pub const fn license_decision(&self) -> LicenseDecision {
        self.license_decision
    }

    /// Returns the predeclared qualification outcome.
    #[must_use]
    pub const fn status(&self) -> QualificationStatus {
        self.status
    }
}

/// Qualification v2 validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum QualificationRecordV2Error {
    /// Encoded input exceeds the fixed pre-decode byte ceiling.
    #[error("encoded qualification v2 record exceeds its limit")]
    EncodedRecordTooLarge,
    /// Encoded JSON is malformed or contains an unknown field.
    #[error("qualification v2 record encoding is invalid")]
    InvalidEncoding,
    /// The qualification schema is unsupported.
    #[error("unsupported qualification v2 schema {0}")]
    UnsupportedSchema(u32),
    /// Qualification v2 represents only claim extraction.
    #[error("qualification v2 role is unsupported")]
    UnsupportedRole,
    /// Resource bounds or the license and outcome combination are invalid.
    #[error("qualification v2 policy is invalid")]
    InvalidPolicy,
    /// Canonical identity bytes exceed the fixed contract ceiling.
    #[error("qualification v2 canonical identity exceeds its limit")]
    CanonicalEncodingTooLarge,
    /// The artifact set differs from the qualification record.
    #[error("qualification v2 artifact-set identity does not match")]
    ArtifactSetMismatch,
    /// Effective-package evidence differs from the qualification record.
    #[error("qualification v2 effective-package evidence does not match")]
    PackageEvidenceMismatch,
    /// The runtime build differs from the qualification record.
    #[error("qualification v2 runtime-build identity does not match")]
    RuntimeBuildMismatch,
    /// The effective runtime state differs from the qualification record.
    #[error("qualification v2 runtime-state identity does not match")]
    RuntimeStateMismatch,
    /// Effective-package evidence fails its own exact relationship checks.
    #[error("qualification v2 package evidence is invalid")]
    InvalidPackageEvidence(#[source] EffectivePackageEvidenceError),
}

#[cfg(test)]
#[path = "qualification_v2/tests.rs"]
mod tests;
