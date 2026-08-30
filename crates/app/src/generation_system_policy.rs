//! Canonical reviewed policy authority for generation-system construction.

use std::fmt;

use rewrite_types::Digest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod verification;
mod wire;

/// Current canonical generation-system policy review schema.
pub const GENERATION_SYSTEM_POLICY_REVIEW_SCHEMA_VERSION: u32 = 1;
/// Current canonical generation-system policy control schema.
pub const GENERATION_SYSTEM_POLICY_CONTROL_SCHEMA_VERSION: u32 = 1;
/// Fixed independent review procedure identity.
pub const GENERATION_SYSTEM_POLICY_REVIEW_PROCEDURE_ID: &str =
    "retonr:generation-system-policy-review:procedure";
/// Fixed independent review procedure version.
pub const GENERATION_SYSTEM_POLICY_REVIEW_PROCEDURE_VERSION: u32 = 1;
/// Fixed portable control procedure identity.
pub const GENERATION_SYSTEM_POLICY_CONTROL_PROCEDURE_ID: &str =
    "retonr:generation-system-policy-control:procedure";
/// Fixed portable control procedure version.
pub const GENERATION_SYSTEM_POLICY_CONTROL_PROCEDURE_VERSION: u32 = 1;
/// Hard ceiling for one canonical review record.
pub const MAX_GENERATION_SYSTEM_POLICY_REVIEW_JSON_BYTES: usize = 16 * 1_024;
/// Hard ceiling for one canonical control record.
pub const MAX_GENERATION_SYSTEM_POLICY_CONTROL_JSON_BYTES: usize = 32 * 1_024;

const CONTROL_ID_DOMAIN: &[u8] = b"retonr:generation-system-policy-control:v1\0";
const PRODUCTION_APPROVALS: &[(
    &str,
    GenerationSystemPolicyPermission,
    GenerationSystemPolicyPurpose,
)] = &[];

/// Exact permission granted by a reviewed generation-system policy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationSystemPolicyPermission {
    /// Construct one inert generation-system record from exact typed relations.
    ConstructGenerationSystem,
    /// Revalidate an existing inert generation-system record without constructing one.
    ValidateGenerationSystem,
}

/// Closed purpose for which a generation-system policy was reviewed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationSystemPolicyPurpose {
    /// Candidate generation inside the managed qualification workflow.
    ManagedCandidateGeneration,
    /// Judge generation inside a separately managed evaluation workflow.
    ManagedJudgeGeneration,
}

/// Exact inert digest bindings reviewed as one generation-system policy.
///
/// These are equality bindings only. They do not prove semantic quality,
/// implementation correctness, hardware behavior, or qualification.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "field names intentionally match the portable GenerationSystemRecordV1 contract"
)]
pub struct GenerationSystemPolicyBindingsV1 {
    strategy_digest: Digest,
    planner_digest: Digest,
    validator_digest: Digest,
    adapter_digest: Digest,
    prompt_digest: Digest,
    output_schema_digest: Digest,
    request_policy_digest: Digest,
    language_digest: Digest,
    mode_digest: Digest,
    format_digest: Digest,
    operating_system_digest: Digest,
    architecture_digest: Digest,
    execution_class_digest: Digest,
    hardware_envelope_digest: Digest,
}

/// Named inputs for one complete inert generation-system policy binding set.
///
/// The named fields prevent silent transposition among otherwise identical
/// digest types at this security-sensitive construction boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationSystemPolicyBindingsV1Input {
    /// Candidate-generation strategy digest.
    pub strategy_digest: Digest,
    /// Planner implementation and policy digest.
    pub planner_digest: Digest,
    /// Output-validator digest.
    pub validator_digest: Digest,
    /// Provider-adapter digest.
    pub adapter_digest: Digest,
    /// Prompt-construction contract digest.
    pub prompt_digest: Digest,
    /// Structured output-schema digest.
    pub output_schema_digest: Digest,
    /// Provider-neutral request-policy digest.
    pub request_policy_digest: Digest,
    /// Language-contract digest.
    pub language_digest: Digest,
    /// Generation-mode digest.
    pub mode_digest: Digest,
    /// Output-format digest.
    pub format_digest: Digest,
    /// Operating-system class digest.
    pub operating_system_digest: Digest,
    /// Architecture class digest.
    pub architecture_digest: Digest,
    /// Execution-class digest.
    pub execution_class_digest: Digest,
    /// Hardware-envelope digest.
    pub hardware_envelope_digest: Digest,
}

impl GenerationSystemPolicyBindingsV1 {
    /// Constructs inert review material from the complete closed digest set.
    #[must_use]
    pub fn new(input: GenerationSystemPolicyBindingsV1Input) -> Self {
        Self {
            strategy_digest: input.strategy_digest,
            planner_digest: input.planner_digest,
            validator_digest: input.validator_digest,
            adapter_digest: input.adapter_digest,
            prompt_digest: input.prompt_digest,
            output_schema_digest: input.output_schema_digest,
            request_policy_digest: input.request_policy_digest,
            language_digest: input.language_digest,
            mode_digest: input.mode_digest,
            format_digest: input.format_digest,
            operating_system_digest: input.operating_system_digest,
            architecture_digest: input.architecture_digest,
            execution_class_digest: input.execution_class_digest,
            hardware_envelope_digest: input.hardware_envelope_digest,
        }
    }

    /// Returns the candidate-generation strategy digest.
    #[must_use]
    pub fn strategy_digest(&self) -> &Digest {
        &self.strategy_digest
    }
    /// Returns the planner implementation and policy digest.
    #[must_use]
    pub fn planner_digest(&self) -> &Digest {
        &self.planner_digest
    }
    /// Returns the output-validator digest.
    #[must_use]
    pub fn validator_digest(&self) -> &Digest {
        &self.validator_digest
    }
    /// Returns the provider-adapter digest.
    #[must_use]
    pub fn adapter_digest(&self) -> &Digest {
        &self.adapter_digest
    }
    /// Returns the prompt-construction contract digest.
    #[must_use]
    pub fn prompt_digest(&self) -> &Digest {
        &self.prompt_digest
    }
    /// Returns the structured output-schema digest.
    #[must_use]
    pub fn output_schema_digest(&self) -> &Digest {
        &self.output_schema_digest
    }
    /// Returns the provider-neutral request-policy digest.
    #[must_use]
    pub fn request_policy_digest(&self) -> &Digest {
        &self.request_policy_digest
    }
    /// Returns the language-contract digest.
    #[must_use]
    pub fn language_digest(&self) -> &Digest {
        &self.language_digest
    }
    /// Returns the generation-mode digest.
    #[must_use]
    pub fn mode_digest(&self) -> &Digest {
        &self.mode_digest
    }
    /// Returns the output-format digest.
    #[must_use]
    pub fn format_digest(&self) -> &Digest {
        &self.format_digest
    }
    /// Returns the operating-system class digest.
    #[must_use]
    pub fn operating_system_digest(&self) -> &Digest {
        &self.operating_system_digest
    }
    /// Returns the architecture class digest.
    #[must_use]
    pub fn architecture_digest(&self) -> &Digest {
        &self.architecture_digest
    }
    /// Returns the execution-class digest.
    #[must_use]
    pub fn execution_class_digest(&self) -> &Digest {
        &self.execution_class_digest
    }
    /// Returns the hardware-envelope digest.
    #[must_use]
    pub fn hardware_envelope_digest(&self) -> &Digest {
        &self.hardware_envelope_digest
    }
}

/// Domain-separated identity of one canonical reviewed policy control.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct GenerationSystemPolicyControlId(Digest);

impl GenerationSystemPolicyControlId {
    /// Returns the digest defining this control identity.
    #[must_use]
    pub fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Repository-owned immutable trust root for generation-system policy controls.
///
/// Production construction is fail-closed and exposes no mutation or decoding
/// surface. An approval requires a reviewed source change to this repository.
pub struct ProductionGenerationSystemPolicyApproval {
    approvals: Vec<(
        GenerationSystemPolicyControlId,
        GenerationSystemPolicyPermission,
        GenerationSystemPolicyPurpose,
    )>,
}

impl ProductionGenerationSystemPolicyApproval {
    /// Returns the empty production approval policy.
    #[must_use]
    pub fn new() -> Self {
        Self {
            approvals: Vec::new(),
        }
    }

    /// Returns the number of exact control, permission, and purpose approvals.
    #[must_use]
    pub fn approved_control_count(&self) -> usize {
        PRODUCTION_APPROVALS.len() + self.approvals.len()
    }

    fn permits(
        &self,
        id: &GenerationSystemPolicyControlId,
        permission: GenerationSystemPolicyPermission,
        purpose: GenerationSystemPolicyPurpose,
    ) -> bool {
        PRODUCTION_APPROVALS
            .iter()
            .any(|(digest, approved_permission, approved_purpose)| {
                *digest == id.digest().as_str()
                    && *approved_permission == permission
                    && *approved_purpose == purpose
            })
            || self
                .approvals
                .iter()
                .any(|(approved_id, approved_permission, approved_purpose)| {
                    approved_id == id
                        && *approved_permission == permission
                        && *approved_purpose == purpose
                })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn exact_test_policy(
        id: GenerationSystemPolicyControlId,
        permission: GenerationSystemPolicyPermission,
        purpose: GenerationSystemPolicyPurpose,
    ) -> Self {
        Self {
            approvals: vec![(id, permission, purpose)],
        }
    }
}

impl Default for ProductionGenerationSystemPolicyApproval {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ProductionGenerationSystemPolicyApproval {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionGenerationSystemPolicyApproval")
            .field("approved_control_count", &self.approved_control_count())
            .finish_non_exhaustive()
    }
}

/// Inert canonical publication material for one reviewed policy.
#[derive(Clone, Eq, PartialEq)]
pub struct CompiledGenerationSystemPolicyControl {
    canonical_bytes: Vec<u8>,
    control_id: GenerationSystemPolicyControlId,
    permission: GenerationSystemPolicyPermission,
    purpose: GenerationSystemPolicyPurpose,
    review_evidence_digest: Digest,
    bindings: GenerationSystemPolicyBindingsV1,
}

impl CompiledGenerationSystemPolicyControl {
    /// Returns the exact canonical publication bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Returns the content-derived portable control identity.
    #[must_use]
    pub fn control_id(&self) -> &GenerationSystemPolicyControlId {
        &self.control_id
    }
    /// Returns the reviewed permission.
    #[must_use]
    pub fn permission(&self) -> GenerationSystemPolicyPermission {
        self.permission
    }
    /// Returns the reviewed purpose.
    #[must_use]
    pub fn purpose(&self) -> GenerationSystemPolicyPurpose {
        self.purpose
    }
    /// Returns the internally derived digest of the canonical review bytes.
    #[must_use]
    pub fn review_evidence_digest(&self) -> &Digest {
        &self.review_evidence_digest
    }
    /// Returns the complete inert reviewed digest set.
    #[must_use]
    pub fn bindings(&self) -> &GenerationSystemPolicyBindingsV1 {
        &self.bindings
    }
}

impl fmt::Debug for CompiledGenerationSystemPolicyControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompiledGenerationSystemPolicyControl")
            .field("control_id", &self.control_id)
            .field("permission", &self.permission)
            .field("purpose", &self.purpose)
            .finish_non_exhaustive()
    }
}

/// Noncloneable authority for one exact production-approved policy control.
///
/// This authority approves only equality to a repository-reviewed control. It
/// does not claim legal approval, semantic correctness, runtime behavior, or
/// qualification.
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationSystemPolicy;
///
/// fn clone_authority(value: &VerifiedGenerationSystemPolicy) {
///     let _forged: VerifiedGenerationSystemPolicy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationSystemPolicy;
///
/// fn serialize_authority(value: &VerifiedGenerationSystemPolicy) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedGenerationSystemPolicy {
    control_id: GenerationSystemPolicyControlId,
    permission: GenerationSystemPolicyPermission,
    purpose: GenerationSystemPolicyPurpose,
    bindings: GenerationSystemPolicyBindingsV1,
}

impl VerifiedGenerationSystemPolicy {
    /// Returns the exact approved portable control identity.
    #[must_use]
    pub fn control_id(&self) -> &GenerationSystemPolicyControlId {
        &self.control_id
    }
    /// Returns the exact approved permission.
    #[must_use]
    pub fn permission(&self) -> GenerationSystemPolicyPermission {
        self.permission
    }
    /// Returns the exact approved purpose.
    #[must_use]
    pub fn purpose(&self) -> GenerationSystemPolicyPurpose {
        self.purpose
    }
    /// Returns only the digest bindings needed for generation-system input.
    #[must_use]
    pub fn bindings(&self) -> &GenerationSystemPolicyBindingsV1 {
        &self.bindings
    }
}

impl fmt::Debug for VerifiedGenerationSystemPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationSystemPolicy")
            .field("control_id", &self.control_id)
            .field("permission", &self.permission)
            .field("purpose", &self.purpose)
            .finish_non_exhaustive()
    }
}

/// Compiler for inert canonical reviewed policy material.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenerationSystemPolicyCompiler;

impl GenerationSystemPolicyCompiler {
    /// Compiles one exact canonical independent review and binding set.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationSystemPolicyError`] for malformed, noncanonical,
    /// oversized, substituted, permission-mismatched, or purpose-mismatched input.
    pub fn compile(
        bindings: &GenerationSystemPolicyBindingsV1,
        permission: GenerationSystemPolicyPermission,
        purpose: GenerationSystemPolicyPurpose,
        reviewer_json: &[u8],
    ) -> Result<CompiledGenerationSystemPolicyControl, GenerationSystemPolicyError> {
        verification::compile(bindings, permission, purpose, reviewer_json)
    }
}

/// Independent verifier for portable generation-system policy controls.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenerationSystemPolicyVerifier;

impl GenerationSystemPolicyVerifier {
    /// Verifies one control against exact expected bindings and production policy.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationSystemPolicyError`] for malformed, unapproved, or
    /// substituted control, binding, permission, purpose, or review data.
    pub fn verify(
        control_json: &[u8],
        expected_bindings: &GenerationSystemPolicyBindingsV1,
        permission: GenerationSystemPolicyPermission,
        purpose: GenerationSystemPolicyPurpose,
        approval: &ProductionGenerationSystemPolicyApproval,
    ) -> Result<VerifiedGenerationSystemPolicy, GenerationSystemPolicyError> {
        verification::verify(
            control_json,
            expected_bindings,
            permission,
            purpose,
            approval,
        )
    }
}

fn control_id(bytes: &[u8]) -> GenerationSystemPolicyControlId {
    let mut material = Vec::with_capacity(CONTROL_ID_DOMAIN.len() + bytes.len());
    material.extend_from_slice(CONTROL_ID_DOMAIN);
    material.extend_from_slice(bytes);
    GenerationSystemPolicyControlId(Digest::sha256(&material))
}

/// Generation-system policy compilation or verification failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationSystemPolicyError {
    /// Review or control JSON was empty, malformed, noncanonical, or unknown.
    #[error("generation-system policy encoding is invalid")]
    InvalidEncoding,
    /// Review or control JSON exceeded its fixed byte ceiling.
    #[error("generation-system policy limit was exceeded")]
    LimitExceeded,
    /// The independent review does not exactly approve the supplied bindings.
    #[error("generation-system policy exact review is required")]
    ReviewRequired,
    /// Nested review, control, or expected digest bindings did not agree.
    #[error("generation-system policy binding is invalid")]
    InvalidBinding,
    /// Requested permission differs from the reviewed permission.
    #[error("generation-system policy permission does not match")]
    PermissionMismatch,
    /// Requested purpose differs from the reviewed purpose.
    #[error("generation-system policy purpose does not match")]
    PurposeMismatch,
    /// Repository-owned production policy does not approve the exact control.
    #[error("generation-system policy control is not approved")]
    ApprovalPolicyDenied,
}

#[cfg(test)]
#[path = "generation_system_policy/tests.rs"]
mod tests;
