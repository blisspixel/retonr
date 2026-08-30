use rewrite_model::{ArtifactId, PackageSourceKind};
use rewrite_ollama_package::{OllamaLocalArchiveMemberBinding, ReconstructionLimits};
use rewrite_types::{CancellationToken, Digest};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    MODEL_LICENSE_REVIEW_PROCEDURE_ID, MODEL_LICENSE_REVIEW_PROCEDURE_VERSION,
    MODEL_LICENSE_REVIEW_SCHEMA_VERSION, ModelLicenseControlCompiler, ModelLicenseControlVerifier,
    ModelLicensePermission, OllamaModelReference, PackageAttestationService,
    ProductionModelLicenseApprovalPolicy, VerifiedManagedOllamaLaunchPlan,
    VerifiedManagedOllamaModelPackageLease,
};

use super::*;

#[expect(
    clippy::duplicate_mod,
    reason = "reuse the exact managed-model fixture without changing its production module"
)]
#[path = "../model_license_control/fixture.rs"]
mod fixture;
use fixture::Fixture;

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ReviewDecision {
    Approved,
}

#[derive(Serialize)]
struct Review<'a> {
    decision: ReviewDecision,
    foundation: Foundation<'a>,
    license_members: Vec<Member<'a>>,
    permission: ModelLicensePermission,
    procedure_id: &'static str,
    procedure_version: u32,
    schema_version: u32,
    source: Source<'a>,
}

#[derive(Serialize)]
struct Foundation<'a> {
    artifact_set_id: &'a Digest,
    descriptor_mapping_digest: &'a Digest,
    #[serde(rename = "foundation_id")]
    identity_digest: &'a Digest,
    logical_binding_digest: &'a Digest,
    model_package_manifest_id: &'a Digest,
    package_source_id: &'a Digest,
    provenance_manifest: Member<'a>,
}

#[derive(Serialize)]
struct Source<'a> {
    kind: PackageSourceKind,
    locator: &'a str,
    provenance_digest: &'a Digest,
    revision: &'a str,
}

#[derive(Serialize)]
struct Member<'a> {
    artifact_id: &'a ArtifactId,
    byte_size: u64,
    relative_path: &'a str,
}

fn member(value: &OllamaLocalArchiveMemberBinding) -> Member<'_> {
    Member {
        artifact_id: value.artifact_id(),
        byte_size: value.byte_size(),
        relative_path: value.relative_path().as_str(),
    }
}

fn reviewer_json(lease: &VerifiedManagedOllamaModelPackageLease) -> Vec<u8> {
    let view = lease.private_view();
    let foundation = view.foundation_evidence();
    let package = view.model_package_manifest();
    let source = package.source();
    serde_json::to_vec(&Review {
        decision: ReviewDecision::Approved,
        foundation: Foundation {
            artifact_set_id: foundation.artifact_set_id().digest(),
            descriptor_mapping_digest: foundation.descriptor_mapping_digest(),
            identity_digest: lease.foundation_id().digest(),
            logical_binding_digest: foundation.logical_binding_digest(),
            model_package_manifest_id: foundation.model_package_manifest_id().digest(),
            package_source_id: foundation.package_source_id().digest(),
            provenance_manifest: member(foundation.provenance_manifest()),
        },
        license_members: foundation.license_members().iter().map(member).collect(),
        permission: ModelLicensePermission::LocalGeneration,
        procedure_id: MODEL_LICENSE_REVIEW_PROCEDURE_ID,
        procedure_version: MODEL_LICENSE_REVIEW_PROCEDURE_VERSION,
        schema_version: MODEL_LICENSE_REVIEW_SCHEMA_VERSION,
        source: Source {
            kind: source.kind(),
            locator: source.locator(),
            provenance_digest: source.provenance_digest(),
            revision: source.revision(),
        },
    })
    .expect("serialize exact reviewer fixture")
}

fn launch<'a>(
    lease: &'a VerifiedManagedOllamaModelPackageLease,
    tag: &str,
) -> VerifiedManagedOllamaLaunchPlan<'a> {
    let review = reviewer_json(lease);
    let compiled = ModelLicenseControlCompiler::compile(
        lease,
        ModelLicensePermission::LocalGeneration,
        &review,
        &CancellationToken::new(),
    )
    .expect("compile exact license control");
    let approval = ProductionModelLicenseApprovalPolicy::exact_test_policy(
        compiled.control_id().clone(),
        ModelLicensePermission::LocalGeneration,
    );
    let license = ModelLicenseControlVerifier::verify(
        compiled.canonical_bytes(),
        lease,
        ModelLicensePermission::LocalGeneration,
        &approval,
        &CancellationToken::new(),
    )
    .expect("verify exact license authority");
    let reference = OllamaModelReference::new("registry.ollama.ai", "library", "fixture", tag)
        .expect("valid fixture reference");
    let input = PackageAttestationService::prepare_verified_managed_ollama_inputs(
        lease,
        &reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect("prepare exact managed input");
    PackageAttestationService::authorize_managed_ollama_v0_32_15_launch(lease, input, license)
        .expect("authorize exact launch")
}

#[test]
fn verified_launch_derives_canonical_redacted_interpretation() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let launch = launch(&lease, "v1");
    let record = StaticModelInterpretationV1::derive(&launch).expect("derive interpretation");
    let bytes = record.canonical_json_bytes().expect("canonical record");

    assert_eq!(record.foundation_id(), launch.foundation_id());
    assert_eq!(
        record.artifact_set_id(),
        launch.input_evidence().artifact_set_id()
    );
    assert_eq!(
        record.model_package_manifest_id(),
        launch.input_evidence().model_package_manifest_id()
    );
    assert_eq!(
        record.model_artifact_id(),
        launch.model_target().artifact_id()
    );
    assert_eq!(record.binding_digest(), record.interpretation_id().digest());
    assert_eq!(
        record.binding_digest().as_str(),
        "5af638e3dba0255443bcad1607c05165d6b4ff76ef7a3d9dee178f6dbc544da7"
    );
    assert_eq!(
        StaticModelInterpretationV1::from_json_bytes(&bytes, &launch)
            .expect("reload exact canonical record"),
        record
    );
    record
        .validate_against(&launch)
        .expect("validate exact record");
    let debug = format!("{record:?}");
    assert!(debug.contains("interpretation_id"));
    assert!(!debug.contains("reference_digest"));
    assert!(!debug.contains("mapping_digest"));
    assert!(!debug.contains("installation"));
}

#[test]
fn installation_and_object_identity_do_not_change_interpretation() {
    let mut fixture = Fixture::new(false);
    let first = {
        let lease = fixture.verify();
        StaticModelInterpretationV1::derive(&launch(&lease, "v1")).expect("first interpretation")
    };
    fixture.advance_generation();
    let second = {
        let lease = fixture.verify();
        StaticModelInterpretationV1::derive(&launch(&lease, "v1")).expect("second interpretation")
    };
    assert_eq!(first, second);
}

#[test]
fn changed_reference_changes_interpretation_and_validation() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let first_launch = launch(&lease, "v1");
    let first = StaticModelInterpretationV1::derive(&first_launch).expect("first");
    drop(first_launch);
    let second_launch = launch(&lease, "v2");
    let second = StaticModelInterpretationV1::derive(&second_launch).expect("second");
    assert_ne!(first, second);
    assert_eq!(
        first.validate_against(&second_launch),
        Err(StaticModelInterpretationError::InvalidBinding)
    );
}

#[test]
fn drifted_foundation_cannot_supply_later_static_interpretation() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let launch = launch(&lease, "v1");
    StaticModelInterpretationV1::derive(&launch).expect("initial interpretation");
    drop(launch);
    fixture.add_unexpected_member();
    assert!(lease.revalidate(&CancellationToken::new()).is_err());
}

#[test]
fn every_encoded_field_is_relationship_checked() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let launch = launch(&lease, "v1");
    let record = StaticModelInterpretationV1::derive(&launch).expect("record");
    let original: Value =
        serde_json::from_slice(&record.canonical_json_bytes().expect("canonical record"))
            .expect("record JSON");
    let changed = Digest::sha256(b"changed").as_str().to_owned();
    let mutations = [
        ("foundation_id", json!(changed)),
        ("managed_input_schema_version", json!(99)),
        ("artifact_set_id", json!(changed)),
        ("model_package_manifest_id", json!(changed)),
        ("reference_digest", json!(changed)),
        ("runtime_reference_digest", json!(changed)),
        ("mapping_digest", json!(changed)),
        ("model_artifact_id", json!(changed)),
        ("model_target_digest", json!(changed)),
        ("member_count", json!(99)),
        ("total_bytes", json!(99)),
        ("model_bytes", json!(99)),
    ];
    for (field, replacement) in mutations {
        let mut value = original.clone();
        value[field] = replacement;
        let bytes = serde_json::to_vec(&value).expect("mutated JSON");
        assert_eq!(
            StaticModelInterpretationV1::from_json_bytes(&bytes, &launch),
            Err(StaticModelInterpretationError::InvalidBinding),
            "field {field}"
        );
    }

    let mut schema = original;
    schema["schema_version"] = json!(2);
    assert_eq!(
        StaticModelInterpretationV1::from_json_bytes(
            &serde_json::to_vec(&schema).expect("schema JSON"),
            &launch,
        ),
        Err(StaticModelInterpretationError::UnsupportedSchema(2))
    );
}

#[test]
fn invalid_verified_fact_relationships_fail_closed() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let launch = launch(&lease, "v1");
    let assert_invalid = |facts| {
        assert!(matches!(
            StaticModelInterpretationV1::derive_from_facts(facts),
            Err(StaticModelInterpretationError::InvalidBinding)
        ));
    };

    let mut facts = facts_from_launch(&launch);
    facts.managed_input_schema_version += 1;
    assert_invalid(facts);
    let mut facts = facts_from_launch(&launch);
    facts.target_artifact_id = ArtifactId::from_digest(Digest::sha256(b"other target"));
    assert_invalid(facts);
    let mut facts = facts_from_launch(&launch);
    facts.target_digest = Digest::sha256(b"other target digest");
    assert_invalid(facts);
    let mut facts = facts_from_launch(&launch);
    facts.member_count = 0;
    assert_invalid(facts);
    let mut facts = facts_from_launch(&launch);
    facts.total_bytes = 0;
    assert_invalid(facts);
    let mut facts = facts_from_launch(&launch);
    facts.model_bytes = 0;
    assert_invalid(facts);
    let mut facts = facts_from_launch(&launch);
    facts.model_bytes = facts.total_bytes + 1;
    assert_invalid(facts);
}

#[test]
fn every_accepted_stable_fact_changes_the_content_identity() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let primary_launch = launch(&lease, "v1");
    let original = StaticModelInterpretationV1::derive(&primary_launch).expect("original");
    let assert_changed = |facts| {
        let changed = StaticModelInterpretationV1::derive_from_facts(facts)
            .expect("changed facts remain structurally valid");
        assert_ne!(changed.interpretation_id(), original.interpretation_id());
    };

    let other_fixture = Fixture::new(true);
    let other_lease = other_fixture.verify();
    let other_launch = launch(&other_lease, "v1");
    let mut facts = facts_from_launch(&primary_launch);
    facts.foundation_id = other_launch.foundation_id().clone();
    assert_changed(facts);

    let mut facts = facts_from_launch(&primary_launch);
    facts.artifact_set_id = rewrite_model::ArtifactSetId::from_digest(digest("other set"));
    assert_changed(facts);
    let mut facts = facts_from_launch(&primary_launch);
    facts.model_package_manifest_id =
        serde_json::from_value(json!(digest("other package"))).expect("model-package ID fixture");
    assert_changed(facts);
    let mut facts = facts_from_launch(&primary_launch);
    facts.reference_digest = digest("other reference");
    assert_changed(facts);
    let mut facts = facts_from_launch(&primary_launch);
    facts.runtime_reference_digest = digest("other runtime reference");
    assert_changed(facts);
    let mut facts = facts_from_launch(&primary_launch);
    facts.mapping_digest = digest("other mapping");
    assert_changed(facts);
    let mut facts = facts_from_launch(&primary_launch);
    let artifact = ArtifactId::from_digest(digest("other model artifact"));
    facts.model_artifact_id = artifact.clone();
    facts.target_artifact_id = artifact;
    assert_changed(facts);
    let mut facts = facts_from_launch(&primary_launch);
    let target = digest("other target");
    facts.model_target_digest = target.clone();
    facts.target_digest = target;
    assert_changed(facts);
    let mut facts = facts_from_launch(&primary_launch);
    facts.member_count += 1;
    assert_changed(facts);
    let mut facts = facts_from_launch(&primary_launch);
    facts.total_bytes += 1;
    assert_changed(facts);
    let mut facts = facts_from_launch(&primary_launch);
    facts.model_bytes += 1;
    assert!(facts.model_bytes <= facts.total_bytes);
    assert_changed(facts);
}

#[test]
fn strict_decoder_rejects_noncanonical_unknown_malformed_and_oversized_input() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let launch = launch(&lease, "v1");
    let record = StaticModelInterpretationV1::derive(&launch).expect("record");
    let pretty = serde_json::to_vec_pretty(&record).expect("pretty JSON");
    assert_eq!(
        StaticModelInterpretationV1::from_json_bytes(&pretty, &launch),
        Err(StaticModelInterpretationError::NonCanonicalEncoding)
    );
    let mut unknown: Value = serde_json::to_value(&record).expect("record value");
    unknown["unknown"] = json!(true);
    assert_eq!(
        StaticModelInterpretationV1::from_json_bytes(
            &serde_json::to_vec(&unknown).expect("unknown JSON"),
            &launch,
        ),
        Err(StaticModelInterpretationError::InvalidEncoding)
    );
    for malformed in [b"".as_slice(), b"{".as_slice(), b"null".as_slice()] {
        assert_eq!(
            StaticModelInterpretationV1::from_json_bytes(malformed, &launch),
            Err(StaticModelInterpretationError::InvalidEncoding)
        );
    }
    assert_eq!(
        StaticModelInterpretationV1::from_json_bytes(
            &vec![b' '; MAX_STATIC_MODEL_INTERPRETATION_JSON_BYTES + 1],
            &launch,
        ),
        Err(StaticModelInterpretationError::LimitExceeded)
    );
}
