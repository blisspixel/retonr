use rewrite_model::{GenerationQualificationPlanId, PlannedCandidateAttemptId};
use rewrite_types::Digest;
use rusqlite::params;
use serde::de::DeserializeOwned;

use super::{CandidateGenerationAttemptAdmissionClassV1, CandidateGenerationEvidenceStorageRootId};
use crate::{ArtifactStateStore, StoreError};

#[test]
fn pristine_checkpoint_and_failed_attempts_keep_their_indexed_classes() {
    let fixture = Fixture::open();
    let plan_id = id("plan");
    let attempt_id = id("attempt");
    let root_id = root("root");
    fixture.insert_plan(&plan_id, &attempt_id);

    assert_eq!(
        class(&fixture, &plan_id, &attempt_id, &root_id),
        CandidateGenerationAttemptAdmissionClassV1::NotStarted
    );

    fixture.insert_precursor(&plan_id, &attempt_id, "precursor");
    assert_eq!(
        class(&fixture, &plan_id, &attempt_id, &root_id),
        CandidateGenerationAttemptAdmissionClassV1::CheckpointOnly
    );

    fixture.insert_failed(&plan_id, &attempt_id, Some("precursor"));
    assert_eq!(
        class(&fixture, &plan_id, &attempt_id, &root_id),
        CandidateGenerationAttemptAdmissionClassV1::TerminalFailed
    );
}

#[test]
fn failed_attempt_without_precursor_is_terminal_and_extra_rows_are_ambiguous() {
    let fixture = Fixture::open();
    let plan_id = id("plan-failed");
    let attempt_id = id("attempt-failed");
    fixture.insert_plan(&plan_id, &attempt_id);
    fixture.insert_failed(&plan_id, &attempt_id, None);
    assert_eq!(
        class(&fixture, &plan_id, &attempt_id, &root("root")),
        CandidateGenerationAttemptAdmissionClassV1::TerminalFailed
    );

    let ambiguous = Fixture::open();
    let ambiguous_plan = id("plan-ambiguous");
    let ambiguous_attempt = id("attempt-ambiguous");
    ambiguous.insert_plan(&ambiguous_plan, &ambiguous_attempt);
    ambiguous.insert_precursor(&ambiguous_plan, &ambiguous_attempt, "precursor");
    ambiguous.insert_failed(&ambiguous_plan, &ambiguous_attempt, Some("precursor"));
    ambiguous.insert_managed("precursor", "managed-field");
    assert_eq!(
        class(
            &ambiguous,
            &ambiguous_plan,
            &ambiguous_attempt,
            &root("root")
        ),
        CandidateGenerationAttemptAdmissionClassV1::Ambiguous
    );
}

#[test]
fn completed_chain_matches_only_the_caller_storage_root() {
    let fixture = Fixture::open();
    let plan_id = id("plan-completed");
    let attempt_id = id("attempt-completed");
    fixture.insert_plan(&plan_id, &attempt_id);
    fixture.insert_completed_chain(&plan_id, &attempt_id, "root");

    let matched = fixture.admit(&plan_id, std::slice::from_ref(&attempt_id), &root("root"));
    assert_eq!(
        matched[0].class(),
        CandidateGenerationAttemptAdmissionClassV1::TerminalCompleted
    );
    assert_eq!(
        matched[0]
            .evidence_bundle_id()
            .map(|bundle| bundle.digest().as_str()),
        Some(digest("bundle").as_str())
    );
    assert_eq!(
        class(&fixture, &plan_id, &attempt_id, &root("other-root")),
        CandidateGenerationAttemptAdmissionClassV1::Ambiguous
    );
}

#[test]
fn admission_read_does_not_write_and_rejects_bad_caller_slices() {
    let fixture = Fixture::open();
    let plan_id = id("plan-bounds");
    let attempt_id = id("attempt-bounds");
    fixture.insert_plan(&plan_id, &attempt_id);
    let before = fixture.store.connection.total_changes();
    assert_eq!(
        class(&fixture, &plan_id, &attempt_id, &root("root")),
        CandidateGenerationAttemptAdmissionClassV1::NotStarted
    );
    assert_eq!(fixture.store.connection.total_changes(), before);

    assert!(matches!(
        fixture
            .store
            .candidate_generation_attempt_admission_v1(&plan_id, &[], &root("root")),
        Err(StoreError::CorruptRecord)
    ));
    let oversized = vec![attempt_id; 1_025];
    assert!(matches!(
        fixture.store.candidate_generation_attempt_admission_v1(
            &plan_id,
            &oversized,
            &root("root")
        ),
        Err(StoreError::RecordTooLarge)
    ));
    assert!(matches!(
        fixture.store.candidate_generation_attempt_admission_v1(
            &plan_id,
            &[id("substitute")],
            &root("root")
        ),
        Err(StoreError::CorruptRecord)
    ));
}

struct Fixture {
    store: ArtifactStateStore,
    _directory: tempfile::TempDir,
}

impl Fixture {
    fn open() -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ArtifactStateStore::open(&directory.path().join("state.db")).expect("store");
        store
            .connection
            .execute_batch("PRAGMA foreign_keys = OFF;")
            .expect("disable foreign keys");
        Self {
            store,
            _directory: directory,
        }
    }

    fn insert_plan(
        &self,
        plan_id: &GenerationQualificationPlanId,
        attempt_id: &PlannedCandidateAttemptId,
    ) {
        self.store
            .connection
            .execute(
                "INSERT INTO generation_qualification_plans (
                     generation_qualification_plan_id,
                     generation_suite_manifest_id,
                     candidate_selection_policy_id,
                     repetition_count,
                     generation_system_count,
                     planned_attempt_count,
                     canonical_json
                 ) VALUES (?1, ?2, ?2, 1, 1, 1, x'ff')",
                params![plan_id.digest().as_str(), digest("suite")],
            )
            .expect("insert plan");
        self.store
            .connection
            .execute(
                "INSERT INTO generation_qualification_plan_attempts (
                     generation_qualification_plan_id,
                     plan_ordinal,
                     planned_candidate_attempt_id
                 ) VALUES (?1, 0, ?2)",
                params![plan_id.digest().as_str(), attempt_id.digest().as_str()],
            )
            .expect("insert plan attempt");
    }

    fn insert_precursor(
        &self,
        plan_id: &GenerationQualificationPlanId,
        attempt_id: &PlannedCandidateAttemptId,
        label: &str,
    ) {
        self.store
            .connection
            .execute(
                "INSERT INTO candidate_generation_attempt_precursors (
                     candidate_generation_attempt_precursor_id,
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id,
                     structured_request_binding_id,
                     canonical_json
                 ) VALUES (?1, ?2, ?3, ?4, x'ff')",
                params![
                    digest(label),
                    plan_id.digest().as_str(),
                    attempt_id.digest().as_str(),
                    digest("request")
                ],
            )
            .expect("insert precursor");
    }

    fn insert_failed(
        &self,
        plan_id: &GenerationQualificationPlanId,
        attempt_id: &PlannedCandidateAttemptId,
        precursor: Option<&str>,
    ) {
        self.store
            .connection
            .execute(
                "INSERT INTO candidate_generation_attempt_records (
                     candidate_generation_attempt_record_id,
                     generation_qualification_plan_id,
                     planned_candidate_attempt_id,
                     outcome,
                     candidate_generation_attempt_precursor_id,
                     candidate_generation_receipt_id,
                     canonical_json
                 ) VALUES (?1, ?2, ?3, 'failed', ?4, NULL, x'ff')",
                params![
                    digest("attempt-record"),
                    plan_id.digest().as_str(),
                    attempt_id.digest().as_str(),
                    precursor.map(digest)
                ],
            )
            .expect("insert failed attempt");
    }

    fn insert_managed(&self, precursor: &str, response: &str) {
        self.store
            .connection
            .execute(
                "INSERT INTO managed_candidate_generation_evidence (
                     managed_candidate_generation_evidence_id,
                     candidate_generation_attempt_precursor_id,
                     bracket_observation_v1_id,
                     effective_package_evidence_v2_id,
                     effective_runtime_state_id,
                     effective_runtime_state_join_id,
                     generation_request_binding_id,
                     structured_request_binding_id,
                     response_id,
                     canonical_json
                 ) VALUES (?1, ?2, ?3, ?3, ?3, ?3, ?3, ?3, ?4, x'ff')",
                params![
                    digest("managed"),
                    digest(precursor),
                    digest("managed-field"),
                    digest(response)
                ],
            )
            .expect("insert managed evidence");
    }

    fn insert_completed_chain(
        &self,
        plan_id: &GenerationQualificationPlanId,
        attempt_id: &PlannedCandidateAttemptId,
        root_label: &str,
    ) {
        let precursor = digest("precursor");
        let managed = digest("managed");
        let cleanup = digest("cleanup");
        let bundle = digest("bundle");
        let response = digest("response");
        self.insert_precursor(plan_id, attempt_id, "precursor");
        self.insert_managed("precursor", "response");
        self.execute(
            "INSERT INTO candidate_generation_cleanup_records (
                 candidate_generation_cleanup_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id,
                 canonical_json
             ) VALUES (?1, ?2, ?3, x'ff')",
            params![cleanup, precursor, managed],
        );
        self.execute(
            "INSERT INTO generation_evidence_bundles (
                 candidate_generation_evidence_bundle_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id,
                 response_id,
                 candidate_generation_cleanup_id,
                 canonical_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, x'ff')",
            params![bundle, precursor, managed, response, cleanup],
        );
        self.insert_completed_closeout(plan_id, attempt_id, root_label);
    }

    fn insert_completed_closeout(
        &self,
        plan_id: &GenerationQualificationPlanId,
        attempt_id: &PlannedCandidateAttemptId,
        root_label: &str,
    ) {
        let precursor = digest("precursor");
        let managed = digest("managed");
        let cleanup = digest("cleanup");
        let bundle = digest("bundle");
        let readback = digest("readback");
        let receipt = digest("receipt");
        let reference = format!(
            "bundles/v1/{}/{}/{}",
            plan_id.digest().as_str(),
            attempt_id.digest().as_str(),
            bundle
        );
        self.execute(
            "INSERT INTO generation_evidence_bundle_storage (
                 candidate_generation_evidence_bundle_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id,
                 storage_root_id,
                 relative_reference,
                 maximum_tree_entries,
                 maximum_tree_depth,
                 maximum_aggregate_bytes
             ) VALUES (?1, ?2, ?3, ?4, ?5, 8, 4, 1024)",
            params![
                bundle,
                plan_id.digest().as_str(),
                attempt_id.digest().as_str(),
                digest(root_label),
                reference
            ],
        );
        self.execute(
            "INSERT INTO generation_evidence_bundle_readbacks (
                 candidate_generation_evidence_bundle_readback_id,
                 candidate_generation_evidence_bundle_id,
                 canonical_json
             ) VALUES (?1, ?2, x'ff')",
            params![readback, bundle],
        );
        self.execute(
            "INSERT INTO candidate_generation_receipts (
                 candidate_generation_receipt_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id,
                 candidate_generation_cleanup_id,
                 candidate_generation_evidence_bundle_id,
                 candidate_generation_evidence_bundle_readback_id,
                 canonical_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, x'ff')",
            params![
                receipt,
                plan_id.digest().as_str(),
                attempt_id.digest().as_str(),
                precursor,
                managed,
                cleanup,
                bundle,
                readback
            ],
        );
        self.execute(
            "INSERT INTO candidate_generation_attempt_records (
                 candidate_generation_attempt_record_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id,
                 outcome,
                 candidate_generation_attempt_precursor_id,
                 candidate_generation_receipt_id,
                 canonical_json
             ) VALUES (?1, ?2, ?3, 'completed', ?4, ?5, x'ff')",
            params![
                digest("attempt-record"),
                plan_id.digest().as_str(),
                attempt_id.digest().as_str(),
                precursor,
                receipt
            ],
        );
    }

    fn execute(&self, sql: &str, values: impl rusqlite::Params) {
        self.store
            .connection
            .execute(sql, values)
            .expect("insert admission fixture");
    }

    fn admit(
        &self,
        plan_id: &GenerationQualificationPlanId,
        attempts: &[PlannedCandidateAttemptId],
        root_id: &CandidateGenerationEvidenceStorageRootId,
    ) -> Vec<super::CandidateGenerationAttemptAdmissionV1> {
        self.store
            .candidate_generation_attempt_admission_v1(plan_id, attempts, root_id)
            .expect("read admission")
    }
}

fn class(
    fixture: &Fixture,
    plan_id: &GenerationQualificationPlanId,
    attempt_id: &PlannedCandidateAttemptId,
    root_id: &CandidateGenerationEvidenceStorageRootId,
) -> CandidateGenerationAttemptAdmissionClassV1 {
    fixture.admit(plan_id, std::slice::from_ref(attempt_id), root_id)[0].class()
}

fn root(label: &str) -> CandidateGenerationEvidenceStorageRootId {
    CandidateGenerationEvidenceStorageRootId::new(digest(label)).expect("root id")
}

fn id<T: DeserializeOwned>(label: &str) -> T {
    serde_json::from_value(serde_json::Value::String(digest(label))).expect("typed digest")
}

fn digest(label: &str) -> String {
    Digest::sha256(label.as_bytes()).as_str().to_owned()
}
