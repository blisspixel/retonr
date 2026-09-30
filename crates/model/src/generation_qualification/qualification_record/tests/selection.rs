use rewrite_types::Digest;
use serde_json::Value;

use super::super::super::{
    GENERATION_QUALIFICATION_SELECTION_ID_DOMAIN, GenerationQualificationSelectionV1,
    GenerationQualificationSelectionV1Error, GenerationQualificationSelectionV1Relations,
    MAX_GENERATION_QUALIFICATION_SELECTION_CANONICAL_BYTES,
    MAX_GENERATION_QUALIFICATION_SELECTION_JSON_BYTES,
};
use super::super::{
    GenerationQualificationId, GenerationQualificationRecordV1, GenerationQualificationStatusV1,
};
use super::RejectedFixture;

#[test]
fn selection_identity_follows_each_parent_and_ignores_status() {
    let fixture = RejectedFixture::new();
    let rejected =
        GenerationQualificationRecordV1::new(&fixture.relations()).expect("rejected record");
    let before = rejected.clone();
    let rejected_relations = GenerationQualificationSelectionV1Relations {
        qualification_record: &rejected,
    };
    let rejected_selection =
        GenerationQualificationSelectionV1::new(&rejected_relations).expect("selection");
    assert_eq!(rejected, before);
    assert_eq!(rejected_selection.schema_version(), 1);
    assert_eq!(
        rejected_selection.generation_qualification_id(),
        rejected.generation_qualification_id()
    );
    assert_selection_preimage(&rejected_selection);
    assert_eq!(
        GenerationQualificationSelectionV1::new(&rejected_relations).expect("same identity"),
        rejected_selection
    );
    rejected_selection
        .validate_against(&rejected_relations)
        .expect("revalidate selection");
    let bytes = serde_json::to_vec(&rejected_selection).expect("selection JSON");
    assert!(bytes.len() < MAX_GENERATION_QUALIFICATION_SELECTION_JSON_BYTES);
    assert_eq!(
        GenerationQualificationSelectionV1::from_json_bytes(&bytes, &rejected_relations)
            .expect("decode selection"),
        rejected_selection
    );
    let value: Value = serde_json::from_slice(&bytes).expect("selection value");
    let object = value.as_object().expect("object");
    assert_eq!(object.len(), 2);
    for forbidden in [
        "status",
        "reason_code",
        "role",
        "action",
        "generation_qualification_selection_id",
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
    let qualified_selection =
        GenerationQualificationSelectionV1::new(&GenerationQualificationSelectionV1Relations {
            qualification_record: &qualified,
        })
        .expect("qualified parent selection");
    assert_eq!(
        qualified_selection.generation_qualification_id(),
        qualified.generation_qualification_id()
    );
    assert_ne!(
        qualified_selection.generation_qualification_selection_id(),
        rejected_selection.generation_qualification_selection_id()
    );
    assert_eq!(rejected.status(), GenerationQualificationStatusV1::Rejected);
}

#[test]
fn selection_decoder_is_strict_bounded_and_canonical() {
    let fixture = RejectedFixture::new();
    let rejected =
        GenerationQualificationRecordV1::new(&fixture.relations()).expect("rejected record");
    let relations = GenerationQualificationSelectionV1Relations {
        qualification_record: &rejected,
    };
    let selection = GenerationQualificationSelectionV1::new(&relations).expect("selection");
    let bytes = serde_json::to_vec(&selection).expect("selection JSON");
    assert_eq!(MAX_GENERATION_QUALIFICATION_SELECTION_JSON_BYTES, 16_384);
    assert_eq!(
        MAX_GENERATION_QUALIFICATION_SELECTION_CANONICAL_BYTES,
        4_096
    );
    assert_eq!(
        GenerationQualificationSelectionV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_QUALIFICATION_SELECTION_JSON_BYTES + 1],
            &relations,
        ),
        Err(GenerationQualificationSelectionV1Error::EncodedRecordTooLarge)
    );
    assert_eq!(
        GenerationQualificationSelectionV1::from_json_bytes(b"{", &relations),
        Err(GenerationQualificationSelectionV1Error::InvalidEncoding)
    );
    let mut unknown: Value = serde_json::from_slice(&bytes).expect("selection value");
    unknown["reason_code"] = Value::String("stale".to_owned());
    assert_eq!(
        decode_selection(&unknown, relations),
        Err(GenerationQualificationSelectionV1Error::InvalidEncoding)
    );
    let text = String::from_utf8(bytes.clone()).expect("UTF-8 selection JSON");
    let duplicate = text.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert_eq!(
        GenerationQualificationSelectionV1::from_json_bytes(duplicate.as_bytes(), &relations),
        Err(GenerationQualificationSelectionV1Error::InvalidEncoding)
    );
    let spaced = format!(" {text}");
    assert_eq!(
        GenerationQualificationSelectionV1::from_json_bytes(spaced.as_bytes(), &relations),
        Err(GenerationQualificationSelectionV1Error::NonCanonicalEncoding)
    );
    let mut wrong_schema: Value = serde_json::from_slice(&bytes).expect("selection value");
    wrong_schema["schema_version"] = Value::from(2);
    assert_eq!(
        decode_selection(&wrong_schema, relations),
        Err(GenerationQualificationSelectionV1Error::UnsupportedSchema)
    );
    let mut substituted: Value = serde_json::from_slice(&bytes).expect("selection value");
    substituted["generation_qualification_id"] =
        Value::String(super::digest("substituted qualification").to_string());
    assert_eq!(
        decode_selection(&substituted, relations),
        Err(GenerationQualificationSelectionV1Error::RelationshipMismatch)
    );
}

fn assert_selection_preimage(selection: &GenerationQualificationSelectionV1) {
    let mut preimage = GENERATION_QUALIFICATION_SELECTION_ID_DOMAIN.to_vec();
    preimage.extend_from_slice(&selection.schema_version().to_be_bytes());
    preimage.extend_from_slice(
        selection
            .generation_qualification_id()
            .digest()
            .as_str()
            .as_bytes(),
    );
    assert_eq!(
        selection.generation_qualification_selection_id().digest(),
        &Digest::sha256(&preimage)
    );
}

fn decode_selection(
    value: &Value,
    relations: GenerationQualificationSelectionV1Relations<'_>,
) -> Result<GenerationQualificationSelectionV1, GenerationQualificationSelectionV1Error> {
    GenerationQualificationSelectionV1::from_json_bytes(
        &serde_json::to_vec(value).expect("encoded selection value"),
        &relations,
    )
}
