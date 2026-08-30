use serde_json::Value;

use super::*;
use crate::EffectivePackageEvidenceV2;

struct ManagedFixture {
    base: Fixture,
    effective_package: EffectivePackageEvidenceV2,
    precursor: CandidateGenerationAttemptPrecursorV1,
    input: ManagedOllamaCandidateGenerationEvidenceV2Input,
    evidence: ManagedOllamaCandidateGenerationEvidenceV2,
}

impl ManagedFixture {
    fn relations(&self) -> ManagedOllamaCandidateGenerationEvidenceV2Relations<'_> {
        let ordinal = usize::try_from(self.precursor.runtime_installation_generation() - 7)
            .expect("fixture ordinal");
        ManagedOllamaCandidateGenerationEvidenceV2Relations {
            precursor: &self.precursor,
            planned_attempt: &self.base.attempts[ordinal],
            generation_system: &self.base.systems[ordinal % self.base.systems.len()],
            effective_package_evidence_v2: &self.effective_package,
        }
    }
}

fn managed_fixture(ordinal: usize) -> ManagedFixture {
    let base = fixture();
    let system_fixture = super::super::generation_system::test_support::fixture(false);
    let effective_package = system_fixture.effective_package;
    let structured_request_binding_id = StructuredCompletionRequestBindingId::from_derived_digest(
        digest(&format!("structured request {ordinal}")),
    );
    let precursor = CandidateGenerationAttemptPrecursorV1::new(
        &base.plan,
        &base.attempts[ordinal],
        &base.systems[ordinal % base.systems.len()],
        CandidateGenerationAttemptPrecursorV1Input {
            runtime_installation_generation: u64::try_from(ordinal).expect("ordinal") + 7,
            model_installation_generation: u64::try_from(ordinal).expect("ordinal") + 11,
            structured_request_binding_id,
        },
    )
    .expect("precursor");
    let input = ManagedOllamaCandidateGenerationEvidenceV2Input {
        bracket_observation_v1_id:
            ManagedOllamaGenerationBracketObservationV1Id::from_derived_digest(digest(&format!(
                "bracket {ordinal}"
            ))),
        effective_runtime_state_join_id:
            ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(digest(&format!(
                "live join {ordinal}"
            ))),
        response_id: OllamaRetainedSessionResponseId::from_derived_digest(digest(&format!(
            "response {ordinal}"
        ))),
    };
    let evidence = ManagedOllamaCandidateGenerationEvidenceV2::new(
        ManagedOllamaCandidateGenerationEvidenceV2Relations {
            precursor: &precursor,
            planned_attempt: &base.attempts[ordinal],
            generation_system: &base.systems[ordinal % base.systems.len()],
            effective_package_evidence_v2: &effective_package,
        },
        input.clone(),
    )
    .expect("managed evidence");
    ManagedFixture {
        base,
        effective_package,
        precursor,
        input,
        evidence,
    }
}

fn replace_once(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let value = String::from_utf8(bytes.to_vec()).expect("UTF-8 JSON");
    assert_eq!(value.matches(from).count(), 1, "unique replacement source");
    value.replacen(from, to, 1).into_bytes()
}

#[test]
fn managed_evidence_round_trips_and_exposes_exact_inert_facts() {
    let fixture = managed_fixture(0);
    let evidence = &fixture.evidence;
    assert_eq!(evidence.schema_version(), 2);
    assert_eq!(evidence.precursor_id(), fixture.precursor.precursor_id());
    assert_eq!(
        evidence.bracket_observation_v1_id(),
        &fixture.input.bracket_observation_v1_id
    );
    assert_eq!(
        evidence.effective_package_evidence_v2_id(),
        &fixture.effective_package.effective_package_evidence_v2_id()
    );
    assert_eq!(
        evidence.effective_runtime_state_id(),
        fixture.precursor.expected_effective_runtime_state_id()
    );
    assert_eq!(
        evidence.effective_runtime_state_join_id(),
        &fixture.input.effective_runtime_state_join_id
    );
    assert_eq!(
        evidence.generation_request_binding_id(),
        fixture.base.attempts[0].generation_request_binding_id()
    );
    assert_eq!(
        evidence.structured_request_binding_id(),
        fixture.precursor.structured_request_binding_id()
    );
    assert_eq!(evidence.response_id(), &fixture.input.response_id);
    assert_eq!(
        evidence.evidence_class(),
        ManagedOllamaCandidateGenerationEvidenceClassV2::RetainedManagedBracketWithEffectiveState
    );
    assert!(evidence.model_loaded_proven());
    assert!(!evidence.model_used_proven());
    assert!(!evidence.application_handler_proven());
    assert!(!evidence.formal_placement_proven());
    assert_eq!(
        evidence.managed_evidence_v2_id().digest().as_str(),
        "09a5632fc68bc9b52409b42e9d0c63f6df527bb0dd2b7ecb99b42c70ca16776f"
    );

    let encoded = serde_json::to_vec(evidence).expect("managed evidence JSON");
    assert!(
        String::from_utf8(encoded.clone())
            .expect("UTF-8 JSON")
            .contains("\"qualified\":false")
    );
    assert_eq!(
        ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
            &encoded,
            fixture.relations(),
            &fixture.input,
        )
        .expect("managed evidence decode"),
        *evidence
    );
    evidence
        .validate_against(fixture.relations(), &fixture.input)
        .expect("managed evidence revalidates");

    let debug = format!("{evidence:?}");
    assert!(debug.contains(evidence.managed_evidence_v2_id().digest().as_str()));
    for hidden in [
        evidence.generation_request_binding_id().digest().as_str(),
        evidence.structured_request_binding_id().digest().as_str(),
        evidence.response_id().digest().as_str(),
        evidence.effective_runtime_state_join_id().digest().as_str(),
    ] {
        assert!(!debug.contains(hidden));
    }
}

#[test]
fn managed_evidence_rejects_record_and_typed_input_substitution() {
    let fixture = managed_fixture(0);
    let relations = fixture.relations();
    assert_eq!(
        ManagedOllamaCandidateGenerationEvidenceV2::new(
            ManagedOllamaCandidateGenerationEvidenceV2Relations {
                planned_attempt: &fixture.base.attempts[1],
                ..relations
            },
            fixture.input.clone(),
        ),
        Err(GenerationQualificationContractError::ManagedEvidenceRelationshipMismatch)
    );
    assert_eq!(
        ManagedOllamaCandidateGenerationEvidenceV2::new(
            ManagedOllamaCandidateGenerationEvidenceV2Relations {
                generation_system: &fixture.base.systems[1],
                ..relations
            },
            fixture.input.clone(),
        ),
        Err(GenerationQualificationContractError::ManagedEvidenceRelationshipMismatch)
    );

    let other_system_fixture = super::super::generation_system::test_support::fixture(false);
    let changed_state = super::super::generation_system::test_support::runtime_state(
        &other_system_fixture.runtime_build,
        "changed",
    );
    let changed_package = super::super::generation_system::test_support::effective_package(
        &other_system_fixture.model_set,
        &other_system_fixture.runtime_build,
        &changed_state,
        false,
    );
    assert_eq!(
        ManagedOllamaCandidateGenerationEvidenceV2::new(
            ManagedOllamaCandidateGenerationEvidenceV2Relations {
                effective_package_evidence_v2: &changed_package,
                ..relations
            },
            fixture.input.clone(),
        ),
        Err(GenerationQualificationContractError::ManagedEvidenceRelationshipMismatch)
    );

    let encoded = serde_json::to_vec(&fixture.evidence).expect("managed evidence JSON");
    for changed in [
        ManagedOllamaCandidateGenerationEvidenceV2Input {
            bracket_observation_v1_id:
                ManagedOllamaGenerationBracketObservationV1Id::from_derived_digest(digest(
                    "other bracket",
                )),
            ..fixture.input.clone()
        },
        ManagedOllamaCandidateGenerationEvidenceV2Input {
            effective_runtime_state_join_id:
                ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(digest("other join")),
            ..fixture.input.clone()
        },
        ManagedOllamaCandidateGenerationEvidenceV2Input {
            response_id: OllamaRetainedSessionResponseId::from_derived_digest(digest(
                "other response",
            )),
            ..fixture.input.clone()
        },
    ] {
        assert_eq!(
            ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
                &encoded, relations, &changed,
            ),
            Err(GenerationQualificationContractError::ManagedEvidenceRelationshipMismatch)
        );
        let variant = ManagedOllamaCandidateGenerationEvidenceV2::new(relations, changed)
            .expect("typed variant");
        assert_ne!(
            variant.managed_evidence_v2_id(),
            fixture.evidence.managed_evidence_v2_id()
        );
    }
}

#[test]
fn managed_evidence_decoder_is_bounded_canonical_and_future_schema_first() {
    let fixture = managed_fixture(0);
    let encoded = serde_json::to_vec(&fixture.evidence).expect("managed evidence JSON");
    assert_eq!(
        ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
            &vec![b' '; MAX_MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_V2_JSON_BYTES + 1],
            fixture.relations(),
            &fixture.input,
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
            b"{",
            fixture.relations(),
            &fixture.input,
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    for invalid in [
        replace_once(
            &encoded,
            "\"schema_version\":2,",
            "\"schema_version\":2,\"schema_version\":2,",
        ),
        [encoded.clone(), b" trailing".to_vec()].concat(),
        replace_once(
            &encoded,
            "retained_managed_bracket_with_effective_state",
            "unsupported_evidence_class",
        ),
    ] {
        assert_eq!(
            ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
                &invalid,
                fixture.relations(),
                &fixture.input,
            ),
            Err(GenerationQualificationContractError::InvalidEncoding)
        );
    }
    let mut unknown = serde_json::to_value(&fixture.evidence).expect("managed value");
    unknown["authority"] = Value::Bool(true);
    assert_eq!(
        ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
            &serde_json::to_vec(&unknown).expect("unknown JSON"),
            fixture.relations(),
            &fixture.input,
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let mut whitespace = vec![b' '];
    whitespace.extend(&encoded);
    assert_eq!(
        ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
            &whitespace,
            fixture.relations(),
            &fixture.input,
        ),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );

    let future = replace_once(&encoded, "\"schema_version\":2", "\"schema_version\":3");
    let future = replace_once(
        &future,
        fixture.evidence.precursor_id().digest().as_str(),
        digest("wrong precursor").as_str(),
    );
    assert_eq!(
        ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
            &future,
            fixture.relations(),
            &fixture.input,
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(3))
    );
}

#[test]
fn managed_evidence_decoder_rejects_claim_and_relationship_substitution() {
    let fixture = managed_fixture(0);
    let encoded = serde_json::to_vec(&fixture.evidence).expect("managed evidence JSON");
    for field in [
        "model_loaded_proven",
        "model_used_proven",
        "application_handler_proven",
        "formal_placement_proven",
        "qualified",
    ] {
        let from = format!("\"{field}\":{}", field == "model_loaded_proven");
        let to = format!("\"{field}\":{}", field != "model_loaded_proven");
        assert_eq!(
            ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
                &replace_once(&encoded, &from, &to),
                fixture.relations(),
                &fixture.input,
            ),
            Err(GenerationQualificationContractError::InvalidManagedEvidenceClaims)
        );
    }

    for (from, to) in [
        (
            fixture
                .evidence
                .effective_package_evidence_v2_id()
                .digest()
                .as_str(),
            digest("wrong effective package"),
        ),
        (
            fixture
                .evidence
                .effective_runtime_state_id()
                .digest()
                .as_str(),
            digest("wrong effective state"),
        ),
        (
            fixture
                .evidence
                .generation_request_binding_id()
                .digest()
                .as_str(),
            digest("wrong provider request"),
        ),
        (
            fixture
                .evidence
                .structured_request_binding_id()
                .digest()
                .as_str(),
            digest("wrong wire request"),
        ),
    ] {
        assert_eq!(
            ManagedOllamaCandidateGenerationEvidenceV2::from_json_bytes(
                &replace_once(&encoded, from, to.as_str()),
                fixture.relations(),
                &fixture.input,
            ),
            Err(GenerationQualificationContractError::ManagedEvidenceRelationshipMismatch)
        );
    }
}

#[test]
fn managed_evidence_schema_is_machine_readable() {
    let schema = schemars::schema_for!(ManagedOllamaCandidateGenerationEvidenceV2);
    let value = serde_json::to_value(schema).expect("schema JSON");
    assert_eq!(
        value["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
}

#[test]
fn every_managed_evidence_field_participates_in_the_stable_identity() {
    let fixture = managed_fixture(0);
    let evidence = &fixture.evidence;
    let mut independent = MANAGED_OLLAMA_CANDIDATE_EVIDENCE_ID_DOMAIN.to_vec();
    let mut field_offsets = vec![independent.len()];
    independent.extend_from_slice(&evidence.schema_version().to_be_bytes());
    for value in [
        evidence.precursor_id().digest(),
        evidence.bracket_observation_v1_id().digest(),
        evidence.effective_package_evidence_v2_id().digest(),
        evidence.effective_runtime_state_id().digest(),
        evidence.effective_runtime_state_join_id().digest(),
        evidence.generation_request_binding_id().digest(),
        evidence.structured_request_binding_id().digest(),
        evidence.response_id().digest(),
    ] {
        field_offsets.push(independent.len());
        independent.extend_from_slice(value.as_str().as_bytes());
    }
    field_offsets.push(independent.len());
    independent.push(0);
    for value in [true, false, false, false, false] {
        field_offsets.push(independent.len());
        independent.push(u8::from(value));
    }
    assert_eq!(
        Digest::sha256(&independent),
        *evidence.managed_evidence_v2_id().digest()
    );
    assert_eq!(field_offsets.len(), 15);
    for offset in field_offsets {
        let mut changed = independent.clone();
        changed[offset] ^= 1;
        assert_ne!(
            Digest::sha256(&changed),
            *evidence.managed_evidence_v2_id().digest()
        );
    }
}

#[path = "managed_cleanup/bundle.rs"]
mod bundle;
#[path = "managed_cleanup/cleanup.rs"]
mod cleanup;
