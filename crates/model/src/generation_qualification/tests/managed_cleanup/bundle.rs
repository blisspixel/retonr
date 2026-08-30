use serde_json::Value;

use super::*;
use crate::ArtifactSetRelativePath;

#[path = "bundle/adversarial.rs"]
mod adversarial;
#[path = "bundle/receipt_attempt.rs"]
mod receipt_attempt;

struct BundleFixture {
    managed: ManagedFixture,
    cleanup: CandidateGenerationCleanupRecordV1,
    response_artifact: StructuredResponseArtifactV1Input,
    candidates: Vec<CandidateArtifactEntryV1>,
    bundle: CandidateGenerationEvidenceBundleManifestV1,
}

impl BundleFixture {
    fn relations(&self) -> CandidateGenerationEvidenceBundleManifestV1Relations<'_> {
        CandidateGenerationEvidenceBundleManifestV1Relations {
            qualification_plan: &self.managed.base.plan,
            planned_attempt: &self.managed.base.attempts[0],
            precursor: &self.managed.precursor,
            managed_evidence: &self.managed.evidence,
            cleanup: &self.cleanup,
            structured_response_artifact: &self.response_artifact,
        }
    }
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("fixture path")
}

fn entry(
    relative_path: &str,
    role: CandidateGenerationEvidenceBundleRoleV1,
    bytes: &[u8],
) -> CandidateGenerationEvidenceBundleEntryV1 {
    CandidateGenerationEvidenceBundleEntryV1::new(
        path(relative_path),
        role,
        ArtifactId::from_digest(Digest::sha256(bytes)),
        u64::try_from(bytes.len()).expect("fixture size"),
    )
}

fn successful_cleanup(managed: &ManagedFixture) -> CandidateGenerationCleanupRecordV1 {
    CandidateGenerationCleanupRecordV1::new(
        &managed.precursor,
        &managed.evidence,
        CandidateGenerationCleanupRecordV1Input {
            process_cleanup_status: CandidateGenerationProcessCleanupStatusV1::Succeeded,
            runtime_package_revalidation_status:
                CandidateGenerationPackageRevalidationStatusV1::Verified,
            model_package_revalidation_status:
                CandidateGenerationPackageRevalidationStatusV1::Verified,
        },
    )
    .expect("successful cleanup")
}

fn candidate(
    managed: &ManagedFixture,
    ordinal: u8,
    relative_path: &str,
    bytes: &[u8],
) -> CandidateArtifactEntryV1 {
    CandidateArtifactEntryV1::new(
        &managed.precursor,
        &managed.base.attempts[0],
        &managed.base.cases[0],
        ordinal,
        path(relative_path),
        bytes,
    )
    .expect("candidate")
}

fn record_entry<T: serde::Serialize>(
    relative_path: &str,
    role: CandidateGenerationEvidenceBundleRoleV1,
    record: &T,
) -> CandidateGenerationEvidenceBundleEntryV1 {
    let bytes = serde_json::to_vec(record).expect("record JSON");
    entry(relative_path, role, &bytes)
}

fn valid_entries(
    managed: &ManagedFixture,
    cleanup: &CandidateGenerationCleanupRecordV1,
    candidates: &[CandidateArtifactEntryV1],
) -> Vec<CandidateGenerationEvidenceBundleEntryV1> {
    let mut entries = vec![
        record_entry(
            "records/planned.json",
            CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt,
            &managed.base.attempts[0],
        ),
        record_entry(
            "records/precursor.json",
            CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor,
            &managed.precursor,
        ),
        record_entry(
            "records/managed.json",
            CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence,
            &managed.evidence,
        ),
        entry(
            "records/response.json",
            CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
            b"response",
        ),
        record_entry(
            "records/cleanup.json",
            CandidateGenerationEvidenceBundleRoleV1::CleanupRecord,
            cleanup,
        ),
    ];
    entries.extend(candidates.iter().map(|candidate| {
        CandidateGenerationEvidenceBundleEntryV1::new(
            candidate.relative_path().clone(),
            CandidateGenerationEvidenceBundleRoleV1::Candidate,
            candidate.artifact_id().clone(),
            candidate.byte_count(),
        )
    }));
    entries.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
    entries
}

fn bundle_fixture() -> BundleFixture {
    let managed = managed_fixture(0);
    let cleanup = successful_cleanup(&managed);
    let response_artifact = StructuredResponseArtifactV1Input::new(
        ArtifactId::from_digest(Digest::sha256(b"response")),
        8,
    )
    .expect("response artifact");
    let candidates = vec![
        candidate(&managed, 0, "candidates/000.txt", b"first candidate"),
        candidate(&managed, 1, "candidates/001.txt", b"second candidate"),
    ];
    let bundle = CandidateGenerationEvidenceBundleManifestV1::new(
        CandidateGenerationEvidenceBundleManifestV1Relations {
            qualification_plan: &managed.base.plan,
            planned_attempt: &managed.base.attempts[0],
            precursor: &managed.precursor,
            managed_evidence: &managed.evidence,
            cleanup: &cleanup,
            structured_response_artifact: &response_artifact,
        },
        valid_entries(&managed, &cleanup, &candidates),
        candidates.clone(),
    )
    .expect("bundle");
    BundleFixture {
        managed,
        cleanup,
        response_artifact,
        candidates,
        bundle,
    }
}

#[test]
fn candidate_derivation_and_bundle_round_trip_bind_exact_facts() {
    let fixture = bundle_fixture();
    let first = &fixture.candidates[0];
    assert_eq!(first.ordinal(), 0);
    assert_eq!(first.byte_count(), 15);
    assert_eq!(
        first.artifact_id().digest(),
        &Digest::sha256(b"first candidate")
    );
    let mut separated = CANDIDATE_CONTENT_DIGEST_DOMAIN.to_vec();
    separated.extend_from_slice(b"first candidate");
    assert_eq!(first.candidate_digest(), &Digest::sha256(&separated));
    assert_ne!(first.candidate_digest(), first.artifact_id().digest());
    let candidate_debug = format!("{first:?}");
    assert!(candidate_debug.contains(first.candidate_evidence_id().digest().as_str()));
    assert!(!candidate_debug.contains(first.relative_path().as_str()));
    assert!(!candidate_debug.contains(first.artifact_id().digest().as_str()));
    let bundle = &fixture.bundle;
    assert_eq!(bundle.schema_version(), 1);
    assert_eq!(
        bundle.precursor_id(),
        fixture.managed.precursor.precursor_id()
    );
    assert_eq!(
        bundle.managed_evidence_id(),
        fixture.managed.evidence.managed_evidence_v2_id()
    );
    assert_eq!(bundle.response_id(), fixture.managed.evidence.response_id());
    assert_eq!(bundle.cleanup_id(), fixture.cleanup.cleanup_id());
    assert_eq!(bundle.entry_count(), 7);
    assert_eq!(bundle.candidate_artifacts(), fixture.candidates);
    assert_eq!(
        bundle
            .entries()
            .iter()
            .map(CandidateGenerationEvidenceBundleEntryV1::byte_size)
            .sum::<u64>(),
        bundle.aggregate_byte_count()
    );
    let encoded = bundle
        .to_canonical_json_bytes()
        .expect("canonical bundle JSON");
    assert_eq!(
        bundle.rederive_evidence_bundle_id().expect("rederive ID"),
        *bundle.evidence_bundle_id()
    );
    assert_eq!(encoded, serde_json::to_vec(bundle).expect("bundle JSON"));
    let preflight = CandidateGenerationEvidenceBundleManifestV1::preflight_json_bytes(&encoded)
        .expect("bounded manifest preflight");
    let derived_response = preflight
        .structured_response_artifact()
        .expect("unique response metadata");
    assert_eq!(
        derived_response.artifact_id(),
        fixture.response_artifact.artifact_id(),
    );
    assert_eq!(
        derived_response.byte_size(),
        fixture.response_artifact.byte_size(),
    );
    let preflight_debug = format!("{preflight:?}");
    assert!(preflight_debug.contains("entry_count: 7"));
    assert!(preflight_debug.contains("candidate_count: 2"));
    assert!(!preflight_debug.contains(first.candidate_evidence_id().digest().as_str()));
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_preflight(
            preflight,
            CandidateGenerationEvidenceBundleManifestV1Relations {
                qualification_plan: &fixture.managed.base.plan,
                planned_attempt: &fixture.managed.base.attempts[0],
                precursor: &fixture.managed.precursor,
                managed_evidence: &fixture.managed.evidence,
                cleanup: &fixture.cleanup,
                structured_response_artifact: &derived_response,
            },
        )
        .expect("decode preflight"),
        *bundle,
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            &encoded,
            fixture.relations(),
            &fixture.candidates
        )
        .expect("decode"),
        *bundle
    );
    let debug = format!("{bundle:?}");
    assert!(debug.contains(bundle.evidence_bundle_id().digest().as_str()));
    assert!(!debug.contains(first.relative_path().as_str()));
    assert!(!debug.contains(first.artifact_id().digest().as_str()));
}

#[test]
fn bundle_allows_shared_content_at_distinct_paths_and_separates_identities() {
    let fixture = bundle_fixture();
    let mut entries = valid_entries(&fixture.managed, &fixture.cleanup, &fixture.candidates);
    entries.push(CandidateGenerationEvidenceBundleEntryV1::new(
        path("z-aux/shared.bin"),
        CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation,
        entries[0].artifact_id().clone(),
        entries[0].byte_size(),
    ));
    let shared = CandidateGenerationEvidenceBundleManifestV1::new(
        fixture.relations(),
        entries,
        fixture.candidates.clone(),
    )
    .expect("shared content");
    assert_ne!(
        shared.evidence_bundle_id(),
        fixture.bundle.evidence_bundle_id()
    );
    let changed_candidates = vec![
        candidate(
            &fixture.managed,
            0,
            "candidates/000.txt",
            b"changed candidate",
        ),
        fixture.candidates[1].clone(),
    ];
    let changed = CandidateGenerationEvidenceBundleManifestV1::new(
        fixture.relations(),
        valid_entries(&fixture.managed, &fixture.cleanup, &changed_candidates),
        changed_candidates,
    )
    .expect("changed candidate");
    assert_ne!(
        changed.evidence_bundle_id(),
        fixture.bundle.evidence_bundle_id()
    );
}

#[test]
fn readback_round_trips_and_rejects_substitution() {
    let fixture = bundle_fixture();
    let readback =
        CandidateGenerationEvidenceBundleReadbackV1::new(&fixture.bundle).expect("readback");
    assert_eq!(readback.schema_version(), 1);
    assert_eq!(readback.bundle_id(), fixture.bundle.evidence_bundle_id());
    assert_eq!(readback.entry_count(), fixture.bundle.entry_count());
    assert_eq!(
        readback.aggregate_byte_count(),
        fixture.bundle.aggregate_byte_count()
    );
    assert_eq!(
        readback.publication_mode(),
        CandidateGenerationEvidenceBundlePublicationModeV1::CreateNewNoReplace
    );
    assert_eq!(
        readback.status(),
        CandidateGenerationEvidenceBundleReadbackStatusV1::Verified
    );
    let encoded = serde_json::to_vec(&readback).expect("readback JSON");
    assert_eq!(
        CandidateGenerationEvidenceBundleReadbackV1::from_json_bytes(&encoded, &fixture.bundle)
            .expect("decode"),
        readback
    );
    let changed = replace_once(&encoded, "\"entry_count\":7", "\"entry_count\":8");
    assert_eq!(
        CandidateGenerationEvidenceBundleReadbackV1::from_json_bytes(&changed, &fixture.bundle),
        Err(GenerationQualificationContractError::ReadbackRelationshipMismatch)
    );
    let future = replace_once(&encoded, "\"schema_version\":1", "\"schema_version\":2");
    assert_eq!(
        CandidateGenerationEvidenceBundleReadbackV1::from_json_bytes(&future, &fixture.bundle),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
    let mut spaced = encoded.clone();
    spaced.push(b' ');
    assert_eq!(
        CandidateGenerationEvidenceBundleReadbackV1::from_json_bytes(&spaced, &fixture.bundle),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );
    let invalid = replace_once(&encoded, "\"status\":\"verified\"", "\"status\":\"failed\"");
    assert_eq!(
        CandidateGenerationEvidenceBundleReadbackV1::from_json_bytes(&invalid, &fixture.bundle),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleReadbackV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_READBACK_JSON_BYTES + 1],
            &fixture.bundle,
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        CandidateGenerationEvidenceBundleReadbackV1::from_json_bytes(b"{", &fixture.bundle),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let debug = format!("{readback:?}");
    assert!(debug.contains(readback.readback_id().digest().as_str()));
    assert!(!debug.contains(fixture.bundle.response_id().digest().as_str()));
}

#[test]
fn candidate_bundle_and_readback_ids_match_stable_vectors() {
    let fixture = bundle_fixture();
    let readback =
        CandidateGenerationEvidenceBundleReadbackV1::new(&fixture.bundle).expect("readback");
    assert_eq!(
        (
            fixture.candidates[0]
                .candidate_evidence_id()
                .digest()
                .as_str(),
            fixture.bundle.evidence_bundle_id().digest().as_str(),
            readback.readback_id().digest().as_str()
        ),
        (
            "6d2bd05668ceb0020949402f7d6c63f01aa1060b35fe4beb8615f7a058b6d2ce",
            "008ca35b33bb307dbc2ae51e4ef4657d5d7a2c29cdefebc04c4c91b029121dc8",
            "0bb218bbfb718ce69c6fcfa9fc3f696876004cae9b89eed076833f8fc8c6f3b7"
        ),
    );
    assert_eq!(
        fixture
            .bundle
            .rederive_evidence_bundle_id()
            .expect("rederive frozen bundle ID")
            .digest()
            .as_str(),
        "008ca35b33bb307dbc2ae51e4ef4657d5d7a2c29cdefebc04c4c91b029121dc8"
    );
}

#[test]
fn wire_field_order_and_closed_role_tags_are_stable() {
    let fixture = bundle_fixture();
    let value = serde_json::to_value(&fixture.bundle).expect("value");
    let encoded =
        String::from_utf8(serde_json::to_vec(&fixture.bundle).expect("JSON")).expect("UTF-8");
    let fields = [
        "schema_version",
        "precursor_id",
        "managed_evidence_id",
        "response_id",
        "cleanup_id",
        "entries",
        "candidate_artifacts",
        "entry_count",
        "aggregate_byte_count",
    ];
    let positions = fields.map(|field| encoded.find(&format!("\"{field}\"")).expect("field"));
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        serde_json::to_value([
            CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt,
            CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor,
            CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence,
            CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
            CandidateGenerationEvidenceBundleRoleV1::CleanupRecord,
            CandidateGenerationEvidenceBundleRoleV1::Candidate,
            CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation,
        ])
        .expect("roles"),
        serde_json::json!([
            "planned_attempt",
            "attempt_precursor",
            "managed_generation_evidence",
            "structured_response",
            "cleanup_record",
            "candidate",
            "auxiliary_observation"
        ])
    );
    assert!(matches!(value, Value::Object(_)));
}
