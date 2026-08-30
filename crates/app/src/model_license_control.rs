use std::fmt;

use rewrite_types::{CancellationToken, Digest};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use rewrite_model::ModelLicenseControlId;

use crate::{
    ManagedOllamaModelPackageError, ModelPackageFoundationId,
    VerifiedManagedOllamaModelPackageLease,
};

mod verification;
mod wire;

/// Fixed model-license reviewer procedure identity.
pub const MODEL_LICENSE_REVIEW_PROCEDURE_ID: &str = "retonr:model-license-review:procedure";
/// Fixed model-license reviewer procedure version.
pub const MODEL_LICENSE_REVIEW_PROCEDURE_VERSION: u32 = 1;
/// Current canonical model-license reviewer schema.
pub const MODEL_LICENSE_REVIEW_SCHEMA_VERSION: u32 = 1;
/// Current canonical portable model-license control schema.
pub const MODEL_LICENSE_CONTROL_SCHEMA_VERSION: u32 = 1;
/// Fixed portable model-license control procedure identity.
pub const MODEL_LICENSE_CONTROL_PROCEDURE_ID: &str = "retonr:model-license-control:procedure";
/// Fixed portable model-license control procedure version.
pub const MODEL_LICENSE_CONTROL_PROCEDURE_VERSION: u32 = 1;
/// Hard ceiling for one canonical model-license reviewer record.
pub const MAX_MODEL_LICENSE_REVIEW_JSON_BYTES: usize = 64 * 1_024;
/// Hard ceiling for one canonical portable model-license control.
pub const MAX_MODEL_LICENSE_CONTROL_JSON_BYTES: usize = 128 * 1_024;

const CONTROL_ID_DOMAIN: &[u8] = b"retonr:model-license-control:v1\0";
const PRODUCTION_MODEL_LICENSE_APPROVALS: &[(&str, ModelLicensePermission)] = &[];

/// Exact permission reviewed for one model package.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelLicensePermission {
    /// Local model generation without publishing the retained package.
    LocalGeneration,
    /// Redistribution of the exact retained model package.
    PackageRedistribution,
}

/// Inert result of checking one structural control against the production root.
///
/// This value records only repository policy membership. It carries no launch,
/// traffic, generation, or qualification authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelLicenseControlAssessmentDisposition {
    /// The exact structural control and permission are present in the production root.
    Approved,
    /// The exact structural control and permission are absent from the production root.
    Denied,
}

/// Repository-owned offline trust root for live model-license approval.
///
/// Production construction is deliberately empty and exposes no mutation or
/// deserialization surface. Adding an approval requires a reviewed source
/// change to this repository-owned policy definition.
pub struct ProductionModelLicenseApprovalPolicy {
    approvals: Vec<(ModelLicenseControlId, ModelLicensePermission)>,
}

impl ProductionModelLicenseApprovalPolicy {
    /// Returns the fail-closed production policy with no approved controls.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            approvals: Vec::new(),
        }
    }

    /// Returns the number of exact control-permission pairs in this trust root.
    #[must_use]
    pub const fn approved_control_count(&self) -> usize {
        PRODUCTION_MODEL_LICENSE_APPROVALS.len() + self.approvals.len()
    }

    fn permits(
        &self,
        control_id: &ModelLicenseControlId,
        permission: ModelLicensePermission,
    ) -> bool {
        PRODUCTION_MODEL_LICENSE_APPROVALS
            .iter()
            .any(|(digest, approved_permission)| {
                *digest == control_id.digest().as_str() && *approved_permission == permission
            })
            || self.approvals.iter().any(|(id, approved_permission)| {
                id == control_id && *approved_permission == permission
            })
    }

    /// Checks production approval without minting launch authority.
    ///
    /// The structural proof is freshly revalidated against the selected live lease
    /// before the immutable production root is consulted. The returned disposition
    /// is inert and cannot be used where an approved license authority is required.
    ///
    /// # Errors
    ///
    /// Returns [`ModelLicenseControlError`] for cancellation, retained-state drift,
    /// lease substitution, installation-generation drift, or permission mismatch.
    pub fn assess<'lease>(
        &self,
        verified: &VerifiedModelLicenseControl<'lease>,
        selected_lease: &'lease VerifiedManagedOllamaModelPackageLease,
        intended_permission: ModelLicensePermission,
        cancellation: &CancellationToken,
    ) -> Result<ModelLicenseControlAssessmentDisposition, ModelLicenseControlError> {
        verification::assess(
            verified,
            selected_lease,
            intended_permission,
            self,
            cancellation,
        )
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn exact_test_policy(
        control_id: ModelLicenseControlId,
        permission: ModelLicensePermission,
    ) -> Self {
        Self {
            approvals: vec![(control_id, permission)],
        }
    }
}

impl Default for ProductionModelLicenseApprovalPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ProductionModelLicenseApprovalPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionModelLicenseApprovalPolicy")
            .field("approved_control_count", &self.approved_control_count())
            .finish_non_exhaustive()
    }
}

/// Inert canonical publication material for one approved model-license review.
///
/// This value is portable across byte-identical reinstallations. It contains no
/// live lease or launch authority.
#[derive(Clone, Eq, PartialEq)]
pub struct CompiledModelLicenseControl {
    canonical_bytes: Vec<u8>,
    control_id: ModelLicenseControlId,
    foundation_id: ModelPackageFoundationId,
    permission: ModelLicensePermission,
    review_evidence_digest: Digest,
    license_member_count: usize,
}

impl CompiledModelLicenseControl {
    /// Returns the exact canonical publication bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the domain-separated control identity.
    #[must_use]
    pub const fn control_id(&self) -> &ModelLicenseControlId {
        &self.control_id
    }

    /// Returns the exact stable model foundation identity.
    #[must_use]
    pub const fn foundation_id(&self) -> &ModelPackageFoundationId {
        &self.foundation_id
    }

    /// Returns the exact permission reviewed by this portable control.
    #[must_use]
    pub const fn permission(&self) -> ModelLicensePermission {
        self.permission
    }

    /// Returns the internally derived digest of the canonical reviewer bytes.
    #[must_use]
    pub const fn review_evidence_digest(&self) -> &Digest {
        &self.review_evidence_digest
    }

    /// Returns the number of independently reviewed logical license paths.
    #[must_use]
    pub const fn license_member_count(&self) -> usize {
        self.license_member_count
    }
}

impl fmt::Debug for CompiledModelLicenseControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompiledModelLicenseControl")
            .field("control_id", &self.control_id)
            .field("foundation_id", &self.foundation_id)
            .field("license_member_count", &self.license_member_count)
            .finish_non_exhaustive()
    }
}

/// Live, nonserializable structural proof for one exact model-license control.
///
/// This value proves that canonical control bytes exactly reconstruct against
/// one live retained model-package lease. It does not carry production approval
/// and cannot authorize model traffic or launch.
///
/// The proof cannot outlive the specialized retained model lease. It is not
/// cloneable and has no public raw constructor.
///
/// ```compile_fail
/// use rewrite_app::VerifiedModelLicenseControl;
///
/// fn clone_authority(value: &VerifiedModelLicenseControl<'_>) {
///     let _forged: VerifiedModelLicenseControl<'_> = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedModelLicenseControl;
///
/// fn serialize_authority(value: &VerifiedModelLicenseControl<'_>) {
///     let _bytes = serde_json::to_vec(value).expect("proof must not serialize");
/// }
/// ```
pub struct VerifiedModelLicenseControl<'lease> {
    lease: &'lease VerifiedManagedOllamaModelPackageLease,
    control_id: ModelLicenseControlId,
    foundation_id: ModelPackageFoundationId,
    permission: ModelLicensePermission,
    installation_generation: u64,
    license_member_count: usize,
}

impl<'lease> VerifiedModelLicenseControl<'lease> {
    /// Returns the independently verified portable control identity.
    #[must_use]
    pub const fn control_id(&self) -> &ModelLicenseControlId {
        &self.control_id
    }

    /// Returns the stable verified foundation identity.
    #[must_use]
    pub const fn foundation_id(&self) -> &ModelPackageFoundationId {
        &self.foundation_id
    }

    /// Returns the exact structurally verified permission.
    #[must_use]
    pub const fn permission(&self) -> ModelLicensePermission {
        self.permission
    }

    /// Returns the exact positive installation generation bound to this proof.
    #[must_use]
    pub const fn installation_generation(&self) -> u64 {
        self.installation_generation
    }

    /// Returns the number of independently reviewed logical license paths.
    #[must_use]
    pub const fn license_member_count(&self) -> usize {
        self.license_member_count
    }

    /// Revalidates this structural proof against the exact selected live lease.
    ///
    /// This operation does not add production approval or launch authority.
    ///
    /// # Errors
    ///
    /// Returns [`ModelLicenseControlError`] for cancellation, retained-state
    /// drift, another live lease, installation-generation drift, or permission
    /// substitution.
    pub fn revalidate(
        &self,
        selected_lease: &'lease VerifiedManagedOllamaModelPackageLease,
        intended_permission: ModelLicensePermission,
        cancellation: &CancellationToken,
    ) -> Result<(), ModelLicenseControlError> {
        verification::revalidate_structural(self, selected_lease, intended_permission, cancellation)
    }
}

impl fmt::Debug for VerifiedModelLicenseControl<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedModelLicenseControl")
            .field("control_id", &self.control_id)
            .field("foundation_id", &self.foundation_id)
            .field("license_member_count", &self.license_member_count)
            .finish_non_exhaustive()
    }
}

/// Live, nonserializable production approval authority for one exact control.
///
/// This is the only model-license control value accepted by launch integration.
/// It can only be obtained by promoting a live structural proof through the
/// repository-owned production approval policy.
///
/// ```compile_fail
/// use rewrite_app::VerifiedApprovedModelLicenseControl;
///
/// fn clone_authority(value: &VerifiedApprovedModelLicenseControl<'_>) {
///     let _forged: VerifiedApprovedModelLicenseControl<'_> = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedApprovedModelLicenseControl;
///
/// fn serialize_authority(value: &VerifiedApprovedModelLicenseControl<'_>) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedApprovedModelLicenseControl<'lease> {
    verified: VerifiedModelLicenseControl<'lease>,
}

impl<'lease> VerifiedApprovedModelLicenseControl<'lease> {
    /// Returns the independently verified portable control identity.
    #[must_use]
    pub const fn control_id(&self) -> &ModelLicenseControlId {
        self.verified.control_id()
    }

    /// Returns the stable verified foundation identity.
    #[must_use]
    pub const fn foundation_id(&self) -> &ModelPackageFoundationId {
        self.verified.foundation_id()
    }

    /// Returns the exact approved permission.
    #[must_use]
    pub const fn permission(&self) -> ModelLicensePermission {
        self.verified.permission()
    }

    /// Returns the exact positive installation generation bound to this authority.
    #[must_use]
    pub const fn installation_generation(&self) -> u64 {
        self.verified.installation_generation()
    }

    /// Returns the number of independently reviewed logical license paths.
    #[must_use]
    pub const fn license_member_count(&self) -> usize {
        self.verified.license_member_count()
    }

    pub(crate) fn binds_exact_package_lease(
        &self,
        selected: &VerifiedManagedOllamaModelPackageLease,
    ) -> bool {
        std::ptr::eq(self.verified.lease, selected)
    }

    /// Revalidates this authority against the exact live lease selected for use.
    ///
    /// Launch integration must call this immediately before consuming model input.
    /// The selected lease, installation generation, foundation, control permission,
    /// and all retained bytes are checked again.
    /// The selected foundation is fully revalidated once at this use boundary.
    /// Compile and portable-control verification retain their separate before and
    /// after revalidation brackets because those operations perform intervening
    /// derivation work.
    ///
    /// # Errors
    ///
    /// Returns [`ModelLicenseControlError`] for cancellation, retained-state
    /// drift, another live lease, installation-generation drift, or permission
    /// substitution.
    pub fn revalidate_for_use(
        &self,
        selected_lease: &'lease VerifiedManagedOllamaModelPackageLease,
        intended_permission: ModelLicensePermission,
        cancellation: &CancellationToken,
    ) -> Result<(), ModelLicenseControlError> {
        verification::revalidate_structural(
            &self.verified,
            selected_lease,
            intended_permission,
            cancellation,
        )
    }
}

impl fmt::Debug for VerifiedApprovedModelLicenseControl<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedApprovedModelLicenseControl")
            .field("control_id", self.control_id())
            .field("foundation_id", self.foundation_id())
            .field("license_member_count", &self.license_member_count())
            .finish_non_exhaustive()
    }
}

/// Failure while compiling, verifying, or using model-license control evidence.
#[derive(Debug, Error)]
pub enum ModelLicenseControlError {
    /// The live specialized model-package lease failed revalidation.
    #[error("managed model package revalidation failed")]
    Lease(#[source] ManagedOllamaModelPackageError),
    /// Work was cancelled before a stable result could be derived.
    #[error("model-license control operation was cancelled")]
    Cancelled,
    /// Canonical control or reviewer JSON was malformed or noncanonical.
    #[error("model-license control encoding is invalid")]
    InvalidEncoding,
    /// Reviewer or control input exceeded its fixed ceiling.
    #[error("model-license control limit was exceeded")]
    LimitExceeded,
    /// Reviewer facts or the approval decision do not match the live foundation.
    #[error("model-license review does not approve the exact live foundation")]
    ReviewRequired,
    /// Portable control evidence does not match its nested review or live foundation.
    #[error("model-license control binding is invalid")]
    InvalidBinding,
    /// The requested permission differs from the reviewed permission.
    #[error("model-license permission does not match")]
    PermissionMismatch,
    /// Authority use selected another installation generation.
    #[error("model-license authority installation generation does not match")]
    InstallationGenerationMismatch,
    /// Authority use selected another live specialized package lease.
    #[error("model-license authority lease does not match")]
    LeaseMismatch,
    /// The repository-owned production trust root does not approve this control.
    #[error("model-license control is not approved by production policy")]
    ApprovalPolicyDenied,
}

/// Compiler for inert canonical model-license publication material.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModelLicenseControlCompiler;

impl ModelLicenseControlCompiler {
    /// Compiles one exact canonical reviewer record against a live model lease.
    ///
    /// The lease is fully revalidated before derivation and after canonical
    /// compilation. Reviewer evidence bytes are hashed internally.
    ///
    /// # Errors
    ///
    /// Returns [`ModelLicenseControlError`] for cancellation, invalid canonical
    /// JSON, an unsupported or mismatched review, or live retained-state drift.
    pub fn compile(
        lease: &VerifiedManagedOllamaModelPackageLease,
        intended_permission: ModelLicensePermission,
        reviewer_json: &[u8],
        cancellation: &CancellationToken,
    ) -> Result<CompiledModelLicenseControl, ModelLicenseControlError> {
        verification::compile(lease, intended_permission, reviewer_json, cancellation)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn reviewer_json_for_test(
        lease: &VerifiedManagedOllamaModelPackageLease,
        intended_permission: ModelLicensePermission,
    ) -> Vec<u8> {
        verification::reviewer_json_for_test(lease, intended_permission)
    }
}

/// Independent verifier for portable model-license controls.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModelLicenseControlVerifier;

impl ModelLicenseControlVerifier {
    /// Verifies one self-contained control structurally against a live lease.
    ///
    /// The returned proof is tied to `lease` and its exact installation
    /// generation. It does not carry production approval and is not accepted by
    /// launch integration.
    ///
    /// # Errors
    ///
    /// Returns [`ModelLicenseControlError`] for cancellation, malformed or
    /// drifting evidence, permission substitution, or retained-state drift.
    pub fn verify_structural<'lease>(
        control_json: &[u8],
        lease: &'lease VerifiedManagedOllamaModelPackageLease,
        intended_permission: ModelLicensePermission,
        cancellation: &CancellationToken,
    ) -> Result<VerifiedModelLicenseControl<'lease>, ModelLicenseControlError> {
        verification::verify_structural(control_json, lease, intended_permission, cancellation)
    }

    /// Verifies one self-contained control against a live model lease.
    ///
    /// This compatibility entry point performs structural verification and then
    /// explicit production-policy promotion. Preflight assessment code that must
    /// retain structurally valid denied evidence should call
    /// [`Self::verify_structural`] and promote it separately.
    ///
    /// The returned authority is tied to `lease` and its exact installation
    /// generation. Verification revalidates retained state before derivation and
    /// after comparing the independently reconstructed canonical control.
    ///
    /// # Errors
    ///
    /// Returns [`ModelLicenseControlError`] for cancellation, malformed or
    /// drifting evidence, permission substitution, retained-state drift, or
    /// production policy denial.
    pub fn verify<'lease>(
        control_json: &[u8],
        lease: &'lease VerifiedManagedOllamaModelPackageLease,
        intended_permission: ModelLicensePermission,
        approval_policy: &ProductionModelLicenseApprovalPolicy,
        cancellation: &CancellationToken,
    ) -> Result<VerifiedApprovedModelLicenseControl<'lease>, ModelLicenseControlError> {
        let verified =
            Self::verify_structural(control_json, lease, intended_permission, cancellation)?;
        ModelLicenseControlApprovalPromoter::promote(verified, approval_policy, cancellation)
    }
}

/// Explicit production-approval boundary for a structural license proof.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModelLicenseControlApprovalPromoter;

impl ModelLicenseControlApprovalPromoter {
    /// Promotes one exact structural proof through the production trust root.
    ///
    /// The structural proof is revalidated against its live lease before policy
    /// is consulted. A denied proof remains structurally valid but grants no
    /// launch authority.
    ///
    /// # Errors
    ///
    /// Returns [`ModelLicenseControlError`] for cancellation, retained-state
    /// drift, structural substitution, or production policy denial.
    pub fn promote<'lease>(
        verified: VerifiedModelLicenseControl<'lease>,
        approval_policy: &ProductionModelLicenseApprovalPolicy,
        cancellation: &CancellationToken,
    ) -> Result<VerifiedApprovedModelLicenseControl<'lease>, ModelLicenseControlError> {
        verification::promote(verified, approval_policy, cancellation)
    }
}

fn control_id(bytes: &[u8]) -> ModelLicenseControlId {
    let mut material = Vec::with_capacity(CONTROL_ID_DOMAIN.len() + bytes.len());
    material.extend_from_slice(CONTROL_ID_DOMAIN);
    material.extend_from_slice(bytes);
    ModelLicenseControlId::from_derived_digest(Digest::sha256(&material))
}

#[cfg(test)]
#[path = "model_license_control/tests.rs"]
mod tests;
