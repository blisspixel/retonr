//! Canonical app-owned resource and human-adjudication phase policies.

use std::fmt;

use rewrite_model::GenerationQualificationOperationPolicyV1;
use rewrite_types::Digest;
use thiserror::Error;

mod verification;
mod wire;

/// Current canonical qualification phase-policy schema.
pub const GENERATION_QUALIFICATION_PHASE_POLICY_SCHEMA_VERSION: u32 = 1;
/// Fixed resource-policy verification procedure identity.
pub const GENERATION_QUALIFICATION_RESOURCE_POLICY_PROCEDURE_ID: &str =
    "retonr:generation-qualification-resource-policy:procedure";
/// Fixed resource-policy verification procedure version.
pub const GENERATION_QUALIFICATION_RESOURCE_POLICY_PROCEDURE_VERSION: u32 = 1;
/// Fixed human-adjudication-policy verification procedure identity.
pub const GENERATION_QUALIFICATION_HUMAN_ADJUDICATION_POLICY_PROCEDURE_ID: &str =
    "retonr:generation-qualification-human-adjudication-policy:procedure";
/// Fixed human-adjudication-policy verification procedure version.
pub const GENERATION_QUALIFICATION_HUMAN_ADJUDICATION_POLICY_PROCEDURE_VERSION: u32 = 1;
/// Hard ceiling for one canonical phase policy.
pub const MAX_GENERATION_QUALIFICATION_PHASE_POLICY_JSON_BYTES: usize = 64 * 1_024;

const RESOURCE_POLICY_DIGEST_DOMAIN: &[u8] =
    b"retonr:generation-qualification-resource-policy:v1\0";
const HUMAN_ADJUDICATION_POLICY_DIGEST_DOMAIN: &[u8] =
    b"retonr:generation-qualification-human-adjudication-policy:v1\0";
const PRODUCTION_RESOURCE_POLICY_APPROVALS: &[&str] = &[];
const PRODUCTION_HUMAN_ADJUDICATION_POLICY_APPROVALS: &[&str] = &[];

/// Exact source-controlled disposition of a structurally verified phase policy.
///
/// A denied policy remains inert. It grants no launch, traffic, generation,
/// evidence, adjudication, or qualification authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationQualificationPhasePolicySourceDisposition {
    /// The exact domain-separated policy digest is source-approved.
    Approved,
    /// The exact domain-separated policy digest is absent from its source root.
    Denied,
}

/// Application-owned immutable resource-policy source-approval root.
///
/// Production construction is empty and fail-closed. The type has no public
/// mutation, decoding, or caller-selected approval surface.
pub struct ProductionGenerationQualificationResourcePolicySource {
    approvals: Vec<Digest>,
}

impl ProductionGenerationQualificationResourcePolicySource {
    /// Returns the empty production resource-policy source root.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            approvals: Vec::new(),
        }
    }

    /// Returns the number of exact source-approved policy digests.
    #[must_use]
    pub const fn approved_policy_count(&self) -> usize {
        PRODUCTION_RESOURCE_POLICY_APPROVALS.len() + self.approvals.len()
    }

    fn disposition(
        &self,
        policy_digest: &Digest,
    ) -> GenerationQualificationPhasePolicySourceDisposition {
        disposition(
            PRODUCTION_RESOURCE_POLICY_APPROVALS
                .iter()
                .any(|approved| *approved == policy_digest.as_str())
                || self
                    .approvals
                    .iter()
                    .any(|approved| approved == policy_digest),
        )
    }

    #[cfg(test)]
    pub(crate) fn exact_test_source(policy_digest: Digest) -> Self {
        Self {
            approvals: vec![policy_digest],
        }
    }
}

impl Default for ProductionGenerationQualificationResourcePolicySource {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ProductionGenerationQualificationResourcePolicySource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionGenerationQualificationResourcePolicySource")
            .field("approved_policy_count", &self.approved_policy_count())
            .finish_non_exhaustive()
    }
}

/// Application-owned immutable human-adjudication-policy source-approval root.
///
/// Production construction is empty and fail-closed. The type has no public
/// mutation, decoding, or caller-selected approval surface.
pub struct ProductionGenerationQualificationHumanAdjudicationPolicySource {
    approvals: Vec<Digest>,
}

impl ProductionGenerationQualificationHumanAdjudicationPolicySource {
    /// Returns the empty production human-adjudication-policy source root.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            approvals: Vec::new(),
        }
    }

    /// Returns the number of exact source-approved policy digests.
    #[must_use]
    pub const fn approved_policy_count(&self) -> usize {
        PRODUCTION_HUMAN_ADJUDICATION_POLICY_APPROVALS.len() + self.approvals.len()
    }

    fn disposition(
        &self,
        policy_digest: &Digest,
    ) -> GenerationQualificationPhasePolicySourceDisposition {
        disposition(
            PRODUCTION_HUMAN_ADJUDICATION_POLICY_APPROVALS
                .iter()
                .any(|approved| *approved == policy_digest.as_str())
                || self
                    .approvals
                    .iter()
                    .any(|approved| approved == policy_digest),
        )
    }

    #[cfg(test)]
    pub(crate) fn exact_test_source(policy_digest: Digest) -> Self {
        Self {
            approvals: vec![policy_digest],
        }
    }
}

impl Default for ProductionGenerationQualificationHumanAdjudicationPolicySource {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ProductionGenerationQualificationHumanAdjudicationPolicySource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionGenerationQualificationHumanAdjudicationPolicySource")
            .field("approved_policy_count", &self.approved_policy_count())
            .finish_non_exhaustive()
    }
}

/// Noncloneable structurally verified resource phase-policy authority.
///
/// Only an `Approved` source disposition may later contribute positive resource
/// evidence. Structural verification alone does not measure resource use.
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationResourcePolicy;
///
/// fn clone_authority(value: &VerifiedGenerationQualificationResourcePolicy) {
///     let _forged: VerifiedGenerationQualificationResourcePolicy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationResourcePolicy;
///
/// fn serialize_authority(value: &VerifiedGenerationQualificationResourcePolicy) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedGenerationQualificationResourcePolicy {
    policy_digest: Digest,
    source_disposition: GenerationQualificationPhasePolicySourceDisposition,
    limits: GenerationQualificationResourcePolicyLimitsV1,
}

impl VerifiedGenerationQualificationResourcePolicy {
    /// Returns the domain-separated canonical policy digest.
    #[must_use]
    pub const fn policy_digest(&self) -> &Digest {
        &self.policy_digest
    }

    /// Returns the exact application-owned source-approval disposition.
    #[must_use]
    pub const fn source_disposition(&self) -> GenerationQualificationPhasePolicySourceDisposition {
        self.source_disposition
    }

    /// Returns every exact nonzero V1 resource ceiling.
    #[must_use]
    pub const fn limits(&self) -> GenerationQualificationResourcePolicyLimitsV1 {
        self.limits
    }

    /// Revalidates the exact operation-policy digest relationship.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationPhasePolicyError::OperationPolicyMismatch`]
    /// when the operation names another resource policy.
    pub fn revalidate_operation_policy(
        &self,
        operation_policy: &GenerationQualificationOperationPolicyV1,
    ) -> Result<(), GenerationQualificationPhasePolicyError> {
        verification::validate_resource_operation_digest(&self.policy_digest, operation_policy)
    }
}

impl fmt::Debug for VerifiedGenerationQualificationResourcePolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationQualificationResourcePolicy")
            .field("source_disposition", &self.source_disposition)
            .finish_non_exhaustive()
    }
}

/// Exact nonzero V1 resource-policy ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationQualificationResourcePolicyLimitsV1 {
    /// Maximum elapsed time for one managed candidate attempt.
    pub maximum_attempt_elapsed_nanoseconds: u64,
    /// Maximum elapsed time before the first provider response.
    pub maximum_first_response_nanoseconds: u64,
    /// Maximum elapsed time for managed cleanup.
    pub maximum_cleanup_nanoseconds: u64,
    /// Maximum observed worker high-water resident bytes.
    pub maximum_worker_high_water_resident_bytes: u64,
    /// Maximum installed runtime and model footprint bytes.
    pub maximum_installed_footprint_bytes: u64,
}

/// Noncloneable structurally verified human-adjudication phase-policy authority.
///
/// Only an `Approved` source disposition may later contribute positive human
/// evidence. Structural verification is not evidence that review occurred.
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationHumanAdjudicationPolicy;
///
/// fn clone_authority(value: &VerifiedGenerationQualificationHumanAdjudicationPolicy) {
///     let _forged: VerifiedGenerationQualificationHumanAdjudicationPolicy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationHumanAdjudicationPolicy;
///
/// fn serialize_authority(value: &VerifiedGenerationQualificationHumanAdjudicationPolicy) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedGenerationQualificationHumanAdjudicationPolicy {
    policy_digest: Digest,
    source_disposition: GenerationQualificationPhasePolicySourceDisposition,
    presentation_seed: u64,
}

impl VerifiedGenerationQualificationHumanAdjudicationPolicy {
    /// Returns the domain-separated canonical policy digest.
    #[must_use]
    pub const fn policy_digest(&self) -> &Digest {
        &self.policy_digest
    }

    /// Returns the exact application-owned source-approval disposition.
    #[must_use]
    pub const fn source_disposition(&self) -> GenerationQualificationPhasePolicySourceDisposition {
        self.source_disposition
    }

    /// Returns the preregistered deterministic presentation seed.
    #[must_use]
    pub const fn presentation_seed(&self) -> u64 {
        self.presentation_seed
    }

    /// Returns the fixed number of independent primary reviewers.
    #[must_use]
    pub const fn primary_reviewer_count(&self) -> u32 {
        2
    }

    /// Revalidates the exact operation-policy digest relationship.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationPhasePolicyError::OperationPolicyMismatch`]
    /// when the operation names another human-adjudication policy.
    pub fn revalidate_operation_policy(
        &self,
        operation_policy: &GenerationQualificationOperationPolicyV1,
    ) -> Result<(), GenerationQualificationPhasePolicyError> {
        verification::validate_human_operation_digest(&self.policy_digest, operation_policy)
    }
}

impl fmt::Debug for VerifiedGenerationQualificationHumanAdjudicationPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationQualificationHumanAdjudicationPolicy")
            .field("source_disposition", &self.source_disposition)
            .finish_non_exhaustive()
    }
}

/// Independent verifier for canonical resource and human-adjudication policies.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenerationQualificationPhasePolicyVerifier;

impl GenerationQualificationPhasePolicyVerifier {
    /// Verifies one resource policy and its exact operation-policy binding.
    ///
    /// Source denial is a valid verified disposition, not an error.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for invalid, noncanonical, oversized,
    /// unsupported, zero-limit, or operation-substituted input.
    pub fn verify_resource(
        policy_json: &[u8],
        operation_policy: &GenerationQualificationOperationPolicyV1,
        source: &ProductionGenerationQualificationResourcePolicySource,
    ) -> Result<
        VerifiedGenerationQualificationResourcePolicy,
        GenerationQualificationPhasePolicyError,
    > {
        verification::verify_resource(policy_json, operation_policy, source)
    }

    /// Verifies one human policy and its exact operation-policy binding.
    ///
    /// Source denial is a valid verified disposition, not an error.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for invalid, noncanonical, oversized,
    /// unsupported, or operation-substituted input.
    pub fn verify_human_adjudication(
        policy_json: &[u8],
        operation_policy: &GenerationQualificationOperationPolicyV1,
        source: &ProductionGenerationQualificationHumanAdjudicationPolicySource,
    ) -> Result<
        VerifiedGenerationQualificationHumanAdjudicationPolicy,
        GenerationQualificationPhasePolicyError,
    > {
        verification::verify_human(policy_json, operation_policy, source)
    }
}

/// Qualification phase-policy verification failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationPhasePolicyError {
    /// Policy JSON was empty, malformed, or contained an unknown value or field.
    #[error("generation qualification phase policy encoding is invalid")]
    InvalidEncoding,
    /// Policy JSON was valid but not its exact compact canonical encoding.
    #[error("generation qualification phase policy encoding is noncanonical")]
    NonCanonicalEncoding,
    /// Policy JSON exceeded its fixed byte ceiling.
    #[error("generation qualification phase policy limit was exceeded")]
    LimitExceeded,
    /// Resource facts differed from the exact reviewed V1 contract.
    #[error("generation qualification resource policy binding is invalid")]
    InvalidResourceBinding,
    /// Human-adjudication facts differed from the exact reviewed V1 contract.
    #[error("generation qualification human adjudication policy binding is invalid")]
    InvalidHumanAdjudicationBinding,
    /// The operation policy named another domain-separated phase policy digest.
    #[error("generation qualification phase policy operation binding is invalid")]
    OperationPolicyMismatch,
}

const fn disposition(approved: bool) -> GenerationQualificationPhasePolicySourceDisposition {
    if approved {
        GenerationQualificationPhasePolicySourceDisposition::Approved
    } else {
        GenerationQualificationPhasePolicySourceDisposition::Denied
    }
}

#[cfg(test)]
mod tests;
