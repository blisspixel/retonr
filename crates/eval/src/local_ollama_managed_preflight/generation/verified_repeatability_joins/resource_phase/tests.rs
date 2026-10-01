use rewrite_app::{
    GenerationQualificationPhasePolicyVerifier, GenerationQualificationResourcePolicyLimitsV1,
    ProductionGenerationQualificationResourcePolicySource,
    SyntheticGenerationQualificationScenario, with_synthetic_generation_qualification_fixture,
};
use rewrite_model::{
    GenerationQualificationPhaseScopeV1, GenerationQualificationPhaseStatusV1,
    GenerationResourceExceededLimitV1,
};
use rewrite_types::{CancellationToken, Digest};

use super::{
    FrozenResourcePhaseEvidence,
    GenerationQualificationResourcePhaseAuthorityError as AuthorityError,
    GenerationQualificationResourcePhaseCompilationError as CompilationError,
    GenerationQualificationResourcePhaseDerivationError as DerivationError, ResourceEvidenceView,
    ResourceFacts, ResourceManifestDerivation, derive_phase, derive_resource_manifest,
    exceeded_limits, phase_status,
};
use crate::generation_case_material::verified_material_test_support::Fixture;

use super::super::tests::complete_qualification_authority;

#[path = "tests/authority_tests.rs"]
mod authority_tests;

fn limits() -> GenerationQualificationResourcePolicyLimitsV1 {
    GenerationQualificationResourcePolicyLimitsV1 {
        maximum_attempt_elapsed_nanoseconds: 10,
        maximum_first_response_nanoseconds: 20,
        maximum_cleanup_nanoseconds: 30,
        maximum_worker_high_water_resident_bytes: 40,
        maximum_installed_footprint_bytes: 50,
    }
}

#[test]
fn exact_boundaries_pass_and_exceedances_keep_closed_order() {
    let exact = ResourceFacts {
        attempt_elapsed_nanoseconds: 10,
        first_response_elapsed_nanoseconds: 20,
        cleanup_elapsed_nanoseconds: 30,
        worker_high_water_resident_bytes: 40,
        installed_footprint_bytes: 50,
    };
    assert!(exceeded_limits(exact, limits()).is_empty());
    assert_eq!(
        phase_status(false),
        GenerationQualificationPhaseStatusV1::Passed
    );

    let exceeded = ResourceFacts {
        attempt_elapsed_nanoseconds: 11,
        first_response_elapsed_nanoseconds: 21,
        cleanup_elapsed_nanoseconds: 31,
        worker_high_water_resident_bytes: 41,
        installed_footprint_bytes: 51,
    };
    assert_eq!(
        exceeded_limits(exceeded, limits()),
        [
            GenerationResourceExceededLimitV1::AttemptElapsed,
            GenerationResourceExceededLimitV1::FirstResponse,
            GenerationResourceExceededLimitV1::Cleanup,
            GenerationResourceExceededLimitV1::WorkerHighWaterResident,
            GenerationResourceExceededLimitV1::InstalledFootprint,
        ]
    );
    assert_eq!(
        phase_status(true),
        GenerationQualificationPhaseStatusV1::Failed
    );
}

#[test]
fn each_independent_limit_is_classified_without_widening() {
    let base = ResourceFacts {
        attempt_elapsed_nanoseconds: 0,
        first_response_elapsed_nanoseconds: 0,
        cleanup_elapsed_nanoseconds: 0,
        worker_high_water_resident_bytes: 0,
        installed_footprint_bytes: 0,
    };
    let cases = [
        (
            ResourceFacts {
                attempt_elapsed_nanoseconds: 11,
                ..base
            },
            GenerationResourceExceededLimitV1::AttemptElapsed,
        ),
        (
            ResourceFacts {
                first_response_elapsed_nanoseconds: 21,
                ..base
            },
            GenerationResourceExceededLimitV1::FirstResponse,
        ),
        (
            ResourceFacts {
                cleanup_elapsed_nanoseconds: 31,
                ..base
            },
            GenerationResourceExceededLimitV1::Cleanup,
        ),
        (
            ResourceFacts {
                worker_high_water_resident_bytes: 41,
                ..base
            },
            GenerationResourceExceededLimitV1::WorkerHighWaterResident,
        ),
        (
            ResourceFacts {
                installed_footprint_bytes: 51,
                ..base
            },
            GenerationResourceExceededLimitV1::InstalledFootprint,
        ),
    ];
    for (facts, expected) in cases {
        assert_eq!(exceeded_limits(facts, limits()), [expected]);
    }
}

#[test]
fn pure_manifest_kernel_derives_passed_and_failed_from_exact_ordered_evidence() {
    with_scope(|scope| {
        let policy = Digest::sha256(b"pure resource policy");
        let passed = vec![
            evidence("passed-a", exact_facts(), vec![]),
            evidence("passed-b", exact_facts(), vec![]),
        ];
        let manifest = derive_resource_manifest(
            ResourceManifestDerivation {
                scope,
                policy_digest: &policy,
                limits: limits(),
                expected_count: passed.len(),
                evidence: &passed,
            },
            &CancellationToken::new(),
        )
        .expect("exact Passed manifest");
        assert_eq!(
            manifest.status(),
            GenerationQualificationPhaseStatusV1::Passed
        );
        assert_eq!(manifest.phase_policy_digest(), &policy);
        assert_eq!(manifest.evidence_item_count(), 2);

        let failed = vec![
            evidence("failed-a", exact_facts(), vec![]),
            evidence(
                "failed-b",
                ResourceFacts {
                    attempt_elapsed_nanoseconds: 11,
                    ..exact_facts()
                },
                vec![GenerationResourceExceededLimitV1::AttemptElapsed],
            ),
        ];
        let manifest = derive_resource_manifest(
            ResourceManifestDerivation {
                scope,
                policy_digest: &policy,
                limits: limits(),
                expected_count: failed.len(),
                evidence: &failed,
            },
            &CancellationToken::new(),
        )
        .expect("exact Failed manifest");
        assert_eq!(
            manifest.status(),
            GenerationQualificationPhaseStatusV1::Failed
        );
    });
}

#[test]
fn pure_manifest_kernel_rejects_count_order_policy_classification_and_duplicate_roots() {
    with_scope(|scope| {
        let policy = Digest::sha256(b"pure resource policy");
        let exact = vec![evidence("exact", exact_facts(), vec![])];
        for expected_count in [0, 2] {
            assert!(matches!(
                derive_resource_manifest(
                    ResourceManifestDerivation {
                        scope,
                        policy_digest: &policy,
                        limits: limits(),
                        expected_count,
                        evidence: &exact,
                    },
                    &CancellationToken::new(),
                ),
                Err(DerivationError::InvalidCount)
            ));
        }

        for relationship_label in ["reordered result", "foreign policy result"] {
            let mut substituted = evidence(relationship_label, exact_facts(), vec![]);
            substituted.relationships_match = false;
            assert!(matches!(
                derive_resource_manifest(
                    ResourceManifestDerivation {
                        scope,
                        policy_digest: &policy,
                        limits: limits(),
                        expected_count: 1,
                        evidence: &[substituted],
                    },
                    &CancellationToken::new(),
                ),
                Err(DerivationError::Relationship)
            ));
        }

        let misclassified = vec![evidence(
            "classification mismatch",
            ResourceFacts {
                attempt_elapsed_nanoseconds: 11,
                ..exact_facts()
            },
            vec![],
        )];
        assert!(matches!(
            derive_resource_manifest(
                ResourceManifestDerivation {
                    scope,
                    policy_digest: &policy,
                    limits: limits(),
                    expected_count: 1,
                    evidence: &misclassified,
                },
                &CancellationToken::new(),
            ),
            Err(DerivationError::LimitClassification)
        ));

        let duplicate = evidence("duplicate", exact_facts(), vec![]);
        assert!(matches!(
            derive_resource_manifest(
                ResourceManifestDerivation {
                    scope,
                    policy_digest: &policy,
                    limits: limits(),
                    expected_count: 2,
                    evidence: &[duplicate.clone(), duplicate],
                },
                &CancellationToken::new(),
            ),
            Err(DerivationError::Manifest(_))
        ));

        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            derive_resource_manifest(
                ResourceManifestDerivation {
                    scope,
                    policy_digest: &policy,
                    limits: limits(),
                    expected_count: 1,
                    evidence: &exact,
                },
                &cancellation,
            ),
            Err(DerivationError::Cancelled)
        ));
    });
}

#[test]
fn public_compiler_denial_suppresses_derivation_and_still_runs_final_validation() {
    let material = Fixture::judge_pair();
    let authority = complete_qualification_authority(&material);
    let policy = GenerationQualificationPhasePolicyVerifier::verify_resource(
        canonical_resource_policy(),
        authority.operation_policy(),
        &ProductionGenerationQualificationResourcePolicySource::new(),
    )
    .expect("exact source-denied resource policy");
    let error = super::GenerationQualificationResourcePhaseCompiler::compile(
        authority,
        policy,
        &CancellationToken::new(),
    )
    .expect_err("source-denied policy must not derive evidence");
    assert!(matches!(
        error,
        CompilationError::InitialAndFinalAuthority {
            initial: AuthorityError::PolicyDenied,
            final_validation: AuthorityError::PolicyDenied,
        }
    ));
}

#[test]
fn private_real_closure_derivation_compiles_exact_ordered_resource_results() {
    let material = Fixture::judge_pair();
    let mut authority = complete_qualification_authority(&material);
    let baseline = authority
        .operation_policy()
        .baseline_generation_system_id()
        .clone();
    let policy = GenerationQualificationPhasePolicyVerifier::verify_resource(
        canonical_resource_policy(),
        authority.operation_policy(),
        &ProductionGenerationQualificationResourcePolicySource::new(),
    )
    .expect("exact structurally verified resource policy");
    let results = authority
        .collect_resource_results(&baseline, &CancellationToken::new())
        .expect("complete strict resource observations");
    let derived = derive_phase(&authority, &policy, &results, &CancellationToken::new())
        .expect("private pure derivation from real exact closure");
    assert_eq!(derived.results, results);
    assert_eq!(
        derived.manifest.status(),
        GenerationQualificationPhaseStatusV1::Passed
    );
    assert_eq!(derived.manifest.evidence_item_count(), 2);
    let frozen = FrozenResourcePhaseEvidence::from_derived(derived);
    assert_eq!(frozen.resource_results(), results);
    assert_eq!(
        frozen.resource_manifest().status(),
        GenerationQualificationPhaseStatusV1::Passed
    );
    let identical = derive_phase(&authority, &policy, &results, &CancellationToken::new())
        .expect("independently rederived exact resource evidence");
    assert!(frozen.matches(&identical));

    let mut missing = results.clone();
    missing.pop();
    assert!(matches!(
        derive_phase(&authority, &policy, &missing, &CancellationToken::new(),),
        Err(DerivationError::InvalidCount)
    ));

    let mut reordered = results.clone();
    reordered.swap(0, 1);
    assert!(matches!(
        derive_phase(&authority, &policy, &reordered, &CancellationToken::new(),),
        Err(DerivationError::Relationship)
    ));

    let duplicated = vec![results[0].clone(), results[0].clone()];
    assert!(matches!(
        derive_phase(&authority, &policy, &duplicated, &CancellationToken::new(),),
        Err(DerivationError::Relationship)
    ));

    let mut changed_results = frozen.resource_results().to_vec();
    changed_results.pop();
    let changed = super::DerivedResourcePhase {
        results: changed_results,
        manifest: identical.manifest.clone(),
    };
    assert!(!frozen.matches(&changed));
}

fn exact_facts() -> ResourceFacts {
    ResourceFacts {
        attempt_elapsed_nanoseconds: 10,
        first_response_elapsed_nanoseconds: 20,
        cleanup_elapsed_nanoseconds: 30,
        worker_high_water_resident_bytes: 40,
        installed_footprint_bytes: 50,
    }
}

fn evidence(
    label: &str,
    facts: ResourceFacts,
    observed_exceeded: Vec<GenerationResourceExceededLimitV1>,
) -> ResourceEvidenceView {
    ResourceEvidenceView {
        relationships_match: true,
        facts,
        observed_exceeded,
        digest: Digest::sha256(label.as_bytes()),
    }
}

fn with_scope(use_scope: impl FnOnce(GenerationQualificationPhaseScopeV1<'_>)) {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::LicenseRejected,
        |input, _platform, _license, _policy, _production| {
            use_scope(GenerationQualificationPhaseScopeV1 {
                generation_system: input
                    .operation_policy_relations
                    .target_system
                    .generation_system,
                qualification_plan: input.operation_policy_relations.plan,
                suite: input.operation_policy_relations.suite,
            });
        },
    );
}

#[test]
fn maximum_resource_manifest_count_fits_established_store_bound() {
    let material = Fixture::judge_pair();
    let authority = complete_qualification_authority(&material);
    let digests = (0..rewrite_model::MAX_GENERATION_QUALIFICATION_PHASE_ITEMS)
        .map(|index| Digest::sha256(&index.to_be_bytes()))
        .collect::<Vec<_>>();
    let relations = rewrite_model::GenerationResourceEvidenceManifestV1Relations {
        scope: GenerationQualificationPhaseScopeV1 {
            generation_system: authority.target_generation_system(),
            qualification_plan: authority.qualification_plan(),
            suite: authority.suite(),
        },
        phase_policy_digest: authority.operation_policy().resource_policy_digest(),
        evidence_record_digests: &digests,
        status: GenerationQualificationPhaseStatusV1::Passed,
    };
    let manifest = rewrite_model::GenerationResourceEvidenceManifestV1::new(relations)
        .expect("maximum bounded manifest");
    let bytes = serde_json::to_vec(&manifest).expect("canonical manifest");
    assert!(bytes.len() < rewrite_model::MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES);
    assert_eq!(
        rewrite_model::GenerationResourceEvidenceManifestV1::from_json_bytes(&bytes, relations)
            .expect("maximum manifest round trip"),
        manifest
    );
}

pub(crate) fn canonical_resource_policy() -> &'static [u8] {
    concat!(
        "{\"authority\":\"none\",",
        "\"decision_rule\":\"all_complete_target_observations_within_declared_limits\",",
        "\"procedure_id\":\"retonr:generation-qualification-resource-policy:procedure\",",
        "\"procedure_version\":1,",
        "\"measurement_profile\":\"managed_local_generation_v1\",",
        "\"maximum_attempt_elapsed_nanoseconds\":30000000000,",
        "\"maximum_first_response_nanoseconds\":5000000000,",
        "\"maximum_cleanup_nanoseconds\":2000000000,",
        "\"maximum_worker_high_water_resident_bytes\":17179869184,",
        "\"maximum_installed_footprint_bytes\":34359738368,",
        "\"required_provider_observations\":[",
        "\"prompt_token_count\",\"generated_token_count\",",
        "\"total_duration_nanoseconds\",\"load_duration_nanoseconds\",",
        "\"prompt_evaluation_duration_nanoseconds\",",
        "\"evaluation_duration_nanoseconds\"],",
        "\"schema_version\":1}"
    )
    .as_bytes()
}
