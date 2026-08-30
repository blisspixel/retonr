use rewrite_types::{Digest, ReasonCode, RewriteStatus};

use super::*;
use crate::{GenerationCaseManifestV1Input, GenerationClusterRecordV1};

const SOURCE: &[u8] = b"Retain Acme 42 exactly.";
const FROZEN_DIGEST: &str = "7f9fb8014389dbcf419d8a8a63e2ef34c761d78ce0cc33a333fe9427f0cef81a";
const FROZEN_JSON: &str = concat!(
    "{\"schema_version\":1,",
    "\"case_key\":\"protected-literal\",",
    "\"source_artifact_id\":\"4ccf56e0cef12eead67cc93cacbcf27b7f8aef84268acfbe0156af97560b1d51\",",
    "\"source_digest\":\"4ccf56e0cef12eead67cc93cacbcf27b7f8aef84268acfbe0156af97560b1d51\",",
    "\"source_byte_count\":23,",
    "\"language_digest\":\"a4ef304ba42a200bafd78b046e0869af9183f6eee5524aead5dcb3a5ab5f8f3f\",",
    "\"mode_digest\":\"e642b12901a6ee51456f654c48cd0aa6e90afd64e035afbc97cfc542209c70f9\",",
    "\"format_digest\":\"e904c9ccfa425ff0b055d2c533462314d35a529b055e8abe41d49bb46d827427\",",
    "\"evaluation_category\":\"protected_value_negative\",",
    "\"protected_terms\":[\"42\",\"Acme\"],",
    "\"reference_judgment\":\"unacceptable\",",
    "\"expected_status\":\"abstained\",",
    "\"expected_reason\":\"protected_value_changed\",",
    "\"expected_output\":\"source\",",
    "\"rubric_clause_ids\":[\"fidelity\",\"protected-values\"]}"
);

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn input() -> GenerationDeterministicCaseContractV1Input {
    let source_digest = Digest::sha256(SOURCE);
    GenerationDeterministicCaseContractV1Input {
        case_key: "protected-literal".to_owned(),
        source_artifact_id: ArtifactId::from_digest(source_digest.clone()),
        source_digest,
        source_byte_count: SOURCE.len() as u64,
        language_digest: digest("language"),
        mode_digest: digest("mode"),
        format_digest: digest("format"),
        evaluation_category: "protected_value_negative".to_owned(),
        protected_terms: vec!["42".to_owned(), "Acme".to_owned()],
        reference_judgment: ReferenceJudgment::Unacceptable,
        expected_status: RewriteStatus::Abstained,
        expected_reason: Some(ReasonCode::ProtectedValueChanged),
        expected_output: ExpectedOutput::Source,
        rubric_clause_ids: vec!["fidelity".to_owned(), "protected-values".to_owned()],
    }
}

fn case_manifest(contract: &GenerationDeterministicCaseContractV1) -> GenerationCaseManifestV1 {
    let cluster = GenerationClusterRecordV1::new("literal", digest("cluster policy"))
        .expect("cluster is valid");
    GenerationCaseManifestV1::new(
        &cluster,
        GenerationCaseManifestV1Input {
            case_key: contract.case_key().to_owned(),
            source_artifact_id: contract.source_artifact_id().clone(),
            source_digest: contract.source_digest().clone(),
            source_byte_count: contract.source_byte_count(),
            case_contract_digest: contract.contract_digest().clone(),
            language_digest: contract.language_digest().clone(),
            mode_digest: contract.mode_digest().clone(),
            format_digest: contract.format_digest().clone(),
        },
    )
    .expect("case is valid")
}

#[test]
fn moved_contract_preserves_frozen_identity_and_canonical_json() {
    let contract = GenerationDeterministicCaseContractV1::new(input()).expect("contract is valid");
    assert_eq!(contract.contract_digest().as_str(), FROZEN_DIGEST);
    assert_eq!(
        contract
            .to_canonical_json_bytes()
            .expect("contract encodes"),
        FROZEN_JSON.as_bytes()
    );
}

#[test]
fn moved_contract_round_trips_and_preserves_projection_behavior() {
    let contract = GenerationDeterministicCaseContractV1::new(input()).expect("contract is valid");
    let case = case_manifest(&contract);
    let decoded =
        GenerationDeterministicCaseContractV1::from_json_bytes(FROZEN_JSON.as_bytes(), &case)
            .expect("frozen contract decodes");
    assert_eq!(decoded, contract);

    let projected = decoded
        .project_evaluation_case(SOURCE, "Retain Acme 43 exactly.")
        .expect("source projects");
    assert_eq!(projected.id, "protected-literal");
    assert_eq!(
        projected.reference_judgment,
        ReferenceJudgment::Unacceptable
    );
    assert_eq!(projected.expected_output, ExpectedOutput::Source);
}

#[test]
fn moved_contract_keeps_strict_canonical_decoder() {
    let contract = GenerationDeterministicCaseContractV1::new(input()).expect("contract is valid");
    let case = case_manifest(&contract);
    let reordered = FROZEN_JSON.replacen(
        "{\"schema_version\":1,\"case_key\":\"protected-literal\"",
        "{\"case_key\":\"protected-literal\",\"schema_version\":1",
        1,
    );
    assert_eq!(
        GenerationDeterministicCaseContractV1::from_json_bytes(reordered.as_bytes(), &case),
        Err(GenerationDeterministicCaseContractError::NonCanonicalEncoding)
    );
}
