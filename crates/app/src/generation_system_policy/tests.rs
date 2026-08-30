use super::verification::reviewer_json_for_test;
use super::*;

#[path = "tests/adversarial.rs"]
mod adversarial;

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn bindings() -> GenerationSystemPolicyBindingsV1 {
    GenerationSystemPolicyBindingsV1::new(GenerationSystemPolicyBindingsV1Input {
        strategy_digest: digest("strategy"),
        planner_digest: digest("planner"),
        validator_digest: digest("validator"),
        adapter_digest: digest("adapter"),
        prompt_digest: digest("prompt"),
        output_schema_digest: digest("output schema"),
        request_policy_digest: digest("request policy"),
        language_digest: digest("language"),
        mode_digest: digest("mode"),
        format_digest: digest("format"),
        operating_system_digest: digest("operating system"),
        architecture_digest: digest("architecture"),
        execution_class_digest: digest("execution class"),
        hardware_envelope_digest: digest("hardware envelope"),
    })
}

#[test]
fn production_policy_is_empty() {
    assert_eq!(
        ProductionGenerationSystemPolicyApproval::new().approved_control_count(),
        0
    );
}

#[test]
fn exact_test_approval_issues_policy_authority() {
    let bindings = bindings();
    let permission = GenerationSystemPolicyPermission::ConstructGenerationSystem;
    let purpose = GenerationSystemPolicyPurpose::ManagedCandidateGeneration;
    let review = reviewer_json_for_test(&bindings, permission, purpose);
    let compiled = GenerationSystemPolicyCompiler::compile(&bindings, permission, purpose, &review)
        .expect("compile exact policy");
    let approval = ProductionGenerationSystemPolicyApproval::exact_test_policy(
        compiled.control_id().clone(),
        permission,
        purpose,
    );
    let verified = GenerationSystemPolicyVerifier::verify(
        compiled.canonical_bytes(),
        &bindings,
        permission,
        purpose,
        &approval,
    )
    .expect("verify exact approved policy");
    assert_eq!(verified.control_id(), compiled.control_id());
    assert_eq!(verified.bindings(), &bindings);
    assert_eq!(verified.permission(), permission);
    assert_eq!(verified.purpose(), purpose);
    assert_eq!(compiled.permission(), permission);
    assert_eq!(compiled.purpose(), purpose);
    assert_eq!(compiled.bindings(), &bindings);
    assert_ne!(
        compiled.review_evidence_digest(),
        compiled.control_id().digest()
    );
    assert_eq!(
        compiled.control_id().digest().as_str(),
        "88dc0cb1f72c174b71cd56c95cad6c0e92aaf58ee238193f948dd9f44c3a2099"
    );
    assert_eq!(
        format!("{verified:?}"),
        format!(
            "VerifiedGenerationSystemPolicy {{ control_id: {:?}, permission: ConstructGenerationSystem, purpose: ManagedCandidateGeneration, .. }}",
            compiled.control_id()
        )
    );
}

#[test]
fn binding_getters_preserve_every_named_input() {
    let bindings = bindings();
    assert_eq!(bindings.strategy_digest(), &digest("strategy"));
    assert_eq!(bindings.planner_digest(), &digest("planner"));
    assert_eq!(bindings.validator_digest(), &digest("validator"));
    assert_eq!(bindings.adapter_digest(), &digest("adapter"));
    assert_eq!(bindings.prompt_digest(), &digest("prompt"));
    assert_eq!(bindings.output_schema_digest(), &digest("output schema"));
    assert_eq!(bindings.request_policy_digest(), &digest("request policy"));
    assert_eq!(bindings.language_digest(), &digest("language"));
    assert_eq!(bindings.mode_digest(), &digest("mode"));
    assert_eq!(bindings.format_digest(), &digest("format"));
    assert_eq!(
        bindings.operating_system_digest(),
        &digest("operating system")
    );
    assert_eq!(bindings.architecture_digest(), &digest("architecture"));
    assert_eq!(
        bindings.execution_class_digest(),
        &digest("execution class")
    );
    assert_eq!(
        bindings.hardware_envelope_digest(),
        &digest("hardware envelope")
    );
}
