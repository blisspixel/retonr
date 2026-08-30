use rewrite_model::{
    ArtifactId, GenerationCaseManifestV1, GenerationCaseManifestV1Input, GenerationClusterRecordV1,
};
use rewrite_types::{Digest, ReasonCode, RewriteStatus};
use serde_json::Value;

use super::*;

const SOURCE: &[u8] = b"Retain Acme 42 exactly.";

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
fn round_trips_canonical_bytes_and_projects_every_suite_field() {
    let contract = GenerationDeterministicCaseContractV1::new(input()).expect("contract is valid");
    let case = case_manifest(&contract);
    contract
        .validate_case_manifest(&case)
        .expect("manifest matches");
    let bytes = contract
        .to_canonical_json_bytes()
        .expect("contract encodes");
    let decoded = GenerationDeterministicCaseContractV1::from_json_bytes(&bytes, &case)
        .expect("contract decodes");
    assert_eq!(decoded, contract);

    let projected = decoded
        .project_evaluation_case(SOURCE, "Retain Acme 43 exactly.")
        .expect("source projects");
    assert_eq!(projected.id, decoded.case_key());
    assert_eq!(projected.category, decoded.evaluation_category());
    assert_eq!(projected.source.as_bytes(), SOURCE);
    assert_eq!(projected.candidate, "Retain Acme 43 exactly.");
    assert_eq!(projected.protected_terms, decoded.protected_terms());
    assert_eq!(projected.reference_judgment, decoded.reference_judgment());
    assert_eq!(projected.expected_status, decoded.expected_status());
    assert_eq!(projected.expected_reason, decoded.expected_reason());
    assert_eq!(projected.expected_output, decoded.expected_output());
}

#[test]
fn digest_is_stable_and_sensitive_to_every_contract_field() {
    let contract = GenerationDeterministicCaseContractV1::new(input()).expect("contract is valid");
    assert_eq!(
        contract.contract_digest().as_str(),
        "7f9fb8014389dbcf419d8a8a63e2ef34c761d78ce0cc33a333fe9427f0cef81a"
    );

    let original = input();
    let mut mutations = Vec::new();
    let mut value = original.clone();
    value.case_key = "protected-literal-two".to_owned();
    mutations.push(value);
    let mut value = original.clone();
    value.source_digest = digest("other source");
    value.source_artifact_id = ArtifactId::from_digest(value.source_digest.clone());
    mutations.push(value);
    let mut value = original.clone();
    value.source_byte_count += 1;
    mutations.push(value);
    let mut value = original.clone();
    value.language_digest = digest("other language");
    mutations.push(value);
    let mut value = original.clone();
    value.mode_digest = digest("other mode");
    mutations.push(value);
    let mut value = original.clone();
    value.format_digest = digest("other format");
    mutations.push(value);
    let mut value = original.clone();
    value.evaluation_category = "other_category".to_owned();
    mutations.push(value);
    let mut value = original.clone();
    value.protected_terms.push("Zulu".to_owned());
    mutations.push(value);
    let mut value = original.clone();
    value.reference_judgment = ReferenceJudgment::Acceptable;
    mutations.push(value);
    let mut value = original.clone();
    value.expected_status = RewriteStatus::Failed;
    mutations.push(value);
    let mut value = original.clone();
    value.expected_reason = None;
    mutations.push(value);
    let mut value = original.clone();
    value.expected_output = ExpectedOutput::Candidate;
    mutations.push(value);
    let mut value = original;
    value.rubric_clause_ids.push("structure".to_owned());
    mutations.push(value);

    for mutation in mutations {
        let changed = GenerationDeterministicCaseContractV1::new(mutation)
            .expect("mutation remains structurally valid");
        assert_ne!(changed.contract_digest(), contract.contract_digest());
    }
}

#[test]
fn rejects_unknown_duplicate_missing_reordered_trailing_and_unsupported_json() {
    let contract = GenerationDeterministicCaseContractV1::new(input()).expect("contract is valid");
    let case = case_manifest(&contract);
    let bytes = contract
        .to_canonical_json_bytes()
        .expect("contract encodes");
    let mut value: Value = serde_json::from_slice(&bytes).expect("JSON value decodes");
    value
        .as_object_mut()
        .expect("record is an object")
        .insert("unknown".to_owned(), Value::Bool(true));
    assert_eq!(
        GenerationDeterministicCaseContractV1::from_json_bytes(
            &serde_json::to_vec(&value).expect("value encodes"),
            &case,
        ),
        Err(GenerationDeterministicCaseContractError::InvalidEncoding)
    );

    let text = std::str::from_utf8(&bytes).expect("canonical JSON is UTF-8");
    let duplicate = text.replacen(
        "{\"schema_version\":1,",
        "{\"schema_version\":1,\"schema_version\":1,",
        1,
    );
    assert_eq!(
        GenerationDeterministicCaseContractV1::from_json_bytes(duplicate.as_bytes(), &case),
        Err(GenerationDeterministicCaseContractError::InvalidEncoding)
    );
    let missing = text.replacen("\"schema_version\":1,", "", 1);
    assert_eq!(
        GenerationDeterministicCaseContractV1::from_json_bytes(missing.as_bytes(), &case),
        Err(GenerationDeterministicCaseContractError::InvalidEncoding)
    );
    let reordered = text.replacen(
        "{\"schema_version\":1,\"case_key\":\"protected-literal\"",
        "{\"case_key\":\"protected-literal\",\"schema_version\":1",
        1,
    );
    assert_eq!(
        GenerationDeterministicCaseContractV1::from_json_bytes(reordered.as_bytes(), &case),
        Err(GenerationDeterministicCaseContractError::NonCanonicalEncoding)
    );
    let trailing = [bytes.as_slice(), b"\n"].concat();
    assert_eq!(
        GenerationDeterministicCaseContractV1::from_json_bytes(&trailing, &case),
        Err(GenerationDeterministicCaseContractError::InvalidEncoding)
    );
    let unsupported = text.replacen("\"schema_version\":1", "\"schema_version\":2", 1);
    assert_eq!(
        GenerationDeterministicCaseContractV1::from_json_bytes(unsupported.as_bytes(), &case),
        Err(GenerationDeterministicCaseContractError::UnsupportedSchema(
            2
        ))
    );
}

#[test]
fn rejects_excessive_and_noncanonical_sets_before_identity_allocation() {
    let contract = GenerationDeterministicCaseContractV1::new(input()).expect("contract is valid");
    let case = case_manifest(&contract);
    assert_eq!(
        GenerationDeterministicCaseContractV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES + 1],
            &case,
        ),
        Err(GenerationDeterministicCaseContractError::EncodedRecordTooLarge)
    );

    let mut invalid = input();
    invalid.protected_terms.reverse();
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidProtectedTerms)
    );
    let mut invalid = input();
    invalid.protected_terms[1] = invalid.protected_terms[0].clone();
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidProtectedTerms)
    );
    let mut invalid = input();
    invalid.rubric_clause_ids.reverse();
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidRubricClauseIds)
    );
    let mut invalid = input();
    invalid.rubric_clause_ids = (0..=MAX_GENERATION_DETERMINISTIC_CASE_RUBRIC_CLAUSES)
        .map(|index| format!("clause-{index:02}"))
        .collect();
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidRubricClauseIds)
    );
    let mut invalid = input();
    invalid.protected_terms = (0..=MAX_GENERATION_DETERMINISTIC_CASE_PROTECTED_TERMS)
        .map(|index| format!("term-{index:04}"))
        .collect();
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidProtectedTerms)
    );
    let mut invalid = input();
    invalid.protected_terms =
        vec!["x".repeat(MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES)];
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::CanonicalEncodingTooLarge)
    );

    let mut invalid = input();
    invalid.protected_terms = vec!["\0".repeat(180_000)];
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::EncodedRecordTooLarge)
    );
}

#[test]
fn rejects_invalid_labels_source_bindings_and_empty_set_entries() {
    let mut invalid = input();
    invalid.case_key = "Invalid Case".to_owned();
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidCaseKey)
    );
    let mut invalid = input();
    invalid.evaluation_category = "Invalid Category".to_owned();
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidEvaluationCategory)
    );
    let mut invalid = input();
    invalid.source_byte_count = 0;
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidSourceBinding)
    );
    let mut invalid = input();
    invalid.source_artifact_id = ArtifactId::from_digest(digest("wrong source artifact"));
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidSourceBinding)
    );
    let mut invalid = input();
    invalid.protected_terms[0].clear();
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidProtectedTerms)
    );
    let mut invalid = input();
    invalid.protected_terms.clear();
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidProtectedTerms)
    );
    let mut invalid = input();
    invalid.rubric_clause_ids[0] = "Invalid Clause".to_owned();
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidRubricClauseIds)
    );
    let mut invalid = input();
    invalid.rubric_clause_ids.clear();
    assert_eq!(
        GenerationDeterministicCaseContractV1::new(invalid),
        Err(GenerationDeterministicCaseContractError::InvalidRubricClauseIds)
    );
}

#[test]
fn every_closed_expectation_variant_has_a_distinct_identity() {
    let mut judgment_digests = std::collections::BTreeSet::new();
    for judgment in [
        ReferenceJudgment::Acceptable,
        ReferenceJudgment::Unacceptable,
        ReferenceJudgment::Identity,
        ReferenceJudgment::NotApplicable,
    ] {
        let mut value = input();
        value.reference_judgment = judgment;
        let record = GenerationDeterministicCaseContractV1::new(value).expect("variant is valid");
        assert!(judgment_digests.insert(record.contract_digest().as_str().to_owned()));
    }
    let mut status_digests = std::collections::BTreeSet::new();
    for status in [
        RewriteStatus::Rewritten,
        RewriteStatus::UnchangedNoEligibleContent,
        RewriteStatus::Abstained,
        RewriteStatus::Failed,
    ] {
        let mut value = input();
        value.expected_status = status;
        let record = GenerationDeterministicCaseContractV1::new(value).expect("variant is valid");
        assert!(status_digests.insert(record.contract_digest().as_str().to_owned()));
    }
    let mut reason_digests = std::collections::BTreeSet::new();
    for reason in [
        ReasonCode::NoCandidate,
        ReasonCode::InvalidCandidate,
        ReasonCode::SentinelIntegrity,
        ReasonCode::ProtectedValueChanged,
        ReasonCode::StructureChanged,
        ReasonCode::UnsafeText,
        ReasonCode::SemanticMismatch,
        ReasonCode::SemanticUncertain,
        ReasonCode::ReassemblyVerification,
        ReasonCode::Cancelled,
        ReasonCode::UnsupportedAtomicity,
    ] {
        let mut value = input();
        value.expected_reason = Some(reason);
        let record = GenerationDeterministicCaseContractV1::new(value).expect("variant is valid");
        assert!(reason_digests.insert(record.contract_digest().as_str().to_owned()));
    }
    let mut output_digests = std::collections::BTreeSet::new();
    for output in [ExpectedOutput::Source, ExpectedOutput::Candidate] {
        let mut value = input();
        value.expected_output = output;
        let record = GenerationDeterministicCaseContractV1::new(value).expect("variant is valid");
        assert!(output_digests.insert(record.contract_digest().as_str().to_owned()));
    }

    let record = GenerationDeterministicCaseContractV1::new(input()).expect("contract is valid");
    assert_eq!(
        record.schema_version(),
        GENERATION_DETERMINISTIC_CASE_CONTRACT_SCHEMA_VERSION
    );
    assert_eq!(
        record.rubric_clause_ids(),
        ["fidelity".to_owned(), "protected-values".to_owned()]
    );
}

#[test]
fn rejects_every_manifest_substitution_and_source_material_change() {
    let contract = GenerationDeterministicCaseContractV1::new(input()).expect("contract is valid");
    let good_case = case_manifest(&contract);
    let cluster = GenerationClusterRecordV1::new("literal", digest("cluster policy"))
        .expect("cluster is valid");
    let manifest_input = GenerationCaseManifestV1Input {
        case_key: contract.case_key().to_owned(),
        source_artifact_id: contract.source_artifact_id().clone(),
        source_digest: contract.source_digest().clone(),
        source_byte_count: contract.source_byte_count(),
        case_contract_digest: contract.contract_digest().clone(),
        language_digest: contract.language_digest().clone(),
        mode_digest: contract.mode_digest().clone(),
        format_digest: contract.format_digest().clone(),
    };
    let mut substitutions = Vec::new();
    let mut changed = manifest_input.clone();
    changed.case_key = "other-case".to_owned();
    substitutions.push(changed);
    let mut changed = manifest_input.clone();
    changed.source_digest = digest("other source");
    changed.source_artifact_id = ArtifactId::from_digest(changed.source_digest.clone());
    substitutions.push(changed);
    let mut changed = manifest_input.clone();
    changed.source_byte_count += 1;
    substitutions.push(changed);
    let mut changed = manifest_input.clone();
    changed.case_contract_digest = digest("wrong contract");
    substitutions.push(changed);
    let mut changed = manifest_input.clone();
    changed.language_digest = digest("other language");
    substitutions.push(changed);
    let mut changed = manifest_input.clone();
    changed.mode_digest = digest("other mode");
    substitutions.push(changed);
    let mut changed = manifest_input;
    changed.format_digest = digest("other format");
    substitutions.push(changed);

    let bytes = contract
        .to_canonical_json_bytes()
        .expect("contract encodes");
    for substitution in substitutions {
        let wrong_case = GenerationCaseManifestV1::new(&cluster, substitution)
            .expect("substituted case is structurally valid");
        assert_eq!(
            contract.validate_case_manifest(&wrong_case),
            Err(GenerationDeterministicCaseContractError::CaseManifestMismatch)
        );
        assert_eq!(
            GenerationDeterministicCaseContractV1::from_json_bytes(&bytes, &wrong_case),
            Err(GenerationDeterministicCaseContractError::CaseManifestMismatch)
        );
    }

    let mut wrong_source_input = input();
    wrong_source_input.source_byte_count += 1;
    let wrong_source = GenerationDeterministicCaseContractV1::new(wrong_source_input)
        .expect("changed source count is structurally valid");
    assert_eq!(
        wrong_source.validate_case_manifest(&good_case),
        Err(GenerationDeterministicCaseContractError::CaseManifestMismatch)
    );
    assert_eq!(
        contract.project_evaluation_case(b"changed", "candidate"),
        Err(GenerationDeterministicCaseContractError::SourceMaterialMismatch)
    );

    let non_utf8 = [0xff];
    let source_digest = Digest::sha256(&non_utf8);
    let mut non_utf8_input = input();
    non_utf8_input.source_artifact_id = ArtifactId::from_digest(source_digest.clone());
    non_utf8_input.source_digest = source_digest;
    non_utf8_input.source_byte_count = 1;
    let non_utf8_contract = GenerationDeterministicCaseContractV1::new(non_utf8_input)
        .expect("opaque source binding is structurally valid");
    assert_eq!(
        non_utf8_contract.project_evaluation_case(&non_utf8, "candidate"),
        Err(GenerationDeterministicCaseContractError::SourceNotUtf8)
    );
}

#[test]
fn debug_output_redacts_case_labels_and_terms() {
    let contract = GenerationDeterministicCaseContractV1::new(input()).expect("contract is valid");
    let output = format!("{contract:?}");
    assert!(output.contains("contract_digest"));
    assert!(!output.contains("protected-literal"));
    assert!(!output.contains("Acme"));
}
