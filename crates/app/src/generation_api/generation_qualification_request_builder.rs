//! Deterministic one-at-a-time request construction for generation qualification.

use std::fmt;

use super::{GenerationCaseSourceLease, GenerationCaseSourceLeaseError};
use crate::{ArtifactInventoryError, VerifiedGenerationSystemPolicy};
use rewrite_inference::{GenerationRequest, StructuredCompletionRequest};
use rewrite_model::{
    CandidateOutputCeilingsV1, GenerationCaseManifestV1, GenerationCaseRequestProfileV1,
    GenerationClusterRecordV1, GenerationDeterministicCaseContractV1,
    GenerationQualificationOperationLimitsV1, GenerationQualificationOperationPolicyV1,
    GenerationQualificationPlanV1, GenerationQualificationRequestProjectionEntryV1Input,
    GenerationRepetitionRecordV1, GenerationRequestBindingId, GenerationSuiteManifestV1,
    GenerationSystemRecordV1, PlannedCandidateAttemptV1,
};
use rewrite_types::{CancellationToken, Digest, RewriteUnitId};

mod digest;
mod error;
mod validation;

#[cfg(any(test, feature = "test-support"))]
#[path = "generation_qualification_request_builder/tests/support.rs"]
#[cfg_attr(
    all(feature = "test-support", not(test)),
    expect(
        dead_code,
        reason = "the synthetic integration seam reuses a strict subset of request fixture support"
    )
)]
pub(super) mod fixture_support;

mod bindings;
pub use bindings::GenerationQualificationRequestBuilderBindingsV1;
pub use error::GenerationQualificationRequestBuildError;

pub use digest::{
    GENERATION_QUALIFICATION_GROUNDED_REQUEST_DIGEST_DOMAIN,
    GENERATION_QUALIFICATION_PLANNER_DIGEST_DOMAIN, GENERATION_QUALIFICATION_PROMPT_DIGEST_DOMAIN,
    GENERATION_QUALIFICATION_REQUEST_POLICY_DIGEST_DOMAIN,
    GENERATION_QUALIFICATION_STRATEGY_DIGEST_DOMAIN,
};

/// Frozen instruction template for V1 qualification candidate requests.
pub const GENERATION_QUALIFICATION_PROMPT_TEMPLATE_V1: &str =
    "Rewrite the masked source conservatively.";
/// Current deterministic request-builder contract schema.
pub const GENERATION_QUALIFICATION_REQUEST_BUILDER_SCHEMA_VERSION: u32 = 1;

/// Exact owned inputs for one system-and-case request-builder authority.
pub struct GenerationQualificationRequestBuilderV1Input {
    /// Exact generation case bound to the retained source authority.
    pub case: GenerationCaseManifestV1,
    /// Exact deterministic expectations and protected terms for the case.
    pub deterministic_contract: GenerationDeterministicCaseContractV1,
    /// Typed request profile that gives meaning to legacy selector digests.
    pub request_profile: GenerationCaseRequestProfileV1,
    /// Stable generation-system record selected for requests from this builder.
    pub generation_system: GenerationSystemRecordV1,
    /// Production-approved managed-candidate policy authority, consumed once.
    pub generation_policy: VerifiedGenerationSystemPolicy,
}

/// Noncloneable, nonserializable authority to build one bounded request at a time.
///
/// A built authority retains the exclusive builder borrow, so a second request
/// cannot coexist with it.
///
/// ```compile_fail
/// use rewrite_app::{GenerationQualificationRequestBuildInput,
///     GenerationQualificationRequestBuilderV1};
/// use rewrite_types::CancellationToken;
/// fn cannot_coexist<'a, 'store>(
///     builder: &'a mut GenerationQualificationRequestBuilderV1,
///     first: GenerationQualificationRequestBuildInput<'a, 'store>,
///     second: GenerationQualificationRequestBuildInput<'a, 'store>,
///     cancellation: &CancellationToken,
/// ) {
///     let held = builder.build(first, cancellation).unwrap();
///     let other = builder.build(second, cancellation).unwrap();
///     drop((held, other));
/// }
/// ```
pub struct GenerationQualificationRequestBuilderV1 {
    case: GenerationCaseManifestV1,
    deterministic_contract: GenerationDeterministicCaseContractV1,
    request_profile: GenerationCaseRequestProfileV1,
    generation_system: GenerationSystemRecordV1,
    generation_policy: VerifiedGenerationSystemPolicy,
    bindings: GenerationQualificationRequestBuilderBindingsV1,
}

impl GenerationQualificationRequestBuilderV1 {
    /// Consumes one approved policy and validates its exact system and case closure.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for a substituted profile, policy, case,
    /// generation system, or fixed V1 component binding.
    pub fn new(
        input: GenerationQualificationRequestBuilderV1Input,
    ) -> Result<Self, GenerationQualificationRequestBuildError> {
        let bindings = GenerationQualificationRequestBuilderBindingsV1::derive(
            &input.request_profile,
            input.generation_policy.bindings().validator_digest(),
            input.generation_policy.bindings().adapter_digest(),
        )?;
        let value = Self {
            case: input.case,
            deterministic_contract: input.deterministic_contract,
            request_profile: input.request_profile,
            generation_system: input.generation_system,
            generation_policy: input.generation_policy,
            bindings,
        };
        validation::validate_builder(&value)?;
        Ok(value)
    }

    /// Derives the content-free attempt facts needed before a plan can be frozen.
    ///
    /// This path accepts only the exact retained source, a declared deterministic
    /// seed, and one already validated common operation-limit value. It accepts no
    /// request digest, request identity, output-contract digest, or candidate count.
    /// Raw request and protection content is discarded before the authority returns.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for source drift, unsupported text, an invalid
    /// unit shape, protection or rendering failure, or a policy or limit mismatch.
    pub fn derive_preplanning<'authority, 'store>(
        &'authority mut self,
        source: &'authority GenerationCaseSourceLease<'store>,
        declared_seed: u64,
        common_limits: GenerationQualificationOperationLimitsV1,
        cancellation: &CancellationToken,
    ) -> Result<
        PreplannedGenerationQualificationRequestV1<'authority, 'store>,
        GenerationQualificationRequestBuildError,
    > {
        validation::validate_builder(self)?;
        validation::validate_source_relationship(self, source, common_limits)?;
        let material = source
            .with_source_bytes(cancellation, |bytes| {
                validation::build_material_for_seed(self, declared_seed, common_limits, bytes)
            })
            .map_err(|error| map_source_error(&error))??;
        validation::validate_builder(self)?;
        validation::validate_source_relationship(self, source, common_limits)?;
        Ok(PreplannedGenerationQualificationRequestV1 {
            builder: self,
            source,
            declared_seed,
            common_limits,
            facts: PreplannedGenerationQualificationRequestFactsV1::from_material(&material),
        })
    }

    /// Builds one exact request while the source lease is freshly revalidated.
    ///
    /// The source is exposed only to the lease callback. No caller-supplied
    /// request identity, candidate count, seed, sampling value, or limit is accepted.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for source drift, unsupported text, an
    /// invalid unit shape, protection or rendering failure, or any substituted
    /// operation, plan, attempt, system, case, request, or limit relationship.
    pub fn build<'authority, 'store>(
        &'authority mut self,
        input: GenerationQualificationRequestBuildInput<'authority, 'store>,
        cancellation: &CancellationToken,
    ) -> Result<
        BuiltGenerationQualificationRequestV1<'authority, 'store>,
        GenerationQualificationRequestBuildError,
    > {
        validation::validate_builder(self)?;
        validation::validate_context(self, input)?;
        let material = input
            .source
            .with_source_bytes(cancellation, |source| {
                validation::build_material(self, input, source)
            })
            .map_err(|error| map_source_error(&error))??;
        validation::validate_builder(self)?;
        validation::validate_context(self, input)?;
        validation::validate_material(input, &material)?;
        Ok(BuiltGenerationQualificationRequestV1 {
            builder: self,
            input,
            unit_id: material.unit_id,
            grounded_request_digest: material.grounded_request_digest,
            generation_request: material.generation_request,
            structured_request: material.structured_request,
            output_ceilings: material.output_ceilings,
        })
    }

    /// Returns the exact case retained by this authority.
    #[must_use]
    pub const fn case(&self) -> &GenerationCaseManifestV1 {
        &self.case
    }

    /// Returns the exact generation system retained by this authority.
    #[must_use]
    pub const fn generation_system(&self) -> &GenerationSystemRecordV1 {
        &self.generation_system
    }

    /// Returns the internally derived builder-owned policy bindings.
    #[must_use]
    pub const fn bindings(&self) -> &GenerationQualificationRequestBuilderBindingsV1 {
        &self.bindings
    }

    pub(super) fn revalidate_static(&self) -> Result<(), GenerationQualificationRequestBuildError> {
        validation::validate_builder(self)
    }
}

impl fmt::Debug for GenerationQualificationRequestBuilderV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationRequestBuilderV1")
            .field("case_id", self.case.case_id())
            .field(
                "generation_system_id",
                self.generation_system.generation_system_id(),
            )
            .field("policy_control_id", self.generation_policy.control_id())
            .finish_non_exhaustive()
    }
}

/// Exact typed records required to build one predeclared request.
#[derive(Clone, Copy)]
pub struct GenerationQualificationRequestBuildInput<'authority, 'store> {
    /// Retained exact source capability for the builder's case.
    pub source: &'authority GenerationCaseSourceLease<'store>,
    /// Exact preregistered operation policy whose limits govern the request.
    pub operation_policy: &'authority GenerationQualificationOperationPolicyV1,
    /// Exact frozen qualification plan containing the attempt.
    pub qualification_plan: &'authority GenerationQualificationPlanV1,
    /// Exact suite containing the builder's case.
    pub suite: &'authority GenerationSuiteManifestV1,
    /// Exact cluster named by the builder's case.
    pub cluster: &'authority GenerationClusterRecordV1,
    /// Exact suite repetition named by the attempt.
    pub repetition: &'authority GenerationRepetitionRecordV1,
    /// Exact predeclared attempt selected by semantic plan position.
    pub planned_attempt: &'authority PlannedCandidateAttemptV1,
}

/// Short-lived opaque authority over one exact provider-neutral and wire request.
///
/// ```compile_fail
/// use rewrite_app::BuiltGenerationQualificationRequestV1;
/// fn cannot_clone(value: &BuiltGenerationQualificationRequestV1<'_, '_>) {
///     let _copy = (*value).clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::BuiltGenerationQualificationRequestV1;
/// fn require_serialize<T: serde::Serialize>(_: &T) {}
/// fn cannot_serialize(value: &BuiltGenerationQualificationRequestV1<'_, '_>) {
///     require_serialize(value);
/// }
/// ```
pub struct BuiltGenerationQualificationRequestV1<'authority, 'store> {
    builder: &'authority mut GenerationQualificationRequestBuilderV1,
    input: GenerationQualificationRequestBuildInput<'authority, 'store>,
    unit_id: RewriteUnitId,
    grounded_request_digest: Digest,
    generation_request: GenerationRequest,
    structured_request: StructuredCompletionRequest,
    output_ceilings: CandidateOutputCeilingsV1,
}

impl BuiltGenerationQualificationRequestV1<'_, '_> {
    /// Freshly revalidates the retained source and all immutable relationships.
    ///
    /// # Errors
    ///
    /// Returns a content-free error if the source or any exact relationship no
    /// longer agrees with this built request.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationRequestBuildError> {
        validation::validate_builder(self.builder)?;
        validation::validate_context(self.builder, self.input)?;
        self.input
            .source
            .revalidate(cancellation)
            .map_err(|error| map_source_error(&error))?;
        validation::validate_built(self)
    }

    /// Returns only the content-free facts required by the projection compiler.
    ///
    /// # Errors
    ///
    /// Returns a content-free derivation error if the complete input length
    /// cannot be represented by the portable projection schema.
    pub fn projection_input(
        &self,
    ) -> Result<
        GenerationQualificationRequestProjectionEntryV1Input,
        GenerationQualificationRequestBuildError,
    > {
        Ok(GenerationQualificationRequestProjectionEntryV1Input {
            structured_completion_request_binding_id: self
                .structured_request
                .structured_request_binding_id(),
            complete_input_byte_count: u64::try_from(self.generation_request.input.len())
                .map_err(|_| GenerationQualificationRequestBuildError::RequestMismatch)?,
            context_token_limit: self.generation_request.context_token_limit,
            output_token_limit: self.generation_request.output_token_limit,
        })
    }
}

impl fmt::Debug for BuiltGenerationQualificationRequestV1<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BuiltGenerationQualificationRequestV1")
            .field(
                "planned_attempt_id",
                self.input.planned_attempt.planned_attempt_id(),
            )
            .field("unit_id", &self.unit_id)
            .field(
                "generation_request_binding_id",
                &self.generation_request.generation_request_binding_id(),
            )
            .field(
                "structured_request_binding_id",
                &self.structured_request.structured_request_binding_id(),
            )
            .finish_non_exhaustive()
    }
}

/// Content-free facts derived before one planned attempt is constructed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreplannedGenerationQualificationRequestFactsV1 {
    grounded_request_digest: Digest,
    generation_request_binding_id: GenerationRequestBindingId,
    candidate_output_contract_digest: Digest,
    output_ceilings: CandidateOutputCeilingsV1,
}

impl PreplannedGenerationQualificationRequestFactsV1 {
    fn from_material(material: &validation::BuiltRequestMaterial) -> Self {
        Self {
            grounded_request_digest: material.grounded_request_digest.clone(),
            generation_request_binding_id: material
                .generation_request
                .generation_request_binding_id(),
            candidate_output_contract_digest: material
                .generation_request
                .output
                .schema_digest
                .clone(),
            output_ceilings: material.output_ceilings,
        }
    }

    /// Returns the derived grounded-request digest.
    #[must_use]
    pub const fn grounded_request_digest(&self) -> &Digest {
        &self.grounded_request_digest
    }

    /// Returns the derived provider-neutral request binding.
    #[must_use]
    pub const fn generation_request_binding_id(&self) -> &GenerationRequestBindingId {
        &self.generation_request_binding_id
    }

    /// Returns the exact fixed candidate-output contract digest.
    #[must_use]
    pub const fn candidate_output_contract_digest(&self) -> &Digest {
        &self.candidate_output_contract_digest
    }

    /// Returns the internally derived exact candidate-output ceilings.
    #[must_use]
    pub const fn output_ceilings(&self) -> CandidateOutputCeilingsV1 {
        self.output_ceilings
    }
}

/// Noncloneable preplanning authority containing no raw source or prompt copy.
///
/// ```compile_fail
/// use rewrite_app::{GenerationCaseSourceLease,
///     GenerationQualificationRequestBuilderV1};
/// use rewrite_model::GenerationQualificationOperationLimitsV1;
/// use rewrite_types::CancellationToken;
/// fn cannot_coexist<'a, 'store>(
///     builder: &'a mut GenerationQualificationRequestBuilderV1,
///     source: &'a GenerationCaseSourceLease<'store>,
///     limits: GenerationQualificationOperationLimitsV1,
///     cancellation: &CancellationToken,
/// ) {
///     let held = builder.derive_preplanning(source, 7, limits, cancellation).unwrap();
///     let other = builder.derive_preplanning(source, 8, limits, cancellation).unwrap();
///     drop((held, other));
/// }
/// ```
pub struct PreplannedGenerationQualificationRequestV1<'authority, 'store> {
    builder: &'authority mut GenerationQualificationRequestBuilderV1,
    source: &'authority GenerationCaseSourceLease<'store>,
    declared_seed: u64,
    common_limits: GenerationQualificationOperationLimitsV1,
    facts: PreplannedGenerationQualificationRequestFactsV1,
}

impl PreplannedGenerationQualificationRequestV1<'_, '_> {
    /// Freshly rederives the bounded request facts and compares them exactly.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for source drift or any changed derivation.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationRequestBuildError> {
        validation::validate_builder(self.builder)?;
        validation::validate_source_relationship(self.builder, self.source, self.common_limits)?;
        let material = self
            .source
            .with_source_bytes(cancellation, |bytes| {
                validation::build_material_for_seed(
                    self.builder,
                    self.declared_seed,
                    self.common_limits,
                    bytes,
                )
            })
            .map_err(|error| map_source_error(&error))??;
        if PreplannedGenerationQualificationRequestFactsV1::from_material(&material) != self.facts {
            return Err(GenerationQualificationRequestBuildError::RequestMismatch);
        }
        validation::validate_builder(self.builder)?;
        validation::validate_source_relationship(self.builder, self.source, self.common_limits)
    }

    /// Returns the explicit declared seed used for deterministic derivation.
    #[must_use]
    pub const fn declared_seed(&self) -> u64 {
        self.declared_seed
    }

    /// Returns only content-free facts needed by an eventual attempt compiler.
    #[must_use]
    pub const fn facts(&self) -> &PreplannedGenerationQualificationRequestFactsV1 {
        &self.facts
    }
}

impl fmt::Debug for PreplannedGenerationQualificationRequestV1<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreplannedGenerationQualificationRequestV1")
            .field("case_id", self.builder.case.case_id())
            .field(
                "generation_system_id",
                self.builder.generation_system.generation_system_id(),
            )
            .field("declared_seed", &self.declared_seed)
            .finish_non_exhaustive()
    }
}

fn map_source_error(
    error: &GenerationCaseSourceLeaseError,
) -> GenerationQualificationRequestBuildError {
    match error {
        GenerationCaseSourceLeaseError::Boundary(ArtifactInventoryError::Cancelled) => {
            GenerationQualificationRequestBuildError::Cancelled
        }
        _ => GenerationQualificationRequestBuildError::SourceMismatch,
    }
}

#[cfg(test)]
#[path = "generation_qualification_request_builder/tests.rs"]
pub(super) mod tests;
