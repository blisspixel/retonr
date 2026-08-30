use rewrite_engine::{ProtectedKind, ProtectionPlan};
use rewrite_grounded::{
    GroundedPromptRenderInputV1, GroundedSentinel, GroundedSentinelKind, render_grounded_prompt_v1,
};
use rewrite_inference::{
    CandidateOutputPolicy, GENERATION_REQUEST_SCHEMA_VERSION, GenerationRequest, ReasoningPolicy,
    SamplingParameters, StructuredCompletionRequest, candidate_output_contract,
    derive_single_candidate_structured_request, validate_single_candidate_structured_request,
};
use rewrite_model::{
    CandidateOutputCeilingsV1, GenerationQualificationOperationLimitsV1,
    PlannedCandidateAttemptV1Relations,
};
use rewrite_text_adapter::TextAdapter;
use rewrite_types::{Digest, RewriteUnit, RewriteUnitId};

use super::{
    BuiltGenerationQualificationRequestV1, GENERATION_QUALIFICATION_PROMPT_TEMPLATE_V1,
    GenerationCaseSourceLease, GenerationQualificationRequestBuildError,
    GenerationQualificationRequestBuildInput, GenerationQualificationRequestBuilderV1, digest,
};
use crate::{GenerationSystemPolicyPermission, GenerationSystemPolicyPurpose};

pub(super) struct BuiltRequestMaterial {
    pub(super) unit_id: RewriteUnitId,
    pub(super) grounded_request_digest: Digest,
    #[cfg(test)]
    pub(super) protection_plan: ProtectionPlan,
    pub(super) generation_request: GenerationRequest,
    pub(super) structured_request: StructuredCompletionRequest,
    pub(super) output_ceilings: CandidateOutputCeilingsV1,
}

pub(super) fn validate_builder(
    builder: &GenerationQualificationRequestBuilderV1,
) -> Result<(), GenerationQualificationRequestBuildError> {
    builder
        .request_profile
        .validate_against(
            &builder.case,
            &builder.deterministic_contract,
            &builder.generation_system,
        )
        .map_err(|_| GenerationQualificationRequestBuildError::ProfileMismatch)?;
    if builder.case.case_contract_digest() != builder.deterministic_contract.contract_digest() {
        return Err(GenerationQualificationRequestBuildError::CaseMismatch);
    }

    let policy = &builder.generation_policy;
    let policy_bindings = policy.bindings();
    let system = &builder.generation_system;
    if policy.permission() != GenerationSystemPolicyPermission::ConstructGenerationSystem
        || policy.purpose() != GenerationSystemPolicyPurpose::ManagedCandidateGeneration
    {
        return Err(GenerationQualificationRequestBuildError::PolicyMismatch);
    }
    for (policy_digest, system_digest) in [
        (policy_bindings.strategy_digest(), system.strategy_digest()),
        (policy_bindings.planner_digest(), system.planner_digest()),
        (
            policy_bindings.validator_digest(),
            system.validator_digest(),
        ),
        (policy_bindings.adapter_digest(), system.adapter_digest()),
        (policy_bindings.prompt_digest(), system.prompt_digest()),
        (
            policy_bindings.output_schema_digest(),
            system.output_schema_digest(),
        ),
        (
            policy_bindings.request_policy_digest(),
            system.request_policy_digest(),
        ),
        (policy_bindings.language_digest(), system.language_digest()),
        (policy_bindings.mode_digest(), system.mode_digest()),
        (policy_bindings.format_digest(), system.format_digest()),
        (
            policy_bindings.operating_system_digest(),
            system.operating_system_digest(),
        ),
        (
            policy_bindings.architecture_digest(),
            system.architecture_digest(),
        ),
        (
            policy_bindings.execution_class_digest(),
            system.execution_class_digest(),
        ),
        (
            policy_bindings.hardware_envelope_digest(),
            system.hardware_envelope_digest(),
        ),
    ] {
        if policy_digest != system_digest {
            return Err(GenerationQualificationRequestBuildError::PolicyMismatch);
        }
    }

    let expected = &builder.bindings;
    if expected.strategy_digest() != system.strategy_digest()
        || expected.planner_digest() != system.planner_digest()
        || expected.prompt_digest() != system.prompt_digest()
        || expected.output_schema_digest() != system.output_schema_digest()
        || expected.request_policy_digest() != system.request_policy_digest()
        || expected.language_digest() != system.language_digest()
        || expected.mode_digest() != system.mode_digest()
        || expected.format_digest() != system.format_digest()
        || candidate_output_contract().schema_digest != *system.output_schema_digest()
    {
        return Err(GenerationQualificationRequestBuildError::SystemMismatch);
    }
    Ok(())
}

pub(super) fn validate_source_relationship(
    builder: &GenerationQualificationRequestBuilderV1,
    source: &GenerationCaseSourceLease<'_>,
    limits: GenerationQualificationOperationLimitsV1,
) -> Result<(), GenerationQualificationRequestBuildError> {
    validate_operation_limits(limits)?;
    if !source.matches_case_manifest(&builder.case)
        || source.source_byte_count() > limits.maximum_source_bytes()
    {
        return Err(GenerationQualificationRequestBuildError::SourceMismatch);
    }
    Ok(())
}

fn validate_operation_limits(
    limits: GenerationQualificationOperationLimitsV1,
) -> Result<(), GenerationQualificationRequestBuildError> {
    let rebuilt = GenerationQualificationOperationLimitsV1::new(
        limits.maximum_source_bytes(),
        limits.maximum_complete_input_bytes(),
        limits.maximum_context_tokens(),
        limits.maximum_output_tokens(),
        limits.maximum_output_bytes(),
        limits.maximum_candidates_per_completion(),
        limits.maximum_candidate_bytes(),
        limits.maximum_aggregate_candidate_bytes(),
        limits.maximum_predeclared_attempts(),
        limits.maximum_concurrent_attempts(),
        limits.maximum_elapsed_milliseconds(),
    )
    .map_err(|_| GenerationQualificationRequestBuildError::OperationMismatch)?;
    if rebuilt != limits {
        return Err(GenerationQualificationRequestBuildError::OperationMismatch);
    }
    Ok(())
}

pub(super) fn validate_context(
    builder: &GenerationQualificationRequestBuilderV1,
    input: GenerationQualificationRequestBuildInput<'_, '_>,
) -> Result<(), GenerationQualificationRequestBuildError> {
    let operation = input.operation_policy;
    let plan = input.qualification_plan;
    let attempt = input.planned_attempt;
    validate_source_relationship(builder, input.source, operation.limits())?;

    attempt
        .validate_against(PlannedCandidateAttemptV1Relations {
            suite: input.suite,
            case: &builder.case,
            cluster: input.cluster,
            repetition: input.repetition,
            generation_system: &builder.generation_system,
        })
        .map_err(|_| GenerationQualificationRequestBuildError::AttemptMismatch)?;

    let ordinal = usize::try_from(attempt.attempt_ordinal())
        .map_err(|_| GenerationQualificationRequestBuildError::PlanMismatch)?;
    if operation.generation_qualification_plan_id() != plan.qualification_plan_id()
        || operation.suite_manifest_id() != input.suite.suite_manifest_id()
        || plan.suite_manifest_id() != input.suite.suite_manifest_id()
        || plan.planned_attempt_ids().get(ordinal) != Some(attempt.planned_attempt_id())
        || plan
            .planned_attempt_ids()
            .iter()
            .filter(|id| *id == attempt.planned_attempt_id())
            .count()
            != 1
        || !plan
            .generation_system_ids()
            .contains(builder.generation_system.generation_system_id())
    {
        return Err(GenerationQualificationRequestBuildError::PlanMismatch);
    }
    if operation.target_generation_system_id() != builder.generation_system.generation_system_id()
        && operation.baseline_generation_system_id()
            != builder.generation_system.generation_system_id()
    {
        return Err(GenerationQualificationRequestBuildError::OperationMismatch);
    }

    let operation_limits = operation.limits();
    let plan_limits = plan.limits();
    let attempt_ceilings = attempt.output_ceilings();
    let expected_attempts = u32::try_from(plan.planned_attempt_ids().len())
        .map_err(|_| GenerationQualificationRequestBuildError::PlanMismatch)?;
    if plan_limits.maximum_candidates_per_completion() != 1
        || operation_limits.maximum_candidates_per_completion() != 1
        || operation_limits.maximum_concurrent_attempts() != 1
        || operation_limits.maximum_predeclared_attempts() != expected_attempts
        || plan_limits.maximum_predeclared_attempts() != expected_attempts
        || operation_limits.maximum_source_bytes() > plan_limits.maximum_retained_input_bytes()
        || operation_limits.maximum_complete_input_bytes()
            > plan_limits.maximum_retained_input_bytes()
        || builder.case.source_byte_count() > operation_limits.maximum_source_bytes()
        || attempt_ceilings.candidate_count() != 1
        || attempt_ceilings.maximum_candidate_bytes() != operation_limits.maximum_candidate_bytes()
        || attempt_ceilings.maximum_aggregate_candidate_bytes()
            != operation_limits.maximum_aggregate_candidate_bytes()
        || attempt_ceilings.maximum_envelope_bytes() != operation_limits.maximum_output_bytes()
        || attempt.candidate_output_contract_digest() != &candidate_output_contract().schema_digest
    {
        return Err(GenerationQualificationRequestBuildError::OperationMismatch);
    }
    Ok(())
}

pub(super) fn build_material(
    builder: &GenerationQualificationRequestBuilderV1,
    input: GenerationQualificationRequestBuildInput<'_, '_>,
    source: &[u8],
) -> Result<BuiltRequestMaterial, GenerationQualificationRequestBuildError> {
    build_material_for_seed(
        builder,
        input.planned_attempt.declared_seed(),
        input.operation_policy.limits(),
        source,
    )
}

pub(super) fn build_material_for_seed(
    builder: &GenerationQualificationRequestBuilderV1,
    declared_seed: u64,
    limits: GenerationQualificationOperationLimitsV1,
    source: &[u8],
) -> Result<BuiltRequestMaterial, GenerationQualificationRequestBuildError> {
    if Digest::sha256(source) != *builder.case.source_digest()
        || u64::try_from(source.len()) != Ok(builder.case.source_byte_count())
        || builder.case.source_byte_count() > limits.maximum_source_bytes()
    {
        return Err(GenerationQualificationRequestBuildError::SourceMismatch);
    }
    let parsed = TextAdapter::parse(source)
        .map_err(|_| GenerationQualificationRequestBuildError::SourceEncodingUnsupported)?;
    let unit = require_single_whole_document_unit(&parsed.document().rewrite_units)?;
    let protection_plan =
        ProtectionPlan::build(&unit.text, builder.deterministic_contract.protected_terms())
            .map_err(|_| GenerationQualificationRequestBuildError::ProtectionFailed)?;
    let sentinels = protection_plan
        .values()
        .iter()
        .map(|value| GroundedSentinel {
            token: value.token.clone(),
            kind: map_protected_kind(value.kind),
        })
        .collect::<Vec<_>>();
    let rendered = render_grounded_prompt_v1(GroundedPromptRenderInputV1 {
        prompt_template: GENERATION_QUALIFICATION_PROMPT_TEMPLATE_V1,
        masked_source: protection_plan.masked_source(),
        protected_sentinels: &sentinels,
        rewrite_mode: builder.request_profile.rewrite_mode(),
        style_context: "",
        required_candidate_count: 1,
        maximum_input_bytes: limits.maximum_complete_input_bytes(),
    })
    .map_err(|_| GenerationQualificationRequestBuildError::PromptFailed)?;

    let output_ceilings = CandidateOutputCeilingsV1::new(
        1,
        limits.maximum_candidate_bytes(),
        limits.maximum_aggregate_candidate_bytes(),
    )
    .map_err(|_| GenerationQualificationRequestBuildError::OperationMismatch)?;
    if output_ceilings.maximum_envelope_bytes() != limits.maximum_output_bytes() {
        return Err(GenerationQualificationRequestBuildError::OperationMismatch);
    }
    let generation_request = GenerationRequest {
        schema_version: GENERATION_REQUEST_SCHEMA_VERSION,
        artifact_id: builder.generation_system.model_artifact_id().clone(),
        artifact_digest: builder
            .generation_system
            .model_artifact_id()
            .digest()
            .clone(),
        input: rendered,
        output: candidate_output_contract(),
        candidate_count: 1,
        source_byte_count: builder.case.source_byte_count(),
        source_byte_limit: limits.maximum_source_bytes(),
        input_byte_limit: limits.maximum_complete_input_bytes(),
        context_token_limit: limits.maximum_context_tokens(),
        output_token_limit: limits.maximum_output_tokens(),
        candidate_byte_limit: limits.maximum_candidate_bytes(),
        sampling: SamplingParameters {
            temperature: f32::from_bits(0x0000_0000),
            top_p: f32::from_bits(0x3f80_0000),
            seed: Some(declared_seed),
        },
        reasoning: ReasoningPolicy::Disabled,
    };
    generation_request
        .validate()
        .map_err(|_| GenerationQualificationRequestBuildError::RequestMismatch)?;
    let output_policy = CandidateOutputPolicy::new(
        1,
        limits.maximum_candidate_bytes(),
        limits.maximum_aggregate_candidate_bytes(),
    )
    .map_err(|_| GenerationQualificationRequestBuildError::OperationMismatch)?;
    let structured_request =
        derive_single_candidate_structured_request(&generation_request, output_policy)
            .map_err(|_| GenerationQualificationRequestBuildError::RequestMismatch)?;
    let grounded_request_digest = digest::grounded_request_digest(
        &builder.case,
        builder.deterministic_contract.contract_digest(),
        builder.bindings.prompt_digest(),
        &unit.id,
        &generation_request.input,
    )?;
    Ok(BuiltRequestMaterial {
        unit_id: unit.id.clone(),
        grounded_request_digest,
        #[cfg(test)]
        protection_plan,
        generation_request,
        structured_request,
        output_ceilings,
    })
}

pub(super) fn validate_material(
    input: GenerationQualificationRequestBuildInput<'_, '_>,
    material: &BuiltRequestMaterial,
) -> Result<(), GenerationQualificationRequestBuildError> {
    validate_material_fields(
        input,
        &material.grounded_request_digest,
        &material.generation_request,
        &material.structured_request,
        material.output_ceilings,
    )
}

fn validate_material_fields(
    input: GenerationQualificationRequestBuildInput<'_, '_>,
    grounded_request_digest: &Digest,
    request: &GenerationRequest,
    structured_request: &StructuredCompletionRequest,
    output_ceilings: CandidateOutputCeilingsV1,
) -> Result<(), GenerationQualificationRequestBuildError> {
    let attempt = input.planned_attempt;
    if grounded_request_digest != attempt.grounded_request_digest()
        || request.generation_request_binding_id() != *attempt.generation_request_binding_id()
        || request.output.schema_digest != *attempt.candidate_output_contract_digest()
        || output_ceilings != attempt.output_ceilings()
        || request.sampling.temperature.to_bits() != 0x0000_0000
        || request.sampling.top_p.to_bits() != 0x3f80_0000
        || request.sampling.seed != Some(attempt.declared_seed())
        || request.reasoning != ReasoningPolicy::Disabled
        || request.source_byte_count != attempt.source_byte_count()
        || request.candidate_count != 1
    {
        return Err(GenerationQualificationRequestBuildError::AttemptMismatch);
    }
    validate_single_candidate_structured_request(
        request,
        CandidateOutputPolicy::new(
            output_ceilings.candidate_count(),
            output_ceilings.maximum_candidate_bytes(),
            output_ceilings.maximum_aggregate_candidate_bytes(),
        )
        .map_err(|_| GenerationQualificationRequestBuildError::RequestMismatch)?,
        structured_request,
    )
    .map_err(|_| GenerationQualificationRequestBuildError::RequestMismatch)?;
    Ok(())
}

pub(super) fn validate_built(
    built: &BuiltGenerationQualificationRequestV1<'_, '_>,
) -> Result<(), GenerationQualificationRequestBuildError> {
    validate_material_fields(
        built.input,
        &built.grounded_request_digest,
        &built.generation_request,
        &built.structured_request,
        built.output_ceilings,
    )
}

fn require_single_whole_document_unit(
    units: &[RewriteUnit],
) -> Result<&RewriteUnit, GenerationQualificationRequestBuildError> {
    let [unit] = units else {
        return Err(GenerationQualificationRequestBuildError::UnitPolicyMismatch);
    };
    if unit.source_span.start() != 0
        || unit.source_span.is_empty()
        || unit.source_span.len() != unit.text.len()
    {
        return Err(GenerationQualificationRequestBuildError::UnitPolicyMismatch);
    }
    Ok(unit)
}

const fn map_protected_kind(kind: ProtectedKind) -> GroundedSentinelKind {
    match kind {
        ProtectedKind::DeclaredTerm => GroundedSentinelKind::DeclaredTerm,
        ProtectedKind::Url => GroundedSentinelKind::Url,
        ProtectedKind::Email => GroundedSentinelKind::Email,
        ProtectedKind::Number => GroundedSentinelKind::Number,
    }
}

#[cfg(test)]
pub(super) fn require_single_unit_for_test(
    units: &[RewriteUnit],
) -> Result<&RewriteUnit, GenerationQualificationRequestBuildError> {
    require_single_whole_document_unit(units)
}
