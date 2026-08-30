use rewrite_types::{Digest, ReasonCode, RewriteMode, RewriteStatus};

use super::*;
use crate::{
    ArtifactId, ExpectedOutput, GenerationCaseManifestV1Input, GenerationClusterRecordV1,
    GenerationDeterministicCaseContractV1Input, ReferenceJudgment,
};

const SOURCE: &[u8] = b"Retain Acme 42 exactly.";
const PURE_JSON: &str = concat!(
    "{\"schema_version\":1,",
    "\"language\":\"english\",",
    "\"rewrite_mode\":\"pure\",",
    "\"format\":\"plain_utf8_text\",",
    "\"unit_policy\":\"exactly_one_whole_document\",",
    "\"style_policy\":\"unavailable\",",
    "\"atomicity\":\"document\",",
    "\"protection_policy\":\"exact_contract_terms\"}"
);

struct Records {
    case: GenerationCaseManifestV1,
    contract: GenerationDeterministicCaseContractV1,
    system: GenerationSystemRecordV1,
}

fn records(language_digest: Digest, mode_digest: Digest, format_digest: Digest) -> Records {
    let source_digest = Digest::sha256(SOURCE);
    let contract =
        GenerationDeterministicCaseContractV1::new(GenerationDeterministicCaseContractV1Input {
            case_key: "protected-literal".to_owned(),
            source_artifact_id: ArtifactId::from_digest(source_digest.clone()),
            source_digest,
            source_byte_count: SOURCE.len() as u64,
            language_digest: language_digest.clone(),
            mode_digest: mode_digest.clone(),
            format_digest: format_digest.clone(),
            evaluation_category: "protected_value_negative".to_owned(),
            protected_terms: vec!["42".to_owned(), "Acme".to_owned()],
            reference_judgment: ReferenceJudgment::Unacceptable,
            expected_status: RewriteStatus::Abstained,
            expected_reason: Some(ReasonCode::ProtectedValueChanged),
            expected_output: ExpectedOutput::Source,
            rubric_clause_ids: vec!["fidelity".to_owned(), "protected-values".to_owned()],
        })
        .expect("deterministic contract");
    let cluster = GenerationClusterRecordV1::new(
        "request-profile",
        Digest::sha256(b"request profile cluster"),
    )
    .expect("cluster");
    let case = GenerationCaseManifestV1::new(
        &cluster,
        GenerationCaseManifestV1Input {
            case_key: contract.case_key().to_owned(),
            source_artifact_id: contract.source_artifact_id().clone(),
            source_digest: contract.source_digest().clone(),
            source_byte_count: contract.source_byte_count(),
            case_contract_digest: contract.contract_digest().clone(),
            language_digest: language_digest.clone(),
            mode_digest: mode_digest.clone(),
            format_digest: format_digest.clone(),
        },
    )
    .expect("case");

    let system_fixture = super::super::generation_system::test_support::fixture(false);
    let mut system_input = system_fixture.input();
    system_input.language_digest = language_digest;
    system_input.mode_digest = mode_digest;
    system_input.format_digest = format_digest;
    let system = GenerationSystemRecordV1::new(system_fixture.relations(), system_input)
        .expect("generation system");

    Records {
        case,
        contract,
        system,
    }
}

fn exact_records(profile: &GenerationCaseRequestProfileV1) -> Records {
    let records = records(
        profile.language_digest(),
        profile.rewrite_mode_digest(),
        profile.format_digest(),
    );
    profile
        .validate_against(&records.case, &records.contract, &records.system)
        .expect("profile fixture relationships");
    records
}

#[test]
fn digest_domains_and_every_closed_tag_have_stable_vectors() {
    let literal = GenerationCaseRequestProfileV1::new(RewriteMode::Literal);
    let pure = GenerationCaseRequestProfileV1::new(RewriteMode::Pure);
    let balanced = GenerationCaseRequestProfileV1::new(RewriteMode::Balanced);
    let strong = GenerationCaseRequestProfileV1::new(RewriteMode::Strong);

    assert_eq!(
        pure.language_digest().as_str(),
        "79f38c33d3c87cd376f4afb983b001697e09ee0224678c92aa288235bff4146f"
    );
    assert_eq!(
        literal.rewrite_mode_digest().as_str(),
        "5961be357e751f7b0ff3e4f06fda5b2e36a96a9e16676148f49d137a6207e4ea"
    );
    assert_eq!(
        pure.rewrite_mode_digest().as_str(),
        "c6dcad50ca48bab42c0a1af481292ff9e1e69658996ef070e410de6af7ad9ec7"
    );
    assert_eq!(
        balanced.rewrite_mode_digest().as_str(),
        "5ed7a923fd88b6240ee93a4e6517452fa6628a0fc0dc4bc9c46ff42e7a7f723a"
    );
    assert_eq!(
        strong.rewrite_mode_digest().as_str(),
        "b2e8b81fc55eaa9fbffdb750e87609e79c2eaeea28e773c44968d63425e55e29"
    );
    assert_eq!(
        pure.format_digest().as_str(),
        "21264fe268663347da561cdf1b36efcf8b294a0d795bba363cfc5070befd8421"
    );
    assert_ne!(pure.language_digest(), pure.format_digest());
}

#[test]
fn canonical_profile_round_trips_against_exact_records() {
    let profile = GenerationCaseRequestProfileV1::new(RewriteMode::Pure);
    let records = exact_records(&profile);
    assert_eq!(
        profile.to_canonical_json_bytes().expect("canonical JSON"),
        PURE_JSON.as_bytes()
    );
    let decoded = GenerationCaseRequestProfileV1::from_json_bytes(
        PURE_JSON.as_bytes(),
        &records.case,
        &records.contract,
        &records.system,
    )
    .expect("canonical profile");
    assert_eq!(decoded, profile);
    assert_eq!(decoded.language(), GenerationCaseLanguageV1::English);
    assert_eq!(decoded.rewrite_mode(), RewriteMode::Pure);
    assert_eq!(decoded.format(), GenerationCaseFormatV1::PlainUtf8Text);
    assert_eq!(
        decoded.unit_policy(),
        GenerationCaseUnitPolicyV1::ExactlyOneWholeDocument
    );
    assert_eq!(
        decoded.style_policy(),
        GenerationCaseStylePolicyV1::Unavailable
    );
    assert_eq!(decoded.atomicity(), GenerationCaseAtomicityV1::Document);
    assert_eq!(
        decoded.protection_policy(),
        GenerationCaseProtectionPolicyV1::ExactContractTerms
    );
}

#[test]
fn arbitrary_legacy_digests_are_readable_records_but_not_supported_profiles() {
    let profile = GenerationCaseRequestProfileV1::new(RewriteMode::Pure);
    for expected in [
        GenerationCaseRequestProfileError::LanguageMismatch,
        GenerationCaseRequestProfileError::RewriteModeMismatch,
        GenerationCaseRequestProfileError::FormatMismatch,
    ] {
        let language = if expected == GenerationCaseRequestProfileError::LanguageMismatch {
            Digest::sha256(b"legacy language")
        } else {
            profile.language_digest()
        };
        let mode = if expected == GenerationCaseRequestProfileError::RewriteModeMismatch {
            Digest::sha256(b"legacy mode")
        } else {
            profile.rewrite_mode_digest()
        };
        let format = if expected == GenerationCaseRequestProfileError::FormatMismatch {
            Digest::sha256(b"legacy format")
        } else {
            profile.format_digest()
        };
        let records = records(language, mode, format);
        assert_eq!(
            profile.validate_against(&records.case, &records.contract, &records.system),
            Err(expected)
        );
    }
}

#[test]
fn profile_rejects_a_contract_that_does_not_bind_the_exact_case() {
    let profile = GenerationCaseRequestProfileV1::new(RewriteMode::Pure);
    let records = exact_records(&profile);
    let foreign_contract =
        GenerationDeterministicCaseContractV1::new(GenerationDeterministicCaseContractV1Input {
            case_key: records.contract.case_key().to_owned(),
            source_artifact_id: records.contract.source_artifact_id().clone(),
            source_digest: records.contract.source_digest().clone(),
            source_byte_count: records.contract.source_byte_count(),
            language_digest: profile.language_digest(),
            mode_digest: profile.rewrite_mode_digest(),
            format_digest: profile.format_digest(),
            evaluation_category: "foreign-contract".to_owned(),
            protected_terms: records.contract.protected_terms().to_vec(),
            reference_judgment: records.contract.reference_judgment(),
            expected_status: records.contract.expected_status(),
            expected_reason: records.contract.expected_reason(),
            expected_output: records.contract.expected_output(),
            rubric_clause_ids: records.contract.rubric_clause_ids().to_vec(),
        })
        .expect("foreign deterministic contract");

    assert_eq!(
        profile.validate_against(&records.case, &foreign_contract, &records.system),
        Err(GenerationCaseRequestProfileError::CaseContractMismatch)
    );
}

#[test]
fn codec_rejects_noncanonical_unknown_trailing_and_oversized_input() {
    let profile = GenerationCaseRequestProfileV1::new(RewriteMode::Pure);
    let records = exact_records(&profile);
    let decode = |bytes: &[u8]| {
        GenerationCaseRequestProfileV1::from_json_bytes(
            bytes,
            &records.case,
            &records.contract,
            &records.system,
        )
    };
    let reordered = PURE_JSON.replacen(
        "\"schema_version\":1,\"language\":\"english\"",
        "\"language\":\"english\",\"schema_version\":1",
        1,
    );
    assert_eq!(
        decode(reordered.as_bytes()),
        Err(GenerationCaseRequestProfileError::NonCanonicalEncoding)
    );
    let unknown = PURE_JSON.replacen('{', "{\"unknown\":true,", 1);
    assert_eq!(
        decode(unknown.as_bytes()),
        Err(GenerationCaseRequestProfileError::InvalidEncoding)
    );
    let trailing = format!("{PURE_JSON} ");
    assert_eq!(
        decode(trailing.as_bytes()),
        Err(GenerationCaseRequestProfileError::NonCanonicalEncoding)
    );
    assert_eq!(
        decode(&vec![
            b' ';
            MAX_GENERATION_CASE_REQUEST_PROFILE_JSON_BYTES + 1
        ]),
        Err(GenerationCaseRequestProfileError::EncodedRecordTooLarge)
    );
}

#[test]
fn debug_is_profile_only_and_omits_digest_material() {
    let profile = GenerationCaseRequestProfileV1::new(RewriteMode::Strong);
    let debug = format!("{profile:?}");
    assert!(debug.contains("rewrite_mode: Strong"));
    assert!(!debug.contains(profile.language_digest().as_str()));
    assert!(!debug.contains(profile.rewrite_mode_digest().as_str()));
    assert!(!debug.contains(profile.format_digest().as_str()));
}
