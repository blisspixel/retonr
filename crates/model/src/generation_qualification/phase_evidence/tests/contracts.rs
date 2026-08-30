use super::*;

#[test]
fn four_manifests_and_terminal_result_round_trip_with_frozen_ids() {
    let fixture = fixture();
    let ledger_relations = fixture.ledger_relations(
        &fixture.target_records,
        GenerationQualificationPhaseStatusV1::Failed,
    );
    let ledger = GenerationAttemptLedgerManifestV1::new(ledger_relations).expect("ledger");
    ledger
        .validate_against(ledger_relations)
        .expect("revalidate");
    let terminal_digest = digest("candidate generation terminal evidence");
    let result_relations = GenerationRepeatabilityResultRecordV1Relations {
        scope: fixture.scope(),
        repetition: &fixture.repetitions[0],
        attempt_ledger: &ledger,
        attempt_ledger_relations: ledger_relations,
        terminal_stage: GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
        candidate_receipt_set: None,
        deterministic_evaluation: None,
        candidate_judge_join: None,
        terminal_evidence_digest: &terminal_digest,
    };
    let result = GenerationRepeatabilityResultRecordV1::new(result_relations).expect("result");
    let results = vec![result.clone()];
    let repeat_policy = policy("repeatability");
    let repeat_relations = GenerationRepeatabilityEvidenceManifestV1Relations {
        scope: fixture.scope(),
        phase_policy_digest: &repeat_policy,
        planned_attempts: &fixture.planned,
        preregistered_repetitions: &fixture.repetitions,
        results: &results,
        status: GenerationQualificationPhaseStatusV1::Failed,
    };
    let repeat =
        GenerationRepeatabilityEvidenceManifestV1::new(repeat_relations).expect("repeatability");
    let resource_digests = [digest("resource one"), digest("resource two")];
    let resource_policy = policy("resource");
    let resource_relations = GenerationResourceEvidenceManifestV1Relations {
        scope: fixture.scope(),
        phase_policy_digest: &resource_policy,
        evidence_record_digests: &resource_digests,
        status: GenerationQualificationPhaseStatusV1::Passed,
    };
    let resource = GenerationResourceEvidenceManifestV1::new(resource_relations).expect("resource");
    let human_digests = [digest("human one")];
    let human_policy = policy("human");
    let human_relations = GenerationHumanAdjudicationEvidenceManifestV1Relations {
        scope: fixture.scope(),
        phase_policy_digest: &human_policy,
        evidence_record_digests: &human_digests,
        status: GenerationQualificationPhaseStatusV1::Failed,
    };
    let human = GenerationHumanAdjudicationEvidenceManifestV1::new(human_relations).expect("human");

    assert_frozen_ids(&ledger, &result, &repeat, &resource, &human);
    assert_public_projections(&ledger, &result, &repeat, &resource, &human);
    assert_manifest_round_trips(&ledger, ledger_relations, &repeat, repeat_relations);
    assert_record_round_trips(
        &result,
        result_relations,
        &resource,
        resource_relations,
        &human,
        human_relations,
    );
}

#[test]
fn target_ledger_rejects_mixed_system_records_and_reordered_plan_closure() {
    let fixture = fixture();
    let mixed = fixture.ledger_relations(
        &fixture.baseline_records,
        GenerationQualificationPhaseStatusV1::Failed,
    );
    assert_eq!(
        GenerationAttemptLedgerManifestV1::new(mixed),
        Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
    );
    let mut reordered = fixture.planned.clone();
    reordered.swap(0, 1);
    let relations = GenerationAttemptLedgerManifestV1Relations {
        planned_attempts: &reordered,
        ..fixture.ledger_relations(
            &fixture.target_records,
            GenerationQualificationPhaseStatusV1::Failed,
        )
    };
    assert_eq!(
        GenerationAttemptLedgerManifestV1::new(relations),
        Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
    );
}

#[test]
fn repeatability_rejects_foreign_duplicate_reordered_and_mixed_scope_repetitions() {
    let fixture = fixture();
    let ledger_relations = fixture.ledger_relations(
        &fixture.target_records,
        GenerationQualificationPhaseStatusV1::Failed,
    );
    let ledger = GenerationAttemptLedgerManifestV1::new(ledger_relations).expect("ledger");
    let terminal_digest = digest("terminal");
    let result = GenerationRepeatabilityResultRecordV1::new(
        GenerationRepeatabilityResultRecordV1Relations {
            scope: fixture.scope(),
            repetition: &fixture.repetitions[0],
            attempt_ledger: &ledger,
            attempt_ledger_relations: ledger_relations,
            terminal_stage: GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
            candidate_receipt_set: None,
            deterministic_evaluation: None,
            candidate_judge_join: None,
            terminal_evidence_digest: &terminal_digest,
        },
    )
    .expect("result");
    let results = [result];
    let repeat_policy = policy("repeat");
    let base = GenerationRepeatabilityEvidenceManifestV1Relations {
        scope: fixture.scope(),
        phase_policy_digest: &repeat_policy,
        planned_attempts: &fixture.planned,
        preregistered_repetitions: &fixture.repetitions,
        results: &results,
        status: GenerationQualificationPhaseStatusV1::Failed,
    };
    GenerationRepeatabilityEvidenceManifestV1::new(base).expect("valid repeatability");

    let foreign = GenerationRepetitionRecordV1::new(&fixture.suite, 2, digest("foreign"))
        .expect("foreign repetition");
    for repetitions in [
        vec![fixture.repetitions[0].clone(), foreign],
        vec![
            fixture.repetitions[0].clone(),
            fixture.repetitions[0].clone(),
        ],
        vec![
            fixture.repetitions[1].clone(),
            fixture.repetitions[0].clone(),
        ],
    ] {
        assert_eq!(
            GenerationRepeatabilityEvidenceManifestV1::new(
                GenerationRepeatabilityEvidenceManifestV1Relations {
                    preregistered_repetitions: &repetitions,
                    ..base
                }
            ),
            Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
        );
    }
    let baseline_scope = GenerationQualificationPhaseScopeV1 {
        generation_system: &fixture.systems[1],
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
    };
    assert_eq!(
        GenerationRepeatabilityEvidenceManifestV1::new(
            GenerationRepeatabilityEvidenceManifestV1Relations {
                scope: baseline_scope,
                ..base
            }
        ),
        Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
    );
}

#[test]
fn digest_manifests_enforce_empty_status_duplicates_order_and_strict_json() {
    let fixture = fixture();
    let values = [digest("a"), digest("b")];
    let resource_policy = policy("resource policy");
    let passed = GenerationResourceEvidenceManifestV1Relations {
        scope: fixture.scope(),
        phase_policy_digest: &resource_policy,
        evidence_record_digests: &values,
        status: GenerationQualificationPhaseStatusV1::Passed,
    };
    let first = GenerationResourceEvidenceManifestV1::new(passed).expect("resource");
    let reversed_values = [values[1].clone(), values[0].clone()];
    let reordered =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            evidence_record_digests: &reversed_values,
            ..passed
        })
        .expect("reordered resource");
    assert_ne!(
        first.resource_evidence_manifest_id(),
        reordered.resource_evidence_manifest_id()
    );
    let duplicate_values = [values[0].clone(), values[0].clone()];
    assert_eq!(
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            evidence_record_digests: &duplicate_values,
            ..passed
        }),
        Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
    );
    assert_eq!(
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            evidence_record_digests: &[],
            ..passed
        }),
        Err(GenerationQualificationPhaseEvidenceError::InvalidCount)
    );
    GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
        evidence_record_digests: &[],
        status: GenerationQualificationPhaseStatusV1::Skipped,
        ..passed
    })
    .expect("skipped empty root");

    let bytes = serde_json::to_vec(&first).expect("JSON");
    let mut stale: Value = serde_json::from_slice(&bytes).expect("value");
    stale["evidence_root_digest"] = Value::String(digest("substitute").as_str().to_owned());
    assert_eq!(
        GenerationResourceEvidenceManifestV1::from_json_bytes(
            &serde_json::to_vec(&stale).expect("stale JSON"),
            passed,
        ),
        Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
    );
    let mut unknown = bytes.clone();
    unknown.pop();
    unknown.extend_from_slice(b",\"unknown\":0}");
    assert_eq!(
        GenerationResourceEvidenceManifestV1::from_json_bytes(&unknown, passed),
        Err(GenerationQualificationPhaseEvidenceError::InvalidEncoding)
    );
    assert_eq!(
        GenerationResourceEvidenceManifestV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES + 1],
            passed,
        ),
        Err(GenerationQualificationPhaseEvidenceError::EncodedRecordTooLarge)
    );
}

#[test]
fn every_relation_slice_rejects_max_plus_one_before_relationship_work() {
    let fixture = fixture();
    let oversized_digests =
        vec![digest("bounded evidence"); MAX_GENERATION_QUALIFICATION_PHASE_ITEMS + 1];
    assert_eq!(
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &policy("resource"),
            evidence_record_digests: &oversized_digests,
            status: GenerationQualificationPhaseStatusV1::Passed,
        }),
        Err(GenerationQualificationPhaseEvidenceError::InvalidCount)
    );

    let oversized_planned =
        vec![fixture.planned[0].clone(); MAX_GENERATION_QUALIFICATION_PHASE_ITEMS + 1];
    assert_eq!(
        GenerationAttemptLedgerManifestV1::new(GenerationAttemptLedgerManifestV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &policy("ledger"),
            planned_attempts: &oversized_planned,
            attempt_records: &[],
            status: GenerationQualificationPhaseStatusV1::Skipped,
        }),
        Err(GenerationQualificationPhaseEvidenceError::InvalidCount)
    );
    let oversized_records =
        vec![fixture.target_records[0].clone(); MAX_GENERATION_QUALIFICATION_PHASE_ITEMS + 1];
    assert_eq!(
        GenerationAttemptLedgerManifestV1::new(GenerationAttemptLedgerManifestV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &policy("ledger"),
            planned_attempts: &fixture.planned,
            attempt_records: &oversized_records,
            status: GenerationQualificationPhaseStatusV1::Failed,
        }),
        Err(GenerationQualificationPhaseEvidenceError::InvalidCount)
    );
    assert_eq!(
        GenerationRepeatabilityEvidenceManifestV1::new(
            GenerationRepeatabilityEvidenceManifestV1Relations {
                scope: fixture.scope(),
                phase_policy_digest: &policy("repeatability"),
                planned_attempts: &oversized_planned,
                preregistered_repetitions: &fixture.repetitions,
                results: &[],
                status: GenerationQualificationPhaseStatusV1::Skipped,
            }
        ),
        Err(GenerationQualificationPhaseEvidenceError::InvalidCount)
    );
    let oversized_repetitions =
        vec![fixture.repetitions[0].clone(); MAX_GENERATION_QUALIFICATION_PHASE_ITEMS + 1];
    assert_eq!(
        GenerationRepeatabilityEvidenceManifestV1::new(
            GenerationRepeatabilityEvidenceManifestV1Relations {
                scope: fixture.scope(),
                phase_policy_digest: &policy("repeatability"),
                planned_attempts: &fixture.planned,
                preregistered_repetitions: &oversized_repetitions,
                results: &[],
                status: GenerationQualificationPhaseStatusV1::Skipped,
            }
        ),
        Err(GenerationQualificationPhaseEvidenceError::InvalidCount)
    );
}

#[test]
fn errors_and_debug_are_strictly_content_free() {
    let fixture = fixture();
    let ledger = GenerationAttemptLedgerManifestV1::new(fixture.ledger_relations(
        &fixture.target_records,
        GenerationQualificationPhaseStatusV1::Failed,
    ))
    .expect("ledger");
    let rendered = format!("{ledger:?}");
    assert!(!rendered.contains("phase source"));
    assert!(!rendered.contains("RequestInvalid"));
    assert_eq!(
        GenerationQualificationPhaseEvidenceError::StatusMismatch.to_string(),
        "generation qualification phase evidence status does not match"
    );
    assert_eq!(fixture.cluster.cluster_key(), "phase-case");
    assert_eq!(fixture.case.case_key(), "phase-case");
}

#[test]
fn phase_domains_and_empty_roots_are_distinct_frozen_vectors() {
    let fixture = fixture();
    let shared_policy = digest("shared phase policy");
    let shared_records = [digest("shared evidence")];
    let resource =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &shared_policy,
            evidence_record_digests: &shared_records,
            status: GenerationQualificationPhaseStatusV1::Passed,
        })
        .expect("resource");
    let human = GenerationHumanAdjudicationEvidenceManifestV1::new(
        GenerationHumanAdjudicationEvidenceManifestV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &shared_policy,
            evidence_record_digests: &shared_records,
            status: GenerationQualificationPhaseStatusV1::Passed,
        },
    )
    .expect("human");
    assert_ne!(
        resource.resource_evidence_manifest_id().digest(),
        human.human_adjudication_evidence_manifest_id().digest()
    );
    assert_ne!(
        resource.evidence_root_digest(),
        human.evidence_root_digest()
    );

    let ledger = GenerationAttemptLedgerManifestV1::new(
        fixture.ledger_relations(&[], GenerationQualificationPhaseStatusV1::Skipped),
    )
    .expect("skipped ledger");
    let repeat = GenerationRepeatabilityEvidenceManifestV1::new(
        GenerationRepeatabilityEvidenceManifestV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &shared_policy,
            planned_attempts: &fixture.planned,
            preregistered_repetitions: &fixture.repetitions,
            results: &[],
            status: GenerationQualificationPhaseStatusV1::Skipped,
        },
    )
    .expect("skipped repeatability");
    let skipped_resource =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &shared_policy,
            evidence_record_digests: &[],
            status: GenerationQualificationPhaseStatusV1::Skipped,
        })
        .expect("skipped resource");
    let skipped_human = GenerationHumanAdjudicationEvidenceManifestV1::new(
        GenerationHumanAdjudicationEvidenceManifestV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &shared_policy,
            evidence_record_digests: &[],
            status: GenerationQualificationPhaseStatusV1::Skipped,
        },
    )
    .expect("skipped human");
    assert_eq!(
        [
            ledger.evidence_root_digest().as_str(),
            repeat.evidence_root_digest().as_str(),
            skipped_resource.evidence_root_digest().as_str(),
            skipped_human.evidence_root_digest().as_str(),
        ],
        [
            "6a300c81740b1434b8767793c73d5f91d5ca84c429318e74f492a648c806ec53",
            "39a46a4bd0609964e941473422d181e3c28d0417b21f5225197ab27d14a0cf95",
            "607a55777a8f1f758f974ab3f4cf10a9e0db0e6ca91724d94d92b25ce2b0dc38",
            "9345cf556f79dfcdab5f7ef451520fb1f79bcf285f619ba38c0082e943741d55",
        ]
    );
    assert_eq!(
        String::from_utf8(serde_json::to_vec(&skipped_resource).expect("canonical JSON"))
            .expect("UTF-8 JSON"),
        concat!(
            "{\"schema_version\":1,",
            "\"generation_system_id\":\"6855cc4c5336d585c6e64d660b4ba73ba5cf9d23b902cad61e586d0c8d8fb8ab\",",
            "\"generation_qualification_plan_id\":\"9795268aab74e0ce006a64e5236b654cd104403e03c67c7739a3db387886dc1c\",",
            "\"suite_manifest_id\":\"04a7cd0a4e6032cbda9edfd0c278dc79392e1728ff2e18fe316878e451f2cd1a\",",
            "\"phase_policy_digest\":\"a8f9cc15792f8518b8520beb257b589c6de2a74aa75848f5fd4b795b0c55f98a\",",
            "\"evidence_root_digest\":\"607a55777a8f1f758f974ab3f4cf10a9e0db0e6ca91724d94d92b25ce2b0dc38\",",
            "\"evidence_item_count\":0,\"status\":\"skipped\"}"
        )
    );
}
