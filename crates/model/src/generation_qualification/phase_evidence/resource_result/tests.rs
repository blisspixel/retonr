use serde_json::Value;

use super::*;
use crate::generation_qualification::{
    GenerationQualificationPhaseStatusV1, GenerationResourceEvidenceManifestV1,
    GenerationResourceEvidenceManifestV1Relations,
};

mod codec;
mod relations;
mod support;

use support::{fixture, input};

#[test]
fn result_round_trips_exposes_exact_facts_and_has_frozen_vectors() {
    let fixture = fixture();
    let input = input();
    let relations = fixture.relations();
    let result = GenerationResourceAttemptResultRecordV1::new(relations, input.clone())
        .expect("resource result");
    result
        .validate_against(relations, &input)
        .expect("exact revalidation");
    let encoded = serde_json::to_vec(&result).expect("resource JSON");
    assert_eq!(
        GenerationResourceAttemptResultRecordV1::from_json_bytes(&encoded, relations, &input)
            .expect("resource decode"),
        result
    );
    assert_public_facts(&result, &fixture);
    assert_frozen_vectors(&result, encoded);
}

fn assert_public_facts(
    result: &GenerationResourceAttemptResultRecordV1,
    fixture: &support::ResourceFixture,
) {
    assert_eq!(result.schema_version(), 1);
    assert_eq!(
        result.generation_system_id(),
        fixture.base.systems[0].generation_system_id()
    );
    assert_eq!(
        result.generation_qualification_plan_id(),
        fixture.base.plan.qualification_plan_id()
    );
    assert_eq!(
        result.suite_manifest_id(),
        fixture.base.suite.suite_manifest_id()
    );
    assert_eq!(result.case_id(), fixture.base.cases[0].case_id());
    assert_eq!(
        result.repetition_id(),
        fixture.base.repetitions[0].repetition_id()
    );
    assert_eq!(
        result.planned_attempt_id(),
        fixture.base.attempts[0].planned_attempt_id()
    );
    assert_eq!(
        result.attempt_record_id(),
        fixture.attempt_record.attempt_record_id()
    );
    assert_eq!(
        result.candidate_generation_receipt_id(),
        fixture.receipt.receipt_id()
    );
    assert_eq!(
        result.resource_policy_digest(),
        fixture.base.policy.resource_policy_digest()
    );
    assert_eq!(
        result.observation_profile(),
        GenerationResourceObservationProfileV1::ManagedOllamaV0_32_15LinuxV1
    );
    assert_eq!(result.prompt_token_count(), 11);
    assert_eq!(result.generated_token_count(), 17);
    assert_eq!(result.total_duration_nanoseconds(), 30_000_000);
    assert_eq!(result.load_duration_nanoseconds(), 1_000_000);
    assert_eq!(result.prompt_evaluation_duration_nanoseconds(), 2_000_000);
    assert_eq!(result.evaluation_duration_nanoseconds(), 23_000_999);
    assert_eq!(result.attempt_elapsed_nanoseconds(), 40_000_000);
    assert_eq!(result.first_response_elapsed_nanoseconds(), 5_000_000);
    assert_eq!(result.cleanup_elapsed_nanoseconds(), 4_000_000);
    assert_eq!(result.worker_high_water_resident_bytes(), 8_192);
    assert_eq!(result.runtime_installed_payload_bytes(), 100);
    assert_eq!(result.model_installed_payload_bytes(), 900);
    assert_eq!(result.installed_footprint_bytes(), 1_000);
    assert_eq!(
        result.exceeded_limits(),
        [
            GenerationResourceExceededLimitV1::FirstResponse,
            GenerationResourceExceededLimitV1::WorkerHighWaterResident,
        ]
    );
}

fn assert_frozen_vectors(result: &GenerationResourceAttemptResultRecordV1, encoded: Vec<u8>) {
    assert_eq!(
        GENERATION_RESOURCE_ATTEMPT_RESULT_ID_DOMAIN,
        b"retonr:generation-resource-attempt-result:v1\0"
    );
    assert_eq!(
        result.resource_attempt_result_id().digest().as_str(),
        "b371a9820f0fd4c8b1ddbedbfdbb6e1bf5f97091a2a7ecb50f9e6fdf76c11b45"
    );
    assert_eq!(
        String::from_utf8(encoded).expect("UTF-8"),
        concat!(
            "{\"schema_version\":1,",
            "\"generation_system_id\":\"ab2764a9d570042750de53003b99f61b90d02d7bd7645f24fa247d9b7487b2fc\",",
            "\"generation_qualification_plan_id\":\"238fb5e32a955cd840ea8841e26b56f0047ac13242d129f5ad52aa52302a8763\",",
            "\"suite_manifest_id\":\"5571aa407686da0063ff23c876f5dc71eb288bfc89e20772460851aca44343b1\",",
            "\"case_id\":\"064af0c3b69b9053a0f89c58061b71a91ce37b35db86bdb8dea219bf481b4c00\",",
            "\"repetition_id\":\"86ec71e094c2e86e9b95db8e47a808fc6da5a5f55f90e98508901eeea0b65dda\",",
            "\"planned_attempt_id\":\"81c2a1cfd2b3011c0df5cd5d2849a458cd04d123f5a9d5d19fcfe1b8112ce3fd\",",
            "\"attempt_record_id\":\"e07331fc8670ba8c47ab7e72f60df3a06d13d9c3cd5fb3600f40ba4d2a8e22d4\",",
            "\"candidate_generation_receipt_id\":\"4dc1b11744dcabb81cbe85cd1cc32491e0b52e667be27e1dc624ea392174d304\",",
            "\"resource_policy_digest\":\"7f46de7c2d1a6137c5b61d929c0340223f7bbcf60dc2ad7a1f014f176c517e72\",",
            "\"observation_profile\":\"managed_ollama_v0_32_15_linux_v1\",",
            "\"prompt_token_count\":11,\"generated_token_count\":17,",
            "\"total_duration_nanoseconds\":30000000,\"load_duration_nanoseconds\":1000000,",
            "\"prompt_evaluation_duration_nanoseconds\":2000000,",
            "\"evaluation_duration_nanoseconds\":23000999,",
            "\"attempt_elapsed_nanoseconds\":40000000,",
            "\"first_response_elapsed_nanoseconds\":5000000,",
            "\"cleanup_elapsed_nanoseconds\":4000000,",
            "\"worker_high_water_resident_bytes\":8192,",
            "\"runtime_installed_payload_bytes\":100,",
            "\"model_installed_payload_bytes\":900,",
            "\"installed_footprint_bytes\":1000,",
            "\"exceeded_limits\":[\"first_response\",\"worker_high_water_resident\"]}"
        )
    );
}

#[test]
fn result_id_is_valid_manifest_input_but_does_not_make_status_authoritative() {
    let fixture = fixture();
    let input = input();
    let result = GenerationResourceAttemptResultRecordV1::new(fixture.relations(), input)
        .expect("resource result");
    let digests = [result.resource_attempt_result_id().digest().clone()];
    let manifest =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: fixture.base.policy.resource_policy_digest(),
            evidence_record_digests: &digests,
            status: GenerationQualificationPhaseStatusV1::Passed,
        })
        .expect("inert manifest");
    let failed =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            status: GenerationQualificationPhaseStatusV1::Failed,
            scope: fixture.scope(),
            phase_policy_digest: fixture.base.policy.resource_policy_digest(),
            evidence_record_digests: &digests,
        })
        .expect("same evidence admits inert failed status");
    assert_eq!(manifest.evidence_item_count(), 1);
    assert_eq!(failed.evidence_item_count(), 1);
    assert_ne!(manifest.status(), failed.status());
    assert_ne!(
        manifest.resource_evidence_manifest_id(),
        failed.resource_evidence_manifest_id()
    );
}

#[test]
fn debug_and_errors_do_not_disclose_observations_or_relationship_digests() {
    let fixture = fixture();
    let input = input();
    let result = GenerationResourceAttemptResultRecordV1::new(fixture.relations(), input.clone())
        .expect("resource result");
    let debug = format!("{result:?}");
    assert!(debug.contains(result.resource_attempt_result_id().digest().as_str()));
    for hidden in [
        input.worker_high_water_resident_bytes.to_string(),
        input.installed_footprint_bytes.to_string(),
        result.resource_policy_digest().as_str().to_owned(),
        result.generation_system_id().digest().as_str().to_owned(),
    ] {
        assert!(!debug.contains(&hidden));
    }
    let error =
        GenerationResourceAttemptResultRecordV1::from_json_bytes(b"{", fixture.relations(), &input)
            .expect_err("malformed record");
    assert_eq!(
        error.to_string(),
        "generation qualification phase evidence encoding is invalid"
    );
}

fn substitute(encoded: &[u8], field: &str, replacement: Value) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(encoded).expect("JSON value");
    value[field] = replacement;
    serde_json::to_vec(&value).expect("substituted JSON")
}
