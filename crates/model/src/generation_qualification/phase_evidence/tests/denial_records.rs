use serde_json::Value;

use super::*;

#[test]
fn denial_records_round_trip_with_frozen_ids_and_canonical_json() {
    let fixture = fixture();
    let policy = policy("shared denied phase policy");
    let resource_relations = GenerationResourcePolicyDenialRecordV1Relations {
        scope: fixture.scope(),
        phase_policy_digest: &policy,
    };
    let human_relations = GenerationHumanAdjudicationPolicyDenialRecordV1Relations {
        scope: fixture.scope(),
        phase_policy_digest: &policy,
    };
    let resource =
        GenerationResourcePolicyDenialRecordV1::new(resource_relations).expect("resource denial");
    let human = GenerationHumanAdjudicationPolicyDenialRecordV1::new(human_relations)
        .expect("human denial");
    let resource_json = serde_json::to_vec(&resource).expect("resource JSON");
    let human_json = serde_json::to_vec(&human).expect("human JSON");

    assert_eq!(resource.schema_version(), 1);
    assert_eq!(
        resource.generation_system_id(),
        fixture.scope().generation_system.generation_system_id()
    );
    assert_eq!(
        resource.generation_qualification_plan_id(),
        fixture.plan.qualification_plan_id()
    );
    assert_eq!(
        resource.suite_manifest_id(),
        fixture.suite.suite_manifest_id()
    );
    assert_eq!(resource.phase_policy_digest(), &policy);
    assert_eq!(
        resource.reason(),
        GenerationPhasePolicyDenialReasonV1::PolicySourceDenied
    );
    resource
        .validate_against(resource_relations)
        .expect("resource revalidation");
    human
        .validate_against(human_relations)
        .expect("human revalidation");
    assert_eq!(
        GenerationResourcePolicyDenialRecordV1::from_json_bytes(
            &resource_json,
            resource_relations,
        )
        .expect("resource decode"),
        resource
    );
    assert_eq!(
        GenerationHumanAdjudicationPolicyDenialRecordV1::from_json_bytes(
            &human_json,
            human_relations,
        )
        .expect("human decode"),
        human
    );

    assert_eq!(
        [
            resource
                .resource_policy_denial_record_id()
                .digest()
                .as_str(),
            human
                .human_adjudication_policy_denial_record_id()
                .digest()
                .as_str(),
        ],
        [
            "54afcc7094bdaaa9d432f71a79591a62e87a0aed9a16894c49509c1836cb041c",
            "070ab12ce85457fecbbcf4763bc4a3e9cb60f1f304f34289d98c0cf15764b5c1",
        ]
    );
    assert_eq!(
        String::from_utf8(resource_json).expect("UTF-8 JSON"),
        concat!(
            "{\"schema_version\":1,\"record_kind\":\"resource_policy_denial\",",
            "\"generation_system_id\":\"6855cc4c5336d585c6e64d660b4ba73ba5cf9d23b902cad61e586d0c8d8fb8ab\",",
            "\"generation_qualification_plan_id\":\"9795268aab74e0ce006a64e5236b654cd104403e03c67c7739a3db387886dc1c\",",
            "\"suite_manifest_id\":\"04a7cd0a4e6032cbda9edfd0c278dc79392e1728ff2e18fe316878e451f2cd1a\",",
            "\"phase_policy_digest\":\"5e4567f1b4d6bb9f17fedd65c8792fe1afcb645ff963b6602308506026ecb225\",",
            "\"reason\":\"policy_source_denied\"}"
        )
    );
    assert_eq!(
        String::from_utf8(human_json).expect("UTF-8 JSON"),
        concat!(
            "{\"schema_version\":1,\"record_kind\":\"human_adjudication_policy_denial\",",
            "\"generation_system_id\":\"6855cc4c5336d585c6e64d660b4ba73ba5cf9d23b902cad61e586d0c8d8fb8ab\",",
            "\"generation_qualification_plan_id\":\"9795268aab74e0ce006a64e5236b654cd104403e03c67c7739a3db387886dc1c\",",
            "\"suite_manifest_id\":\"04a7cd0a4e6032cbda9edfd0c278dc79392e1728ff2e18fe316878e451f2cd1a\",",
            "\"phase_policy_digest\":\"5e4567f1b4d6bb9f17fedd65c8792fe1afcb645ff963b6602308506026ecb225\",",
            "\"reason\":\"policy_source_denied\"}"
        )
    );
}

#[test]
fn every_resource_wire_field_is_rederived_or_closed() {
    let fixture = fixture();
    let policy = policy("resource denied policy");
    let relations = GenerationResourcePolicyDenialRecordV1Relations {
        scope: fixture.scope(),
        phase_policy_digest: &policy,
    };
    let record = GenerationResourcePolicyDenialRecordV1::new(relations).expect("resource denial");
    let canonical = serde_json::to_vec(&record).expect("canonical JSON");

    assert_substitution(
        &canonical,
        "schema_version",
        Value::from(2),
        GenerationQualificationPhaseEvidenceError::UnsupportedSchema,
        |bytes| GenerationResourcePolicyDenialRecordV1::from_json_bytes(bytes, relations).map(drop),
    );
    assert_substitution(
        &canonical,
        "record_kind",
        Value::String("human_adjudication_policy_denial".to_owned()),
        GenerationQualificationPhaseEvidenceError::InvalidEncoding,
        |bytes| GenerationResourcePolicyDenialRecordV1::from_json_bytes(bytes, relations).map(drop),
    );
    for field in [
        "generation_system_id",
        "generation_qualification_plan_id",
        "suite_manifest_id",
        "phase_policy_digest",
    ] {
        assert_substitution(
            &canonical,
            field,
            Value::String(digest(&format!("substitute {field}")).as_str().to_owned()),
            GenerationQualificationPhaseEvidenceError::RelationshipMismatch,
            |bytes| {
                GenerationResourcePolicyDenialRecordV1::from_json_bytes(bytes, relations).map(drop)
            },
        );
    }
    assert_substitution(
        &canonical,
        "reason",
        Value::String("policy_allowed".to_owned()),
        GenerationQualificationPhaseEvidenceError::InvalidEncoding,
        |bytes| GenerationResourcePolicyDenialRecordV1::from_json_bytes(bytes, relations).map(drop),
    );
}

#[test]
fn every_human_wire_field_is_rederived_or_closed() {
    let fixture = fixture();
    let policy = policy("human denied policy");
    let relations = GenerationHumanAdjudicationPolicyDenialRecordV1Relations {
        scope: fixture.scope(),
        phase_policy_digest: &policy,
    };
    let record =
        GenerationHumanAdjudicationPolicyDenialRecordV1::new(relations).expect("human denial");
    let canonical = serde_json::to_vec(&record).expect("canonical JSON");

    assert_substitution(
        &canonical,
        "schema_version",
        Value::from(2),
        GenerationQualificationPhaseEvidenceError::UnsupportedSchema,
        |bytes| {
            GenerationHumanAdjudicationPolicyDenialRecordV1::from_json_bytes(bytes, relations)
                .map(drop)
        },
    );
    assert_substitution(
        &canonical,
        "record_kind",
        Value::String("resource_policy_denial".to_owned()),
        GenerationQualificationPhaseEvidenceError::InvalidEncoding,
        |bytes| {
            GenerationHumanAdjudicationPolicyDenialRecordV1::from_json_bytes(bytes, relations)
                .map(drop)
        },
    );
    for field in [
        "generation_system_id",
        "generation_qualification_plan_id",
        "suite_manifest_id",
        "phase_policy_digest",
    ] {
        assert_substitution(
            &canonical,
            field,
            Value::String(
                digest(&format!("substitute human {field}"))
                    .as_str()
                    .to_owned(),
            ),
            GenerationQualificationPhaseEvidenceError::RelationshipMismatch,
            |bytes| {
                GenerationHumanAdjudicationPolicyDenialRecordV1::from_json_bytes(bytes, relations)
                    .map(drop)
            },
        );
    }
    assert_substitution(
        &canonical,
        "reason",
        Value::String("policy_allowed".to_owned()),
        GenerationQualificationPhaseEvidenceError::InvalidEncoding,
        |bytes| {
            GenerationHumanAdjudicationPolicyDenialRecordV1::from_json_bytes(bytes, relations)
                .map(drop)
        },
    );
}

#[test]
fn denial_json_is_strict_bounded_and_cross_phase_substitution_resistant() {
    let fixture = fixture();
    let policy = policy("strict denied policy");
    let resource_relations = GenerationResourcePolicyDenialRecordV1Relations {
        scope: fixture.scope(),
        phase_policy_digest: &policy,
    };
    let human_relations = GenerationHumanAdjudicationPolicyDenialRecordV1Relations {
        scope: fixture.scope(),
        phase_policy_digest: &policy,
    };
    let resource =
        GenerationResourcePolicyDenialRecordV1::new(resource_relations).expect("resource denial");
    let human = GenerationHumanAdjudicationPolicyDenialRecordV1::new(human_relations)
        .expect("human denial");
    let resource_json = serde_json::to_vec(&resource).expect("resource JSON");
    let human_json = serde_json::to_vec(&human).expect("human JSON");

    assert_eq!(
        GenerationHumanAdjudicationPolicyDenialRecordV1::from_json_bytes(
            &resource_json,
            human_relations,
        ),
        Err(GenerationQualificationPhaseEvidenceError::InvalidEncoding)
    );
    assert_eq!(
        GenerationResourcePolicyDenialRecordV1::from_json_bytes(&human_json, resource_relations,),
        Err(GenerationQualificationPhaseEvidenceError::InvalidEncoding)
    );
    assert_eq!(
        GenerationResourcePolicyDenialRecordV1::from_json_bytes(b"{", resource_relations),
        Err(GenerationQualificationPhaseEvidenceError::InvalidEncoding)
    );

    let duplicate = String::from_utf8(resource_json.clone())
        .expect("UTF-8 JSON")
        .replacen(
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
            1,
        );
    assert_eq!(
        GenerationResourcePolicyDenialRecordV1::from_json_bytes(
            duplicate.as_bytes(),
            resource_relations,
        ),
        Err(GenerationQualificationPhaseEvidenceError::InvalidEncoding)
    );

    let mut unknown = resource_json.clone();
    unknown.pop();
    unknown.extend_from_slice(b",\"unknown\":0}");
    assert_eq!(
        GenerationResourcePolicyDenialRecordV1::from_json_bytes(&unknown, resource_relations),
        Err(GenerationQualificationPhaseEvidenceError::InvalidEncoding)
    );

    let mut trailing = resource_json.clone();
    trailing.push(b' ');
    assert_eq!(
        GenerationResourcePolicyDenialRecordV1::from_json_bytes(&trailing, resource_relations),
        Err(GenerationQualificationPhaseEvidenceError::NonCanonicalEncoding)
    );

    let reordered = reorder_first_two_fields(&resource_json);
    assert_eq!(
        GenerationResourcePolicyDenialRecordV1::from_json_bytes(&reordered, resource_relations),
        Err(GenerationQualificationPhaseEvidenceError::NonCanonicalEncoding)
    );
    assert_eq!(
        GenerationResourcePolicyDenialRecordV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_PHASE_POLICY_DENIAL_RECORD_JSON_BYTES + 1],
            resource_relations,
        ),
        Err(GenerationQualificationPhaseEvidenceError::EncodedRecordTooLarge)
    );
}

#[test]
fn equal_relationships_have_distinct_domains_and_form_exact_failed_manifests() {
    let fixture = fixture();
    let policy = policy("equal denied policy");
    let resource = GenerationResourcePolicyDenialRecordV1::new(
        GenerationResourcePolicyDenialRecordV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &policy,
        },
    )
    .expect("resource denial");
    let human = GenerationHumanAdjudicationPolicyDenialRecordV1::new(
        GenerationHumanAdjudicationPolicyDenialRecordV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &policy,
        },
    )
    .expect("human denial");

    assert_ne!(
        GENERATION_RESOURCE_POLICY_DENIAL_RECORD_ID_DOMAIN,
        GENERATION_HUMAN_ADJUDICATION_POLICY_DENIAL_RECORD_ID_DOMAIN
    );
    assert_ne!(
        resource.resource_policy_denial_record_id().digest(),
        human.human_adjudication_policy_denial_record_id().digest()
    );

    let resource_items = [resource.resource_policy_denial_record_id().digest().clone()];
    let resource_manifest =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &policy,
            evidence_record_digests: &resource_items,
            status: GenerationQualificationPhaseStatusV1::Failed,
        })
        .expect("failed resource manifest");
    let human_items = [human
        .human_adjudication_policy_denial_record_id()
        .digest()
        .clone()];
    let human_manifest = GenerationHumanAdjudicationEvidenceManifestV1::new(
        GenerationHumanAdjudicationEvidenceManifestV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &policy,
            evidence_record_digests: &human_items,
            status: GenerationQualificationPhaseStatusV1::Failed,
        },
    )
    .expect("failed human manifest");

    assert_eq!(resource_manifest.evidence_item_count(), 1);
    assert_eq!(human_manifest.evidence_item_count(), 1);
    assert_eq!(
        resource_manifest.status(),
        GenerationQualificationPhaseStatusV1::Failed
    );
    assert_eq!(
        human_manifest.status(),
        GenerationQualificationPhaseStatusV1::Failed
    );
}

#[test]
fn denial_debug_is_redacted_and_relationship_revalidation_rejects_substitution() {
    let fixture = fixture();
    let denied_policy = policy("redacted denied policy");
    let relations = GenerationResourcePolicyDenialRecordV1Relations {
        scope: fixture.scope(),
        phase_policy_digest: &denied_policy,
    };
    let resource = GenerationResourcePolicyDenialRecordV1::new(relations).expect("resource denial");
    let rendered = format!("{resource:?}");
    assert!(rendered.contains("record_id"));
    assert!(rendered.contains("PolicySourceDenied"));
    assert!(!rendered.contains(denied_policy.as_str()));
    assert!(!rendered.contains(resource.generation_system_id().digest().as_str()));

    let foreign_scope = GenerationQualificationPhaseScopeV1 {
        generation_system: &fixture.systems[1],
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
    };
    assert_eq!(
        resource.validate_against(GenerationResourcePolicyDenialRecordV1Relations {
            scope: foreign_scope,
            phase_policy_digest: &denied_policy,
        }),
        Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
    );
    let foreign_policy = policy("foreign denied policy");
    assert_eq!(
        resource.validate_against(GenerationResourcePolicyDenialRecordV1Relations {
            scope: fixture.scope(),
            phase_policy_digest: &foreign_policy,
        }),
        Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
    );
}

fn assert_substitution(
    canonical: &[u8],
    field: &str,
    replacement: Value,
    expected: GenerationQualificationPhaseEvidenceError,
    decode: impl Fn(&[u8]) -> Result<(), GenerationQualificationPhaseEvidenceError>,
) {
    let mut value: Value = serde_json::from_slice(canonical).expect("JSON value");
    value[field] = replacement;
    assert_eq!(
        decode(&serde_json::to_vec(&value).expect("substituted JSON")),
        Err(expected),
        "field {field} accepted substitution"
    );
}

fn reorder_first_two_fields(canonical: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(canonical.to_vec()).expect("UTF-8 JSON");
    let first_comma = text.find(',').expect("first field");
    let second_comma = text[first_comma + 1..]
        .find(',')
        .map(|index| index + first_comma + 1)
        .expect("second field");
    format!(
        "{{{},{},{}",
        &text[first_comma + 1..second_comma],
        &text[1..first_comma],
        &text[second_comma + 1..]
    )
    .into_bytes()
}
