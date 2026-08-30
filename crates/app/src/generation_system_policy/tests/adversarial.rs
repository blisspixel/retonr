use serde_json::{Value, json};

use super::*;

fn exact_compiled() -> (
    GenerationSystemPolicyBindingsV1,
    CompiledGenerationSystemPolicyControl,
) {
    let bindings = bindings();
    let review = reviewer_json_for_test(
        &bindings,
        GenerationSystemPolicyPermission::ConstructGenerationSystem,
        GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
    );
    let compiled = GenerationSystemPolicyCompiler::compile(
        &bindings,
        GenerationSystemPolicyPermission::ConstructGenerationSystem,
        GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
        &review,
    )
    .expect("compile exact policy");
    (bindings, compiled)
}

fn exact_approval(
    compiled: &CompiledGenerationSystemPolicyControl,
) -> ProductionGenerationSystemPolicyApproval {
    ProductionGenerationSystemPolicyApproval::exact_test_policy(
        compiled.control_id().clone(),
        GenerationSystemPolicyPermission::ConstructGenerationSystem,
        GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
    )
}

#[test]
fn production_policy_denies_even_a_structurally_exact_control() {
    let (bindings, compiled) = exact_compiled();
    assert_eq!(
        GenerationSystemPolicyVerifier::verify(
            compiled.canonical_bytes(),
            &bindings,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
            &ProductionGenerationSystemPolicyApproval::default(),
        )
        .expect_err("empty production policy denies"),
        GenerationSystemPolicyError::ApprovalPolicyDenied
    );
    assert_eq!(
        format!("{:?}", ProductionGenerationSystemPolicyApproval::new()),
        "ProductionGenerationSystemPolicyApproval { approved_control_count: 0, .. }"
    );
}

#[test]
fn production_policy_denial_does_not_mask_untrusted_control_failures() {
    let (bindings, compiled) = exact_compiled();
    let production_policy = ProductionGenerationSystemPolicyApproval::new();
    let permission = GenerationSystemPolicyPermission::ConstructGenerationSystem;
    let purpose = GenerationSystemPolicyPurpose::ManagedCandidateGeneration;

    assert_eq!(
        GenerationSystemPolicyVerifier::verify(
            b"{",
            &bindings,
            permission,
            purpose,
            &production_policy,
        )
        .expect_err("malformed control fails before policy"),
        GenerationSystemPolicyError::InvalidEncoding
    );

    let mut foreign_bindings = bindings.clone();
    foreign_bindings.strategy_digest = digest("foreign strategy");
    let foreign_review = reviewer_json_for_test(&foreign_bindings, permission, purpose);
    let foreign_control = GenerationSystemPolicyCompiler::compile(
        &foreign_bindings,
        permission,
        purpose,
        &foreign_review,
    )
    .expect("compile exact foreign control");
    assert_eq!(
        GenerationSystemPolicyVerifier::verify(
            foreign_control.canonical_bytes(),
            &bindings,
            permission,
            purpose,
            &production_policy,
        )
        .expect_err("foreign binding fails before policy"),
        GenerationSystemPolicyError::InvalidBinding
    );

    let mut substituted = super::super::wire::parse_control(compiled.canonical_bytes())
        .expect("canonical control value");
    substituted.review_evidence_digest = digest("substituted review evidence");
    let substituted =
        super::super::wire::encode_control(&substituted).expect("canonical substituted control");
    assert_eq!(
        GenerationSystemPolicyVerifier::verify(
            &substituted,
            &bindings,
            permission,
            purpose,
            &production_policy,
        )
        .expect_err("canonical substitution fails before policy"),
        GenerationSystemPolicyError::InvalidBinding
    );

    let mut current_bindings = bindings.clone();
    current_bindings.hardware_envelope_digest = digest("current hardware envelope");
    assert_eq!(
        GenerationSystemPolicyVerifier::verify(
            compiled.canonical_bytes(),
            &current_bindings,
            permission,
            purpose,
            &production_policy,
        )
        .expect_err("stale retained static binding fails before policy"),
        GenerationSystemPolicyError::InvalidBinding
    );
}

#[test]
fn review_must_match_every_binding_permission_and_purpose() {
    let original = bindings();
    let exact_review = reviewer_json_for_test(
        &original,
        GenerationSystemPolicyPermission::ConstructGenerationSystem,
        GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
    );
    let mut changed = original.clone();
    changed.strategy_digest = digest("other strategy");
    assert_eq!(
        GenerationSystemPolicyCompiler::compile(
            &changed,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
            &exact_review,
        )
        .expect_err("substituted binding fails"),
        GenerationSystemPolicyError::ReviewRequired
    );
    assert_eq!(
        GenerationSystemPolicyCompiler::compile(
            &original,
            GenerationSystemPolicyPermission::ValidateGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
            &exact_review,
        )
        .expect_err("permission substitution fails"),
        GenerationSystemPolicyError::PermissionMismatch
    );
    assert_eq!(
        GenerationSystemPolicyCompiler::compile(
            &original,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedJudgeGeneration,
            &exact_review,
        )
        .expect_err("purpose substitution fails"),
        GenerationSystemPolicyError::PurposeMismatch
    );
}

#[test]
fn every_binding_changes_review_and_control_identity() {
    let original = bindings();
    let (_, compiled) = exact_compiled();
    let variants = [
        |v: &mut GenerationSystemPolicyBindingsV1| v.strategy_digest = digest("changed"),
        |v: &mut GenerationSystemPolicyBindingsV1| v.planner_digest = digest("changed"),
        |v: &mut GenerationSystemPolicyBindingsV1| v.validator_digest = digest("changed"),
        |v: &mut GenerationSystemPolicyBindingsV1| v.adapter_digest = digest("changed"),
        |v: &mut GenerationSystemPolicyBindingsV1| v.prompt_digest = digest("changed"),
        |v: &mut GenerationSystemPolicyBindingsV1| v.output_schema_digest = digest("changed"),
        |v: &mut GenerationSystemPolicyBindingsV1| v.request_policy_digest = digest("changed"),
        |v: &mut GenerationSystemPolicyBindingsV1| v.language_digest = digest("changed"),
        |v: &mut GenerationSystemPolicyBindingsV1| v.mode_digest = digest("changed"),
        |v: &mut GenerationSystemPolicyBindingsV1| v.format_digest = digest("changed"),
        |v: &mut GenerationSystemPolicyBindingsV1| {
            v.operating_system_digest = digest("changed");
        },
        |v: &mut GenerationSystemPolicyBindingsV1| v.architecture_digest = digest("changed"),
        |v: &mut GenerationSystemPolicyBindingsV1| v.execution_class_digest = digest("changed"),
        |v: &mut GenerationSystemPolicyBindingsV1| {
            v.hardware_envelope_digest = digest("changed");
        },
    ];
    for mutate in variants {
        let mut changed = original.clone();
        mutate(&mut changed);
        let review = reviewer_json_for_test(
            &changed,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
        );
        let candidate = GenerationSystemPolicyCompiler::compile(
            &changed,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
            &review,
        )
        .expect("compile changed exact policy");
        assert_ne!(candidate.control_id(), compiled.control_id());
        assert_ne!(
            candidate.review_evidence_digest(),
            compiled.review_evidence_digest()
        );
    }
}

#[test]
fn verifier_rejects_expected_binding_permission_and_purpose_substitution() {
    let (bindings, compiled) = exact_compiled();
    let approval = exact_approval(&compiled);
    let mut changed = bindings.clone();
    changed.hardware_envelope_digest = digest("other hardware");
    assert_eq!(
        GenerationSystemPolicyVerifier::verify(
            compiled.canonical_bytes(),
            &changed,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
            &approval,
        )
        .expect_err("expected binding substitution fails"),
        GenerationSystemPolicyError::InvalidBinding
    );
    assert_eq!(
        GenerationSystemPolicyVerifier::verify(
            compiled.canonical_bytes(),
            &bindings,
            GenerationSystemPolicyPermission::ValidateGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
            &approval,
        )
        .expect_err("intended permission substitution fails"),
        GenerationSystemPolicyError::PermissionMismatch
    );
    assert_eq!(
        GenerationSystemPolicyVerifier::verify(
            compiled.canonical_bytes(),
            &bindings,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedJudgeGeneration,
            &approval,
        )
        .expect_err("intended purpose substitution fails"),
        GenerationSystemPolicyError::PurposeMismatch
    );
}

#[test]
fn control_is_self_contained_canonical_and_tamper_evident() {
    let (bindings, compiled) = exact_compiled();
    let original: Value =
        serde_json::from_slice(compiled.canonical_bytes()).expect("canonical control JSON");
    let mutations = [
        ("authority", json!("other")),
        ("control", json!("other")),
        ("procedure_id", json!("other")),
        ("procedure_version", json!(2)),
        ("review_evidence_digest", json!(digest("other"))),
        ("schema_version", json!(2)),
        ("status", json!("other")),
    ];
    for (field, replacement) in mutations {
        let mut changed = original.clone();
        changed[field] = replacement;
        let bytes = serde_json::to_vec(&changed).expect("changed control JSON");
        let changed_id = control_id(&bytes);
        let approval = ProductionGenerationSystemPolicyApproval::exact_test_policy(
            changed_id,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
        );
        assert!(
            GenerationSystemPolicyVerifier::verify(
                &bytes,
                &bindings,
                GenerationSystemPolicyPermission::ConstructGenerationSystem,
                GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
                &approval,
            )
            .is_err(),
            "field {field}"
        );
    }
}

#[test]
fn strict_codecs_reject_empty_oversized_unknown_and_noncanonical_json() {
    let bindings = bindings();
    let permission = GenerationSystemPolicyPermission::ConstructGenerationSystem;
    let purpose = GenerationSystemPolicyPurpose::ManagedCandidateGeneration;
    let review = reviewer_json_for_test(&bindings, permission, purpose);
    let pretty_review =
        serde_json::to_vec_pretty(&serde_json::from_slice::<Value>(&review).expect("review value"))
            .expect("pretty review");
    for bytes in [b"".as_slice(), b"{".as_slice(), pretty_review.as_slice()] {
        assert_eq!(
            GenerationSystemPolicyCompiler::compile(&bindings, permission, purpose, bytes)
                .expect_err("invalid review rejected"),
            GenerationSystemPolicyError::InvalidEncoding
        );
    }
    assert_eq!(
        GenerationSystemPolicyCompiler::compile(
            &bindings,
            permission,
            purpose,
            &vec![b' '; MAX_GENERATION_SYSTEM_POLICY_REVIEW_JSON_BYTES + 1],
        )
        .expect_err("oversized review rejected"),
        GenerationSystemPolicyError::LimitExceeded
    );

    let (_, compiled) = exact_compiled();
    let mut unknown: Value =
        serde_json::from_slice(compiled.canonical_bytes()).expect("control value");
    unknown["unknown"] = json!(true);
    let unknown = serde_json::to_vec(&unknown).expect("unknown control");
    let approval = exact_approval(&compiled);
    assert_eq!(
        GenerationSystemPolicyVerifier::verify(
            &unknown,
            &bindings,
            permission,
            purpose,
            &approval,
        )
        .expect_err("unknown control field rejected"),
        GenerationSystemPolicyError::InvalidEncoding
    );
    assert_eq!(
        GenerationSystemPolicyVerifier::verify(
            &vec![b' '; MAX_GENERATION_SYSTEM_POLICY_CONTROL_JSON_BYTES + 1],
            &bindings,
            permission,
            purpose,
            &approval,
        )
        .expect_err("oversized control rejected"),
        GenerationSystemPolicyError::LimitExceeded
    );
}
