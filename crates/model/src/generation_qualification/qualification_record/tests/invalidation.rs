use rewrite_types::Digest;
use serde_json::Value;

use super::super::super::{
    GENERATION_QUALIFICATION_INVALIDATION_ID_DOMAIN, GenerationQualificationInvalidationV1,
    GenerationQualificationInvalidationV1Error, GenerationQualificationInvalidationV1Relations,
    MAX_GENERATION_QUALIFICATION_INVALIDATION_CANONICAL_BYTES,
    MAX_GENERATION_QUALIFICATION_INVALIDATION_JSON_BYTES,
};
use super::super::{
    GenerationQualificationId, GenerationQualificationRecordV1, GenerationQualificationStatusV1,
};
use super::RejectedFixture;

#[test]
fn invalidation_identity_follows_each_parent_and_ignores_status() {
    let fixture = RejectedFixture::new();
    let rejected =
        GenerationQualificationRecordV1::new(&fixture.relations()).expect("rejected record");
    let before = rejected.clone();
    let rejected_relations = GenerationQualificationInvalidationV1Relations {
        qualification_record: &rejected,
    };
    let rejected_invalidation =
        GenerationQualificationInvalidationV1::new(&rejected_relations).expect("invalidation");
    assert_eq!(rejected, before);
    assert_eq!(rejected_invalidation.schema_version(), 1);
    assert_eq!(
        rejected_invalidation.generation_qualification_id(),
        rejected.generation_qualification_id()
    );
    assert_invalidation_preimage(&rejected_invalidation);
    assert_eq!(
        GenerationQualificationInvalidationV1::new(&rejected_relations).expect("same identity"),
        rejected_invalidation
    );
    rejected_invalidation
        .validate_against(&rejected_relations)
        .expect("revalidate invalidation");
    let bytes = serde_json::to_vec(&rejected_invalidation).expect("invalidation JSON");
    assert!(bytes.len() < MAX_GENERATION_QUALIFICATION_INVALIDATION_JSON_BYTES);
    assert_eq!(
        GenerationQualificationInvalidationV1::from_json_bytes(&bytes, &rejected_relations)
            .expect("decode invalidation"),
        rejected_invalidation
    );
    let value: Value = serde_json::from_slice(&bytes).expect("invalidation value");
    let object = value.as_object().expect("object");
    assert_eq!(object.len(), 2);
    for forbidden in [
        "status",
        "reason_code",
        "generation_qualification_invalidation_id",
        "target_generation_system_id",
        "operation_receipt_id",
    ] {
        assert!(object.get(forbidden).is_none(), "{forbidden}");
    }

    let mut qualified = rejected.clone();
    qualified.status = GenerationQualificationStatusV1::Qualified;
    qualified.id = GenerationQualificationId::from_canonical_bytes(&qualified.canonical_bytes());
    assert_eq!(
        qualified.status(),
        GenerationQualificationStatusV1::Qualified
    );
    let qualified_invalidation = GenerationQualificationInvalidationV1::new(
        &GenerationQualificationInvalidationV1Relations {
            qualification_record: &qualified,
        },
    )
    .expect("qualified parent invalidation");
    assert_eq!(
        qualified_invalidation.generation_qualification_id(),
        qualified.generation_qualification_id()
    );
    assert_ne!(
        qualified_invalidation.generation_qualification_invalidation_id(),
        rejected_invalidation.generation_qualification_invalidation_id()
    );
    assert_eq!(rejected.status(), GenerationQualificationStatusV1::Rejected);
}

#[test]
fn invalidation_decoder_is_strict_bounded_and_canonical() {
    let fixture = RejectedFixture::new();
    let rejected =
        GenerationQualificationRecordV1::new(&fixture.relations()).expect("rejected record");
    let relations = GenerationQualificationInvalidationV1Relations {
        qualification_record: &rejected,
    };
    let invalidation =
        GenerationQualificationInvalidationV1::new(&relations).expect("invalidation");
    let bytes = serde_json::to_vec(&invalidation).expect("invalidation JSON");
    assert_eq!(MAX_GENERATION_QUALIFICATION_INVALIDATION_JSON_BYTES, 16_384);
    assert_eq!(
        MAX_GENERATION_QUALIFICATION_INVALIDATION_CANONICAL_BYTES,
        4_096
    );
    assert_eq!(
        GenerationQualificationInvalidationV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_QUALIFICATION_INVALIDATION_JSON_BYTES + 1],
            &relations,
        ),
        Err(GenerationQualificationInvalidationV1Error::EncodedRecordTooLarge)
    );
    assert_eq!(
        GenerationQualificationInvalidationV1::from_json_bytes(b"{", &relations),
        Err(GenerationQualificationInvalidationV1Error::InvalidEncoding)
    );
    let mut unknown: Value = serde_json::from_slice(&bytes).expect("invalidation value");
    unknown["reason_code"] = Value::String("stale".to_owned());
    assert_eq!(
        decode_invalidation(&unknown, relations),
        Err(GenerationQualificationInvalidationV1Error::InvalidEncoding)
    );
    let text = String::from_utf8(bytes.clone()).expect("UTF-8 invalidation JSON");
    let duplicate = text.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert_eq!(
        GenerationQualificationInvalidationV1::from_json_bytes(duplicate.as_bytes(), &relations),
        Err(GenerationQualificationInvalidationV1Error::InvalidEncoding)
    );
    let spaced = format!(" {text}");
    assert_eq!(
        GenerationQualificationInvalidationV1::from_json_bytes(spaced.as_bytes(), &relations),
        Err(GenerationQualificationInvalidationV1Error::NonCanonicalEncoding)
    );
    let mut wrong_schema: Value = serde_json::from_slice(&bytes).expect("invalidation value");
    wrong_schema["schema_version"] = Value::from(2);
    assert_eq!(
        decode_invalidation(&wrong_schema, relations),
        Err(GenerationQualificationInvalidationV1Error::UnsupportedSchema)
    );
    let mut substituted: Value = serde_json::from_slice(&bytes).expect("invalidation value");
    substituted["generation_qualification_id"] =
        Value::String(super::digest("substituted qualification").to_string());
    assert_eq!(
        decode_invalidation(&substituted, relations),
        Err(GenerationQualificationInvalidationV1Error::RelationshipMismatch)
    );
}

fn assert_invalidation_preimage(invalidation: &GenerationQualificationInvalidationV1) {
    let mut preimage = GENERATION_QUALIFICATION_INVALIDATION_ID_DOMAIN.to_vec();
    preimage.extend_from_slice(&invalidation.schema_version().to_be_bytes());
    preimage.extend_from_slice(
        invalidation
            .generation_qualification_id()
            .digest()
            .as_str()
            .as_bytes(),
    );
    assert_eq!(
        invalidation
            .generation_qualification_invalidation_id()
            .digest(),
        &Digest::sha256(&preimage)
    );
}

fn decode_invalidation(
    value: &Value,
    relations: GenerationQualificationInvalidationV1Relations<'_>,
) -> Result<GenerationQualificationInvalidationV1, GenerationQualificationInvalidationV1Error> {
    GenerationQualificationInvalidationV1::from_json_bytes(
        &serde_json::to_vec(value).expect("encoded invalidation value"),
        &relations,
    )
}
