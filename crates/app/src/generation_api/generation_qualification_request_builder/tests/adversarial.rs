use rewrite_model::{
    CandidateOutputCeilingsV1, GenerationCaseRequestProfileV1, GenerationClusterRecordV1,
    GenerationRepetitionRecordV1, GenerationSuiteManifestV1,
};
use rewrite_types::{CancellationToken, RewriteMode};

use super::super::*;
use super::support::{Fixture, SOURCE, common_limits, digest, policy_for};
use crate::{GenerationSystemPolicyPermission, GenerationSystemPolicyPurpose};

#[test]
fn errors_are_content_free() {
    let error = GenerationQualificationRequestBuildError::RequestMismatch;
    assert_eq!(
        error.to_string(),
        "generation qualification request derivation does not match"
    );
}

fn exact_facts(fixture: &mut Fixture) -> PreplannedGenerationQualificationRequestFactsV1 {
    let lease = fixture.source.acquire();
    fixture
        .builder
        .derive_preplanning(&lease, 7, common_limits(), &CancellationToken::new())
        .expect("preplanning")
        .facts()
        .clone()
}

#[test]
fn source_state_drift_invalidates_a_built_authority() {
    let mut fixture = Fixture::new(SOURCE, vec!["Acme".to_owned()]);
    let plan = fixture.plan();
    let lease = fixture.source.acquire();
    let built = fixture
        .builder
        .build(
            GenerationQualificationRequestBuildInput {
                source: &lease,
                operation_policy: &plan.operation,
                qualification_plan: &plan.plan,
                suite: &fixture.suite,
                cluster: &fixture.cluster,
                repetition: &fixture.repetition,
                planned_attempt: &plan.target_attempt,
            },
            &CancellationToken::new(),
        )
        .expect("built authority");
    let connection = rusqlite::Connection::open(&fixture.source.database).expect("state database");
    connection
        .execute(
            "UPDATE installed_artifacts SET installation_epoch = installation_epoch + 1",
            [],
        )
        .expect("advance installation epoch");
    assert_eq!(
        built.revalidate(&CancellationToken::new()),
        Err(GenerationQualificationRequestBuildError::SourceMismatch)
    );
}

#[test]
fn a_baseline_attempt_cannot_be_substituted_into_the_target_builder() {
    let mut fixture = Fixture::new(SOURCE, Vec::new());
    let plan = fixture.plan();
    let lease = fixture.source.acquire();
    assert!(matches!(
        fixture.builder.build(
            GenerationQualificationRequestBuildInput {
                source: &lease,
                operation_policy: &plan.operation,
                qualification_plan: &plan.plan,
                suite: &fixture.suite,
                cluster: &fixture.cluster,
                repetition: &fixture.repetition,
                planned_attempt: &plan.baseline_attempt,
            },
            &CancellationToken::new(),
        ),
        Err(GenerationQualificationRequestBuildError::AttemptMismatch)
    ));
}

#[test]
fn every_caller_substituted_planned_request_fact_fails_closed() {
    let mut fixture = Fixture::new(SOURCE, vec!["Acme".to_owned()]);
    let exact = exact_facts(&mut fixture);
    let mut request_changed =
        validation::build_material_for_seed(&fixture.builder, 7, common_limits(), SOURCE)
            .expect("exact material")
            .generation_request;
    request_changed.input.push('x');
    let variants = [
        (
            PreplannedGenerationQualificationRequestFactsV1 {
                grounded_request_digest: digest("substituted grounded request"),
                ..exact.clone()
            },
            GenerationQualificationRequestBuildError::AttemptMismatch,
        ),
        (
            PreplannedGenerationQualificationRequestFactsV1 {
                generation_request_binding_id: request_changed.generation_request_binding_id(),
                ..exact.clone()
            },
            GenerationQualificationRequestBuildError::AttemptMismatch,
        ),
        (
            PreplannedGenerationQualificationRequestFactsV1 {
                candidate_output_contract_digest: digest("substituted output contract"),
                ..exact.clone()
            },
            GenerationQualificationRequestBuildError::OperationMismatch,
        ),
    ];
    for (index, (facts, expected)) in variants.into_iter().enumerate() {
        let plan = fixture.plan_from_target_facts(7, &facts);
        let lease = fixture.source.acquire();
        let error = fixture
            .builder
            .build(
                GenerationQualificationRequestBuildInput {
                    source: &lease,
                    operation_policy: &plan.operation,
                    qualification_plan: &plan.plan,
                    suite: &fixture.suite,
                    cluster: &fixture.cluster,
                    repetition: &fixture.repetition,
                    planned_attempt: &plan.target_attempt,
                },
                &CancellationToken::new(),
            )
            .expect_err("substituted fact must fail");
        assert_eq!(error, expected, "substitution {index}");
    }

    let wrong_seed = fixture.plan_from_target_facts(8, &exact);
    let lease = fixture.source.acquire();
    assert!(matches!(
        fixture.builder.build(
            GenerationQualificationRequestBuildInput {
                source: &lease,
                operation_policy: &wrong_seed.operation,
                qualification_plan: &wrong_seed.plan,
                suite: &fixture.suite,
                cluster: &fixture.cluster,
                repetition: &fixture.repetition,
                planned_attempt: &wrong_seed.target_attempt,
            },
            &CancellationToken::new(),
        ),
        Err(GenerationQualificationRequestBuildError::AttemptMismatch)
    ));
}

#[test]
fn output_ceiling_and_negative_zero_substitutions_fail_closed() {
    let mut fixture = Fixture::new(SOURCE, Vec::new());
    let exact = exact_facts(&mut fixture).output_ceilings();
    let mut aggregate_wire = serde_json::to_value(exact).expect("serialize ceilings");
    aggregate_wire["maximum_aggregate_candidate_bytes"] = serde_json::Value::from(4_095);
    assert!(serde_json::from_value::<CandidateOutputCeilingsV1>(aggregate_wire).is_err());
    let mut envelope_wire = serde_json::to_value(exact).expect("serialize ceilings");
    envelope_wire["maximum_envelope_bytes"] =
        serde_json::Value::from(exact.maximum_envelope_bytes() + 1);
    assert!(serde_json::from_value::<CandidateOutputCeilingsV1>(envelope_wire).is_err());

    let exact_plan = fixture.plan();
    let mut material =
        validation::build_material_for_seed(&fixture.builder, 7, common_limits(), SOURCE)
            .expect("exact material");
    material.generation_request.sampling.temperature = -0.0;
    let lease = fixture.source.acquire();
    assert_eq!(
        validation::validate_material(
            GenerationQualificationRequestBuildInput {
                source: &lease,
                operation_policy: &exact_plan.operation,
                qualification_plan: &exact_plan.plan,
                suite: &fixture.suite,
                cluster: &fixture.cluster,
                repetition: &fixture.repetition,
                planned_attempt: &exact_plan.target_attempt,
            },
            &material,
        ),
        Err(GenerationQualificationRequestBuildError::AttemptMismatch)
    );
}

#[test]
fn wrong_profile_policy_system_and_context_records_are_rejected() {
    let fixture = Fixture::new(SOURCE, Vec::new());
    let system = fixture.builder.generation_system().clone();
    let wrong_purpose = policy_for(
        &system,
        GenerationSystemPolicyPermission::ConstructGenerationSystem,
        GenerationSystemPolicyPurpose::ManagedJudgeGeneration,
    );
    assert!(matches!(
        GenerationQualificationRequestBuilderV1::new(
            GenerationQualificationRequestBuilderV1Input {
                case: fixture.builder.case.clone(),
                deterministic_contract: fixture.builder.deterministic_contract.clone(),
                request_profile: fixture.builder.request_profile.clone(),
                generation_system: system.clone(),
                generation_policy: wrong_purpose,
            },
        ),
        Err(GenerationQualificationRequestBuildError::PolicyMismatch)
    ));

    let exact_policy = policy_for(
        &system,
        GenerationSystemPolicyPermission::ConstructGenerationSystem,
        GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
    );
    assert!(matches!(
        GenerationQualificationRequestBuilderV1::new(
            GenerationQualificationRequestBuilderV1Input {
                case: fixture.builder.case.clone(),
                deterministic_contract: fixture.builder.deterministic_contract.clone(),
                request_profile: GenerationCaseRequestProfileV1::new(RewriteMode::Strong),
                generation_system: system,
                generation_policy: exact_policy,
            },
        ),
        Err(GenerationQualificationRequestBuildError::ProfileMismatch)
    ));
}

#[test]
fn suite_cluster_repetition_and_plan_substitutions_fail_before_rendering() {
    let mut fixture = Fixture::new(SOURCE, Vec::new());
    let plan = fixture.plan();
    let foreign_plan = fixture.plan_with_selection("foreign selection policy");
    let foreign_cluster = GenerationClusterRecordV1::new("foreign", digest("foreign cluster"))
        .expect("foreign cluster");
    let foreign_suite = GenerationSuiteManifestV1::new(
        digest("foreign suite"),
        std::slice::from_ref(&fixture.case),
    )
    .expect("foreign suite");
    let foreign_repetition =
        GenerationRepetitionRecordV1::new(&foreign_suite, 0, digest("foreign repetition"))
            .expect("foreign repetition");
    let lease = fixture.source.acquire();
    for (suite, cluster, repetition) in [
        (&fixture.suite, &foreign_cluster, &fixture.repetition),
        (&foreign_suite, &fixture.cluster, &fixture.repetition),
        (&fixture.suite, &fixture.cluster, &foreign_repetition),
    ] {
        assert!(matches!(
            fixture.builder.build(
                GenerationQualificationRequestBuildInput {
                    source: &lease,
                    operation_policy: &plan.operation,
                    qualification_plan: &plan.plan,
                    suite,
                    cluster,
                    repetition,
                    planned_attempt: &plan.target_attempt,
                },
                &CancellationToken::new(),
            ),
            Err(GenerationQualificationRequestBuildError::AttemptMismatch)
        ));
    }
    for (operation_policy, qualification_plan) in [
        (&plan.operation, &foreign_plan.plan),
        (&foreign_plan.operation, &plan.plan),
    ] {
        assert!(matches!(
            fixture.builder.build(
                GenerationQualificationRequestBuildInput {
                    source: &lease,
                    operation_policy,
                    qualification_plan,
                    suite: &fixture.suite,
                    cluster: &fixture.cluster,
                    repetition: &fixture.repetition,
                    planned_attempt: &plan.target_attempt,
                },
                &CancellationToken::new(),
            ),
            Err(GenerationQualificationRequestBuildError::PlanMismatch)
        ));
    }
}
