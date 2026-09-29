use std::fs;
use std::path::Path;
use std::time::Duration;

use rewrite_app::{
    CandidateGenerationEvidenceRepository, GenerationQualificationLicenseAssessmentCompiler,
    SyntheticGenerationQualificationScenario, with_synthetic_generation_qualification_fixture,
};
use rewrite_types::Digest;
use rusqlite::{Connection, params};
use tempfile::tempdir;

use crate::{
    GenerationQualificationActivationError, GenerationQualificationOperationDraft,
    GenerationQualificationPreregistrationRepository,
};

const ADMISSION_TABLES: [&str; 8] = [
    "candidate_generation_attempt_precursors",
    "candidate_generation_attempt_records",
    "managed_candidate_generation_evidence",
    "candidate_generation_cleanup_records",
    "generation_evidence_bundles",
    "generation_evidence_bundle_storage",
    "generation_evidence_bundle_readbacks",
    "candidate_generation_receipts",
];

macro_rules! prepare_traffic {
    (
        $input:ident,
        $platform_owners:ident,
        $license_proof:ident,
        $license_policy:ident,
        $production_policy:ident,
        $cancellation:ident,
        $directory:ident,
        $repository:ident,
        $evidence:ident,
        $prepared:ident
    ) => {
        let $cancellation = rewrite_types::CancellationToken::new();
        let draft = GenerationQualificationOperationDraft::begin(
            $input.operation_policy_relations,
            $input.operation_policy_input,
            &$cancellation,
        )
        .expect("draft");
        let projected = draft
            .project($input.case_authorities, &$cancellation)
            .expect("projection");
        let platform = $platform_owners
            .assess(projected.platform_portable_relations(), &$cancellation)
            .expect("platform assessment");
        let assessment_input = projected.license_assessment_input(
            &$license_policy,
            $license_proof.control(),
            $license_proof.selected_lease(),
            &$production_policy,
        );
        let license = GenerationQualificationLicenseAssessmentCompiler::compile(
            &assessment_input,
            &$cancellation,
        )
        .expect("license assessment");
        let $directory = tempdir().expect("temporary repository directory");
        let mut $repository = GenerationQualificationPreregistrationRepository::open(
            &$directory.path().join("qualification.db"),
        )
        .expect("durable repository");
        let $prepared = projected
            .finish(
                &mut $repository,
                $input.foundation,
                platform,
                license,
                $license_policy,
                $production_policy,
                &$cancellation,
            )
            .expect("prepared operation");
        let $evidence = CandidateGenerationEvidenceRepository::initialize($directory.path())
            .expect("evidence root");
    };
}

#[test]
fn pristine_plan_activates_without_writing_a_precursor() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::TrafficEligible,
        |input, platform_owners, license_proof, license_policy, production_policy| {
            prepare_traffic!(
                input,
                platform_owners,
                license_proof,
                license_policy,
                production_policy,
                cancellation,
                directory,
                repository,
                evidence,
                prepared
            );
            let database = directory.path().join("qualification.db");
            let before = census(&database);
            assert_eq!(before[0], 0);
            let active = prepared
                .activate(&repository, &evidence, &cancellation)
                .expect("active owner");
            assert!(!active.ever_acquired_live_authority());
            assert_eq!(active.peak_live_authorities(), 0);
            assert_eq!(census(&database), before);
            assert!(
                directory
                    .path()
                    .join("generation-evidence")
                    .join(".staging")
                    .read_dir()
                    .expect("staging directory")
                    .next()
                    .is_none()
            );
        },
    );
}

#[test]
fn checkpoint_only_attempt_refuses_activation_without_adding_rows() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::TrafficEligible,
        |input, platform_owners, license_proof, license_policy, production_policy| {
            prepare_traffic!(
                input,
                platform_owners,
                license_proof,
                license_policy,
                production_policy,
                cancellation,
                directory,
                repository,
                evidence,
                prepared
            );
            let database = directory.path().join("qualification.db");
            let (plan_id, attempt_id) = plan_attempt(&prepared);
            insert_precursor(&database, &plan_id, &attempt_id);
            let before = census(&database);
            assert_eq!(
                prepared
                    .activate(&repository, &evidence, &cancellation)
                    .expect_err("checkpoint refusal"),
                GenerationQualificationActivationError::CandidateCheckpointOnly
            );
            assert_eq!(census(&database), before);
        },
    );
}

#[test]
fn publication_orphan_refuses_activation_and_keeps_the_directory() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::TrafficEligible,
        |input, platform_owners, license_proof, license_policy, production_policy| {
            prepare_traffic!(
                input,
                platform_owners,
                license_proof,
                license_policy,
                production_policy,
                cancellation,
                directory,
                repository,
                evidence,
                prepared
            );
            let database = directory.path().join("qualification.db");
            let (plan_id, attempt_id) = plan_attempt(&prepared);
            let orphan =
                bundle_directory(directory.path(), &plan_id, &attempt_id, &digest("bundle"));
            fs::create_dir_all(&orphan).expect("publication orphan");
            let before = census(&database);
            assert_eq!(
                prepared
                    .activate(&repository, &evidence, &cancellation)
                    .expect_err("orphan refusal"),
                GenerationQualificationActivationError::CandidatePublicationOrphan
            );
            assert_eq!(census(&database), before);
            assert!(orphan.is_dir());
        },
    );
}

#[test]
fn terminal_failed_and_completed_attempts_refuse_without_mutation() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::TrafficEligible,
        |input, platform_owners, license_proof, license_policy, production_policy| {
            prepare_traffic!(
                input,
                platform_owners,
                license_proof,
                license_policy,
                production_policy,
                cancellation,
                directory,
                repository,
                evidence,
                prepared
            );
            let database = directory.path().join("qualification.db");
            let (plan_id, attempt_id) = plan_attempt(&prepared);
            insert_failed(&database, &plan_id, &attempt_id);
            let before = census(&database);
            assert_eq!(
                prepared
                    .activate(&repository, &evidence, &cancellation)
                    .expect_err("failed refusal"),
                GenerationQualificationActivationError::CandidateTerminalFailed
            );
            assert_eq!(census(&database), before);
        },
    );
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::TrafficEligible,
        |input, platform_owners, license_proof, license_policy, production_policy| {
            prepare_traffic!(
                input,
                platform_owners,
                license_proof,
                license_policy,
                production_policy,
                cancellation,
                directory,
                repository,
                evidence,
                prepared
            );
            let database = directory.path().join("qualification.db");
            let (plan_id, attempt_id) = plan_attempt(&prepared);
            let bundle_id = digest("bundle");
            let bundle = bundle_directory(directory.path(), &plan_id, &attempt_id, &bundle_id);
            fs::create_dir_all(&bundle).expect("completed bundle");
            insert_completed(
                &database,
                &plan_id,
                &attempt_id,
                evidence.root_id().as_str(),
                &bundle_id,
            );
            let before = census(&database);
            assert_eq!(
                prepared
                    .activate(&repository, &evidence, &cancellation)
                    .expect_err("completed refusal"),
                GenerationQualificationActivationError::CandidateTerminalCompleted
            );
            assert_eq!(census(&database), before);
            assert!(bundle.is_dir());
        },
    );
}

fn plan_attempt(
    prepared: &crate::PreparedGenerationQualificationOperation<'_, '_, '_, '_, '_>,
) -> (String, String) {
    let plan = prepared.plan_foundation().plan();
    let attempt = plan.planned_attempt_ids().first().expect("planned attempt");
    (
        plan.qualification_plan_id().digest().as_str().to_owned(),
        attempt.digest().as_str().to_owned(),
    )
}

fn bundle_directory(
    data: &Path,
    plan_id: &str,
    attempt_id: &str,
    bundle_id: &str,
) -> std::path::PathBuf {
    data.join("generation-evidence")
        .join("bundles")
        .join("v1")
        .join(plan_id)
        .join(attempt_id)
        .join(bundle_id)
}

fn census(database: &Path) -> [i64; 8] {
    let connection = open_connection(database);
    let mut counts = [0; 8];
    for (index, table) in ADMISSION_TABLES.iter().enumerate() {
        counts[index] = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("census count");
    }
    counts
}

fn insert_precursor(database: &Path, plan_id: &str, attempt_id: &str) {
    open_connection(database)
        .execute(
            "INSERT INTO candidate_generation_attempt_precursors (
                 candidate_generation_attempt_precursor_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id,
                 structured_request_binding_id,
                 canonical_json
             ) VALUES (?1, ?2, ?3, ?4, x'ff')",
            params![digest("precursor"), plan_id, attempt_id, digest("request")],
        )
        .expect("insert precursor");
}

fn insert_failed(database: &Path, plan_id: &str, attempt_id: &str) {
    open_connection(database)
        .execute(
            "INSERT INTO candidate_generation_attempt_records (
                 candidate_generation_attempt_record_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id,
                 outcome,
                 candidate_generation_attempt_precursor_id,
                 candidate_generation_receipt_id,
                 canonical_json
             ) VALUES (?1, ?2, ?3, 'failed', NULL, NULL, x'ff')",
            params![digest("attempt-record"), plan_id, attempt_id],
        )
        .expect("insert failed attempt");
}

fn insert_completed(
    database: &Path,
    plan_id: &str,
    attempt_id: &str,
    root_id: &str,
    bundle_id: &str,
) {
    let connection = open_connection(database);
    let precursor = digest("precursor");
    let managed = digest("managed");
    let cleanup = digest("cleanup");
    let response = digest("response");
    connection
        .execute(
            "INSERT INTO candidate_generation_attempt_precursors (
                 candidate_generation_attempt_precursor_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id,
                 structured_request_binding_id,
                 canonical_json
             ) VALUES (?1, ?2, ?3, ?4, x'ff')",
            params![precursor, plan_id, attempt_id, digest("request")],
        )
        .expect("insert precursor");
    connection
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
            params![managed, precursor, digest("managed-field"), response],
        )
        .expect("insert managed evidence");
    connection
        .execute(
            "INSERT INTO candidate_generation_cleanup_records (
                 candidate_generation_cleanup_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id,
                 canonical_json
             ) VALUES (?1, ?2, ?3, x'ff')",
            params![cleanup, precursor, managed],
        )
        .expect("insert cleanup");
    connection
        .execute(
            "INSERT INTO generation_evidence_bundles (
                 candidate_generation_evidence_bundle_id,
                 candidate_generation_attempt_precursor_id,
                 managed_candidate_generation_evidence_id,
                 response_id,
                 candidate_generation_cleanup_id,
                 canonical_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, x'ff')",
            params![bundle_id, precursor, managed, response, cleanup],
        )
        .expect("insert bundle");
    insert_completed_closeout(&connection, plan_id, attempt_id, root_id, bundle_id);
}

fn insert_completed_closeout(
    connection: &Connection,
    plan_id: &str,
    attempt_id: &str,
    root_id: &str,
    bundle_id: &str,
) {
    let precursor = digest("precursor");
    let managed = digest("managed");
    let cleanup = digest("cleanup");
    let readback = digest("readback");
    let receipt = digest("receipt");
    let reference = format!("bundles/v1/{plan_id}/{attempt_id}/{bundle_id}");
    connection
        .execute(
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
            params![bundle_id, plan_id, attempt_id, root_id, reference],
        )
        .expect("insert storage");
    connection
        .execute(
            "INSERT INTO generation_evidence_bundle_readbacks (
                 candidate_generation_evidence_bundle_readback_id,
                 candidate_generation_evidence_bundle_id,
                 canonical_json
             ) VALUES (?1, ?2, x'ff')",
            params![readback, bundle_id],
        )
        .expect("insert readback");
    connection
        .execute(
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
                receipt, plan_id, attempt_id, precursor, managed, cleanup, bundle_id, readback
            ],
        )
        .expect("insert receipt");
    connection
        .execute(
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
                plan_id,
                attempt_id,
                precursor,
                receipt
            ],
        )
        .expect("insert completed attempt");
}

fn open_connection(database: &Path) -> Connection {
    let connection = Connection::open(database).expect("qualification database");
    connection
        .busy_timeout(Duration::from_secs(5))
        .expect("busy timeout");
    connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("disable foreign keys");
    connection
}

fn digest(label: &str) -> String {
    Digest::sha256(label.as_bytes()).as_str().to_owned()
}
