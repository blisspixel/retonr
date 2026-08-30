//! Canonical app-owned assessment policies for generation qualification.

use std::fmt;

use rewrite_model::{
    GenerationQualificationLicenseAssessmentPolicyId, GenerationQualificationLicensePermissionV1,
    GenerationQualificationPlatformAssessmentPolicyId, ModelLicenseControlId, RuntimeTarget,
};
use rewrite_types::Digest;
use thiserror::Error;

#[path = "generation_qualification_assessment_policy/verification.rs"]
mod verification;
#[path = "generation_qualification_assessment_policy/wire.rs"]
mod wire;

#[cfg(feature = "test-support")]
pub(crate) use verification::{license_policy_json_for_test, platform_policy_json_for_test};

/// Current canonical qualification assessment-policy schema.
pub const GENERATION_QUALIFICATION_ASSESSMENT_POLICY_SCHEMA_VERSION: u32 = 1;
/// Fixed platform assessment procedure identity.
pub const GENERATION_QUALIFICATION_PLATFORM_ASSESSMENT_PROCEDURE_ID: &str =
    "retonr:generation-qualification-platform-assessment:procedure";
/// Fixed platform assessment procedure version.
pub const GENERATION_QUALIFICATION_PLATFORM_ASSESSMENT_PROCEDURE_VERSION: u32 = 1;
/// Fixed model-license assessment procedure identity.
pub const GENERATION_QUALIFICATION_LICENSE_ASSESSMENT_PROCEDURE_ID: &str =
    "retonr:generation-qualification-license-assessment:procedure";
/// Fixed model-license assessment procedure version.
pub const GENERATION_QUALIFICATION_LICENSE_ASSESSMENT_PROCEDURE_VERSION: u32 = 1;
/// Hard ceiling for one canonical assessment policy.
pub const MAX_GENERATION_QUALIFICATION_ASSESSMENT_POLICY_JSON_BYTES: usize = 64 * 1_024;

const PRODUCTION_PLATFORM_POLICY_APPROVALS: &[&str] = &[];
const PRODUCTION_LICENSE_POLICY_APPROVALS: &[&str] = &[];

/// Exact source-controlled approval result for a structurally verified policy.
///
/// A denied disposition is usable only to derive deterministic negative
/// evidence. It grants no traffic, launch, generation, or qualification authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationQualificationAssessmentPolicySourceDisposition {
    /// The exact policy identity is present in its repository-owned source root.
    Approved,
    /// The exact policy identity is absent from its repository-owned source root.
    Denied,
}

/// Exact dynamic bindings of the reviewed managed Linux native CPU profile.
#[derive(Clone, Eq, PartialEq)]
pub struct GenerationQualificationPlatformAssessmentPolicyV1Bindings {
    /// Expected native runtime target.
    pub runtime_target: RuntimeTarget,
    /// Expected operating-system class digest.
    pub operating_system_digest: Digest,
    /// Expected architecture class digest.
    pub architecture_digest: Digest,
    /// Expected execution-class digest.
    pub execution_class_digest: Digest,
    /// Expected hardware-envelope digest.
    pub hardware_envelope_digest: Digest,
}

/// Repository-owned source root for platform assessment policies.
///
/// Production construction is fail-closed and exposes no mutation or decoding
/// surface. Adding an approval requires a reviewed source change.
pub struct ProductionGenerationQualificationPlatformAssessmentPolicySource {
    approvals: Vec<GenerationQualificationPlatformAssessmentPolicyId>,
}

impl ProductionGenerationQualificationPlatformAssessmentPolicySource {
    /// Returns the empty production platform-policy source root.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            approvals: Vec::new(),
        }
    }

    /// Returns the number of exact approved platform-policy identities.
    #[must_use]
    pub const fn approved_policy_count(&self) -> usize {
        PRODUCTION_PLATFORM_POLICY_APPROVALS.len() + self.approvals.len()
    }

    fn disposition(
        &self,
        policy_id: &GenerationQualificationPlatformAssessmentPolicyId,
    ) -> GenerationQualificationAssessmentPolicySourceDisposition {
        let approved = PRODUCTION_PLATFORM_POLICY_APPROVALS
            .iter()
            .any(|digest| *digest == policy_id.digest().as_str())
            || self.approvals.iter().any(|approved| approved == policy_id);
        disposition(approved)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn exact_test_source(
        policy_id: GenerationQualificationPlatformAssessmentPolicyId,
    ) -> Self {
        Self {
            approvals: vec![policy_id],
        }
    }
}

impl Default for ProductionGenerationQualificationPlatformAssessmentPolicySource {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ProductionGenerationQualificationPlatformAssessmentPolicySource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionGenerationQualificationPlatformAssessmentPolicySource")
            .field("approved_policy_count", &self.approved_policy_count())
            .finish_non_exhaustive()
    }
}

/// Repository-owned source root for model-license assessment policies.
///
/// Production construction is fail-closed and exposes no mutation or decoding
/// surface. Adding an approval requires a reviewed source change.
pub struct ProductionGenerationQualificationLicenseAssessmentPolicySource {
    approvals: Vec<GenerationQualificationLicenseAssessmentPolicyId>,
}

impl ProductionGenerationQualificationLicenseAssessmentPolicySource {
    /// Returns the empty production license-policy source root.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            approvals: Vec::new(),
        }
    }

    /// Returns the number of exact approved license-policy identities.
    #[must_use]
    pub const fn approved_policy_count(&self) -> usize {
        PRODUCTION_LICENSE_POLICY_APPROVALS.len() + self.approvals.len()
    }

    fn disposition(
        &self,
        policy_id: &GenerationQualificationLicenseAssessmentPolicyId,
    ) -> GenerationQualificationAssessmentPolicySourceDisposition {
        let approved = PRODUCTION_LICENSE_POLICY_APPROVALS
            .iter()
            .any(|digest| *digest == policy_id.digest().as_str())
            || self.approvals.iter().any(|approved| approved == policy_id);
        disposition(approved)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn exact_test_source(
        policy_id: GenerationQualificationLicenseAssessmentPolicyId,
    ) -> Self {
        Self {
            approvals: vec![policy_id],
        }
    }
}

impl Default for ProductionGenerationQualificationLicenseAssessmentPolicySource {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ProductionGenerationQualificationLicenseAssessmentPolicySource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionGenerationQualificationLicenseAssessmentPolicySource")
            .field("approved_policy_count", &self.approved_policy_count())
            .finish_non_exhaustive()
    }
}

/// Noncloneable structurally verified platform assessment-policy state.
///
/// Only [`GenerationQualificationAssessmentPolicySourceDisposition::Approved`]
/// permits a later compiler to derive positive platform evidence.
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationPlatformAssessmentPolicy;
///
/// fn clone_authority(value: &VerifiedGenerationQualificationPlatformAssessmentPolicy) {
///     let _forged: VerifiedGenerationQualificationPlatformAssessmentPolicy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationPlatformAssessmentPolicy;
///
/// fn serialize_authority(value: &VerifiedGenerationQualificationPlatformAssessmentPolicy) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedGenerationQualificationPlatformAssessmentPolicy {
    policy_id: GenerationQualificationPlatformAssessmentPolicyId,
    source_disposition: GenerationQualificationAssessmentPolicySourceDisposition,
    bindings: GenerationQualificationPlatformAssessmentPolicyV1Bindings,
}

impl VerifiedGenerationQualificationPlatformAssessmentPolicy {
    /// Returns the typed identity derived after exact policy validation.
    #[must_use]
    pub const fn policy_id(&self) -> &GenerationQualificationPlatformAssessmentPolicyId {
        &self.policy_id
    }

    /// Returns the exact repository-owned source approval disposition.
    #[must_use]
    pub const fn source_disposition(
        &self,
    ) -> GenerationQualificationAssessmentPolicySourceDisposition {
        self.source_disposition
    }

    /// Returns the exact reviewed platform bindings.
    #[must_use]
    pub const fn bindings(&self) -> &GenerationQualificationPlatformAssessmentPolicyV1Bindings {
        &self.bindings
    }
}

impl fmt::Debug for VerifiedGenerationQualificationPlatformAssessmentPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationQualificationPlatformAssessmentPolicy")
            .field("source_disposition", &self.source_disposition)
            .finish_non_exhaustive()
    }
}

/// Noncloneable structurally verified model-license assessment-policy state.
///
/// Only [`GenerationQualificationAssessmentPolicySourceDisposition::Approved`]
/// permits a later compiler to derive positive license evidence.
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationLicenseAssessmentPolicy;
///
/// fn clone_authority(value: &VerifiedGenerationQualificationLicenseAssessmentPolicy) {
///     let _forged: VerifiedGenerationQualificationLicenseAssessmentPolicy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationQualificationLicenseAssessmentPolicy;
///
/// fn serialize_authority(value: &VerifiedGenerationQualificationLicenseAssessmentPolicy) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedGenerationQualificationLicenseAssessmentPolicy {
    policy_id: GenerationQualificationLicenseAssessmentPolicyId,
    source_disposition: GenerationQualificationAssessmentPolicySourceDisposition,
    model_license_control_id: ModelLicenseControlId,
    permission: GenerationQualificationLicensePermissionV1,
}

impl VerifiedGenerationQualificationLicenseAssessmentPolicy {
    /// Returns the typed identity derived after exact policy validation.
    #[must_use]
    pub const fn policy_id(&self) -> &GenerationQualificationLicenseAssessmentPolicyId {
        &self.policy_id
    }

    /// Returns the exact repository-owned source approval disposition.
    #[must_use]
    pub const fn source_disposition(
        &self,
    ) -> GenerationQualificationAssessmentPolicySourceDisposition {
        self.source_disposition
    }

    /// Returns the expected portable model-license control identity.
    #[must_use]
    pub const fn model_license_control_id(&self) -> &ModelLicenseControlId {
        &self.model_license_control_id
    }

    /// Returns the fixed reviewed V1 permission.
    #[must_use]
    pub const fn permission(&self) -> GenerationQualificationLicensePermissionV1 {
        self.permission
    }
}

impl fmt::Debug for VerifiedGenerationQualificationLicenseAssessmentPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationQualificationLicenseAssessmentPolicy")
            .field("source_disposition", &self.source_disposition)
            .finish_non_exhaustive()
    }
}

/// Independent verifier for canonical qualification assessment policies.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenerationQualificationAssessmentPolicyVerifier;

impl GenerationQualificationAssessmentPolicyVerifier {
    /// Verifies one platform policy against exact expected bindings and source root.
    ///
    /// Source denial is returned as a verified denied disposition, not an error.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationAssessmentPolicyError`] for malformed,
    /// noncanonical, oversized, unsupported, or substituted input.
    pub fn verify_platform(
        policy_json: &[u8],
        expected_bindings: &GenerationQualificationPlatformAssessmentPolicyV1Bindings,
        source: &ProductionGenerationQualificationPlatformAssessmentPolicySource,
    ) -> Result<
        VerifiedGenerationQualificationPlatformAssessmentPolicy,
        GenerationQualificationAssessmentPolicyError,
    > {
        verification::verify_platform(policy_json, expected_bindings, source)
    }

    /// Verifies one license policy against an expected control and source root.
    ///
    /// Source denial is returned as a verified denied disposition, not an error.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationAssessmentPolicyError`] for malformed,
    /// noncanonical, oversized, unsupported, or substituted input.
    pub fn verify_license(
        policy_json: &[u8],
        expected_control_id: &ModelLicenseControlId,
        source: &ProductionGenerationQualificationLicenseAssessmentPolicySource,
    ) -> Result<
        VerifiedGenerationQualificationLicenseAssessmentPolicy,
        GenerationQualificationAssessmentPolicyError,
    > {
        verification::verify_license(policy_json, expected_control_id, source)
    }
}

/// Qualification assessment-policy verification failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationAssessmentPolicyError {
    /// Policy JSON was empty, malformed, or contained an unknown value or field.
    #[error("generation qualification assessment policy encoding is invalid")]
    InvalidEncoding,
    /// Policy JSON was valid but not its exact compact canonical encoding.
    #[error("generation qualification assessment policy encoding is noncanonical")]
    NonCanonicalEncoding,
    /// Policy JSON exceeded its fixed byte ceiling.
    #[error("generation qualification assessment policy limit was exceeded")]
    LimitExceeded,
    /// Platform policy facts differed from the exact reviewed V1 profile or bindings.
    #[error("generation qualification platform assessment policy binding is invalid")]
    InvalidPlatformBinding,
    /// License policy facts differed from the exact reviewed V1 control or permission.
    #[error("generation qualification license assessment policy binding is invalid")]
    InvalidLicenseBinding,
}

const fn disposition(approved: bool) -> GenerationQualificationAssessmentPolicySourceDisposition {
    if approved {
        GenerationQualificationAssessmentPolicySourceDisposition::Approved
    } else {
        GenerationQualificationAssessmentPolicySourceDisposition::Denied
    }
}

#[cfg(test)]
#[path = "generation_qualification_assessment_policy/tests.rs"]
mod tests;
