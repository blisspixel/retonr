use super::*;

#[test]
fn receipt_preserves_exact_relationships_claims_and_candidate_closure() {
    let fixture = receipt_fixture();
    let receipt = &fixture.receipt;
    assert_eq!(receipt.schema_version(), 1);
    assert_eq!(
        receipt.qualification_plan_id(),
        fixture.bundle.managed.base.plan.qualification_plan_id()
    );
    assert_eq!(
        receipt.suite_manifest_id(),
        fixture.bundle.managed.base.suite.suite_manifest_id()
    );
    assert_eq!(
        receipt.case_id(),
        fixture.bundle.managed.base.cases[0].case_id()
    );
    assert_eq!(
        receipt.cluster_id(),
        fixture.bundle.managed.base.cluster.cluster_id()
    );
    assert_eq!(
        receipt.repetition_id(),
        fixture.bundle.managed.base.repetition.repetition_id()
    );
    assert_eq!(
        receipt.planned_attempt_id(),
        fixture.bundle.managed.base.attempts[0].planned_attempt_id()
    );
    assert_eq!(
        receipt.precursor_id(),
        fixture.bundle.managed.precursor.precursor_id()
    );
    assert_eq!(
        receipt.generation_system_id(),
        fixture.bundle.managed.base.systems[0].generation_system_id()
    );
    assert_eq!(
        receipt.source_digest(),
        fixture.bundle.managed.base.attempts[0].source_digest()
    );
    assert_eq!(
        receipt.case_contract_digest(),
        fixture.bundle.managed.base.attempts[0].case_contract_digest()
    );
    assert_eq!(
        receipt.grounded_request_digest(),
        fixture.bundle.managed.base.attempts[0].grounded_request_digest()
    );
    assert_eq!(
        receipt.generation_request_binding_id(),
        fixture.bundle.managed.base.attempts[0].generation_request_binding_id()
    );
    assert_eq!(
        receipt.structured_request_binding_id(),
        fixture
            .bundle
            .managed
            .precursor
            .structured_request_binding_id()
    );
    assert_eq!(
        receipt.managed_evidence_id(),
        fixture.bundle.managed.evidence.managed_evidence_v2_id()
    );
    assert_eq!(
        receipt.bracket_observation_v1_id(),
        fixture.bundle.managed.evidence.bracket_observation_v1_id()
    );
    assert_eq!(
        receipt.response_id(),
        fixture.bundle.managed.evidence.response_id()
    );
}

#[test]
fn receipt_preserves_runtime_model_cleanup_and_claim_closure() {
    let fixture = receipt_fixture();
    let receipt = &fixture.receipt;
    let precursor = &fixture.bundle.managed.precursor;
    assert_eq!(
        receipt.runtime_admission_join_id(),
        precursor.runtime_admission_join_id()
    );
    assert_eq!(
        receipt.managed_generation_path_id(),
        precursor.managed_generation_path_id()
    );
    assert_eq!(
        receipt.frozen_external_component_set_id(),
        precursor.frozen_external_component_set_id()
    );
    assert_eq!(
        receipt.runtime_package_manifest_id(),
        precursor.runtime_package_manifest_id()
    );
    assert_eq!(receipt.runtime_build_id(), precursor.runtime_build_id());
    assert_eq!(
        receipt.effective_runtime_state_id(),
        precursor.expected_effective_runtime_state_id()
    );
    assert_eq!(
        receipt.effective_runtime_state_join_id(),
        fixture
            .bundle
            .managed
            .evidence
            .effective_runtime_state_join_id()
    );
    assert_eq!(
        receipt.model_artifact_set_id(),
        precursor.model_artifact_set_id()
    );
    assert_eq!(
        receipt.model_package_manifest_id(),
        precursor.model_package_manifest_id()
    );
    assert_eq!(receipt.model_artifact_id(), precursor.model_artifact_id());
    assert_eq!(
        receipt.effective_package_evidence_v2_id(),
        precursor.effective_package_evidence_v2_id()
    );
    assert_eq!(
        receipt.runtime_installation_generation(),
        precursor.runtime_installation_generation()
    );
    assert_eq!(
        receipt.model_installation_generation(),
        precursor.model_installation_generation()
    );
    assert_eq!(
        receipt.static_model_binding_digest(),
        precursor.static_model_binding_digest()
    );
    assert_eq!(
        receipt.candidate_output_contract_digest(),
        fixture.bundle.managed.base.attempts[0].candidate_output_contract_digest()
    );
    assert_eq!(receipt.cleanup_id(), fixture.bundle.cleanup.cleanup_id());
    assert_eq!(
        receipt.bundle_id(),
        fixture.bundle.bundle.evidence_bundle_id()
    );
    assert_eq!(receipt.readback_id(), fixture.readback.readback_id());
    assert_eq!(receipt.candidate_entries(), fixture.bundle.candidates);
    assert_eq!(receipt.usage_observation(), fixture.usage);
    assert_eq!(fixture.usage.input_tokens(), Some(11));
    assert_eq!(fixture.usage.output_tokens(), Some(17));
    assert_eq!(fixture.usage.generation_micros(), Some(23_000));
    assert_eq!(
        receipt.evidence_class(),
        CandidateGenerationReceiptEvidenceClassV1::CleanupAndReadbackVerifiedManagedBracket
    );
    assert!(receipt.model_loaded_proven());
    assert!(!receipt.model_used_proven());
    assert!(!receipt.application_handler_proven());
    assert!(!receipt.formal_placement_proven());
}

#[test]
fn receipt_wire_field_order_and_usage_identity_are_stable() {
    let fixture = receipt_fixture();
    let encoded = String::from_utf8(serde_json::to_vec(&fixture.receipt).expect("receipt JSON"))
        .expect("UTF-8");
    let fields = [
        "schema_version",
        "qualification_plan_id",
        "suite_manifest_id",
        "case_id",
        "cluster_id",
        "repetition_id",
        "planned_attempt_id",
        "precursor_id",
        "generation_system_id",
        "source_digest",
        "case_contract_digest",
        "grounded_request_digest",
        "generation_request_binding_id",
        "structured_request_binding_id",
        "managed_evidence_id",
        "bracket_observation_v1_id",
        "response_id",
        "runtime_admission_join_id",
        "managed_generation_path_id",
        "frozen_external_component_set_id",
        "runtime_package_manifest_id",
        "runtime_build_id",
        "effective_runtime_state_id",
        "effective_runtime_state_join_id",
        "model_artifact_set_id",
        "model_package_manifest_id",
        "model_artifact_id",
        "effective_package_evidence_v2_id",
        "runtime_installation_generation",
        "model_installation_generation",
        "static_model_binding_digest",
        "candidate_output_contract_digest",
        "cleanup_id",
        "bundle_id",
        "readback_id",
        "candidate_entries",
        "usage_observation",
        "evidence_class",
        "model_loaded_proven",
        "model_used_proven",
        "application_handler_proven",
        "formal_placement_proven",
        "qualified",
    ];
    let positions = fields.map(|field| encoded.find(&format!("\"{field}\"")).expect("field"));
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));

    let options = [None, Some(0), Some(u64::MAX)];
    let mut ids = std::collections::HashSet::new();
    for input_tokens in options {
        for output_tokens in options {
            for generation_micros in options {
                let usage = CandidateGenerationUsageObservationV1::new(
                    input_tokens,
                    output_tokens,
                    generation_micros,
                );
                let receipt = CandidateGenerationReceiptV1::new(fixture.relations(), usage)
                    .expect("usage receipt");
                assert!(ids.insert(receipt.receipt_id().clone()));
            }
        }
    }
    assert_eq!(ids.len(), 27);
}

#[test]
fn receipt_codec_and_debug_are_canonical_and_redacted() {
    let fixture = receipt_fixture();
    let receipt = &fixture.receipt;
    let encoded = serde_json::to_vec(receipt).expect("receipt JSON");
    assert!(
        String::from_utf8(encoded.clone())
            .expect("UTF-8")
            .contains("\"qualified\":false")
    );
    assert_eq!(
        CandidateGenerationReceiptV1::from_json_bytes(&encoded, fixture.relations(), fixture.usage)
            .expect("receipt decode"),
        *receipt,
    );
    let debug = format!("{receipt:?}");
    assert!(debug.contains(receipt.receipt_id().digest().as_str()));
    assert!(!debug.contains(receipt.response_id().digest().as_str()));
    assert!(
        !debug.contains(
            receipt.candidate_entries()[0]
                .artifact_id()
                .digest()
                .as_str()
        )
    );
}
