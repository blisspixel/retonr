use rewrite_inference::candidate_output_contract;
use rewrite_model::GenerationSystemRecordV1;
use rewrite_types::Digest;

use super::{GenerationQualificationRequestBuilderBindingsV1, verified_policy};
use crate::{
    GenerationSystemPolicyBindingsV1, GenerationSystemPolicyBindingsV1Input,
    GenerationSystemPolicyPermission, GenerationSystemPolicyPurpose,
    VerifiedGenerationSystemPolicy,
};

pub(crate) fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

pub(crate) fn policy_for(
    system: &GenerationSystemRecordV1,
    permission: GenerationSystemPolicyPermission,
    purpose: GenerationSystemPolicyPurpose,
) -> VerifiedGenerationSystemPolicy {
    let bindings = GenerationSystemPolicyBindingsV1::new(GenerationSystemPolicyBindingsV1Input {
        strategy_digest: system.strategy_digest().clone(),
        planner_digest: system.planner_digest().clone(),
        validator_digest: system.validator_digest().clone(),
        adapter_digest: system.adapter_digest().clone(),
        prompt_digest: system.prompt_digest().clone(),
        output_schema_digest: system.output_schema_digest().clone(),
        request_policy_digest: system.request_policy_digest().clone(),
        language_digest: system.language_digest().clone(),
        mode_digest: system.mode_digest().clone(),
        format_digest: system.format_digest().clone(),
        operating_system_digest: system.operating_system_digest().clone(),
        architecture_digest: system.architecture_digest().clone(),
        execution_class_digest: system.execution_class_digest().clone(),
        hardware_envelope_digest: system.hardware_envelope_digest().clone(),
    });
    verified_policy(&bindings, permission, purpose)
}

pub(crate) fn bindings(
    components: &GenerationQualificationRequestBuilderBindingsV1,
    validator_digest: Digest,
    adapter_digest: Digest,
    platform: &str,
) -> GenerationSystemPolicyBindingsV1 {
    bindings_with_platform_digests(
        components,
        validator_digest,
        adapter_digest,
        digest(&format!("{platform} operating system")),
        digest(&format!("{platform} architecture")),
        digest(&format!("{platform} execution class")),
        digest(&format!("{platform} hardware envelope")),
    )
}

pub(crate) fn bindings_with_platform_digests(
    components: &GenerationQualificationRequestBuilderBindingsV1,
    validator_digest: Digest,
    adapter_digest: Digest,
    operating_system_digest: Digest,
    architecture_digest: Digest,
    execution_class_digest: Digest,
    hardware_envelope_digest: Digest,
) -> GenerationSystemPolicyBindingsV1 {
    GenerationSystemPolicyBindingsV1::new(GenerationSystemPolicyBindingsV1Input {
        strategy_digest: components.strategy_digest().clone(),
        planner_digest: components.planner_digest().clone(),
        validator_digest,
        adapter_digest,
        prompt_digest: components.prompt_digest().clone(),
        output_schema_digest: candidate_output_contract().schema_digest,
        request_policy_digest: components.request_policy_digest().clone(),
        language_digest: components.language_digest().clone(),
        mode_digest: components.mode_digest().clone(),
        format_digest: components.format_digest().clone(),
        operating_system_digest,
        architecture_digest,
        execution_class_digest,
        hardware_envelope_digest,
    })
}
