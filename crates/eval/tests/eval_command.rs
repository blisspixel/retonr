use std::{fs, path::Path, path::PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use rewrite_eval::{
    EVALUATION_SCHEMA_VERSION, EvaluationCase, EvaluationSuite, ExpectedOutput,
    HYBRID_SCORECARD_SCHEMA_VERSION, HybridScorecardCasePlan, HybridScorecardPlan, JudgeAuthority,
    JudgeChoice, JudgeExecution, JudgeObservation, JudgeObservationBatch, JudgeOrderPolicy,
    JudgePresentation, LocalJudgePolicy, ReferenceJudgment,
    hybrid_scorecard_deterministic_policy_digest, hybrid_scorecard_plan_digest,
    hybrid_scorecard_suite_pair_digest,
};
use rewrite_inference::{ReasoningPolicy, SamplingParameters, candidate_output_contract};
use rewrite_model::ArtifactId;
use rewrite_types::Digest;
use serde_json::json;
use tempfile::tempdir;

fn hybrid_suite(candidate: &str) -> EvaluationSuite {
    EvaluationSuite {
        schema_version: EVALUATION_SCHEMA_VERSION,
        cases: vec![EvaluationCase {
            id: "case-a".to_owned(),
            category: "positive_literal".to_owned(),
            source: "Hello world".to_owned(),
            candidate: candidate.to_owned(),
            protected_terms: Vec::new(),
            reference_judgment: ReferenceJudgment::Acceptable,
            expected_status: rewrite_types::RewriteStatus::Rewritten,
            expected_reason: None,
            expected_output: ExpectedOutput::Candidate,
        }],
    }
}

fn hybrid_plan(
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
) -> HybridScorecardPlan {
    HybridScorecardPlan {
        schema_version: HYBRID_SCORECARD_SCHEMA_VERSION,
        plan_id: "cli-scorecard-v1".to_owned(),
        corpus_digest: hybrid_scorecard_suite_pair_digest(candidate_a, candidate_b)
            .expect("valid scorecard suites"),
        rubric_digest: Digest::sha256(b"rubric"),
        deterministic_policy_digest: hybrid_scorecard_deterministic_policy_digest(),
        judge: LocalJudgePolicy {
            execution: JudgeExecution::LocalIsolated,
            authority: JudgeAuthority::TriageOnly,
            order_policy: JudgeOrderPolicy::BothOrders,
            judge_system_digest: Digest::sha256(b"judge"),
            judge_model_reference: "judge-fixture:latest".to_owned(),
            judge_model_digest: Digest::sha256(b"judge-model"),
            judge_prompt_contract_digest: rewrite_eval::local_judge_prompt_contract_digest(),
            judge_output_schema_digest: rewrite_inference::local_judge_attempt_output_contract()
                .schema_digest,
            presentation_seed: 17,
            temperature_milli: 0,
            top_p_milli: 1_000,
            attempts_per_order: 1,
            max_judge_cases: 32,
            max_source_bytes: 4_096,
            max_candidate_bytes: 4_096,
            max_input_bytes: 16_384,
            context_token_limit: 8_192,
            output_token_limit: 512,
            max_response_bytes: 4096,
            maximum_elapsed_millis: 30_000,
        },
        cases: vec![HybridScorecardCasePlan {
            id: "case-a".to_owned(),
            cluster_id: "cluster-a".to_owned(),
            source_digest: Digest::sha256(candidate_a.cases[0].source.as_bytes()),
            candidate_a_digest: Digest::sha256(candidate_a.cases[0].candidate.as_bytes()),
            candidate_b_digest: Digest::sha256(candidate_b.cases[0].candidate.as_bytes()),
            candidate_a_system_digest: Digest::sha256(b"generator-a"),
            candidate_b_system_digest: Digest::sha256(b"generator-b"),
            rubric_clauses: vec!["meaning".to_owned()],
        }],
    }
}

fn hybrid_observations(plan: &HybridScorecardPlan) -> JudgeObservationBatch {
    JudgeObservationBatch {
        schema_version: HYBRID_SCORECARD_SCHEMA_VERSION,
        plan_id: plan.plan_id.clone(),
        plan_digest: hybrid_scorecard_plan_digest(plan).expect("valid scorecard plan"),
        observations: vec![
            JudgeObservation {
                case_id: "case-a".to_owned(),
                presentation: JudgePresentation::CandidateAFirst,
                choice: JudgeChoice::First,
                rubric_clauses: vec!["meaning".to_owned()],
            },
            JudgeObservation {
                case_id: "case-a".to_owned(),
                presentation: JudgePresentation::CandidateBFirst,
                choice: JudgeChoice::Second,
                rubric_clauses: vec!["meaning".to_owned()],
            },
        ],
    }
}

#[test]
fn hybrid_scorecard_runs_as_a_redacted_process() {
    let directory = tempdir().expect("temporary directory");
    let candidate_a = hybrid_suite("Hello, world!");
    let candidate_b = hybrid_suite("Hello world.");
    let plan = hybrid_plan(&candidate_a, &candidate_b);
    let observations = hybrid_observations(&plan);
    let plan_path = directory.path().join("plan.json");
    let candidate_a_path = directory.path().join("candidate-a.json");
    let candidate_b_path = directory.path().join("candidate-b.json");
    let observations_path = directory.path().join("observations.json");
    fs::write(
        &plan_path,
        serde_json::to_vec(&plan).expect("serialize plan"),
    )
    .expect("write plan");
    fs::write(
        &candidate_a_path,
        serde_json::to_vec(&candidate_a).expect("serialize candidate A"),
    )
    .expect("write candidate A");
    fs::write(
        &candidate_b_path,
        serde_json::to_vec(&candidate_b).expect("serialize candidate B"),
    )
    .expect("write candidate B");
    fs::write(
        &observations_path,
        serde_json::to_vec(&observations).expect("serialize observations"),
    )
    .expect("write observations");

    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .arg("--hybrid-scorecard")
        .arg(plan_path)
        .arg("--candidate-a-suite")
        .arg(candidate_a_path)
        .arg("--candidate-b-suite")
        .arg(candidate_b_path)
        .arg("--judge-observations")
        .arg(observations_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"deterministic_total\": 2"))
        .stdout(predicate::str::contains("\"deterministic_passed\": 2"))
        .stdout(predicate::str::contains("\"stable_a\": 1"))
        .stdout(predicate::str::contains(
            "\"judge_observation_evidence_class\": \"caller_declared\"",
        ))
        .stdout(predicate::str::contains("Hello world").not());
}

#[test]
fn checked_in_no_rewrite_baseline_runs_offline() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args([
            "--baseline",
            root.join("fixtures/no_rewrite_baseline_v1.json")
                .to_str()
                .expect("utf-8 fixture path"),
        ])
        .arg(root.join("fixtures/core.json"))
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"baseline_id\": \"no-rewrite-v1\"",
        ))
        .stdout(predicate::str::contains("\"kind\": \"no_rewrite\""))
        .stdout(predicate::str::contains("\"unchanged\": 49"))
        .stdout(predicate::str::contains("\"failed\": 0"))
        .stdout(predicate::str::contains("Hello world").not());
}

#[test]
fn incomplete_generative_baseline_fails_closed() {
    let directory = tempdir().expect("temporary directory");
    let definition = directory.path().join("direct.json");
    fs::write(
        &definition,
        r#"{
            "schema_version": 1,
            "id": "direct-prompt-v1",
            "kind": "direct_prompt"
        }"#,
    )
    .expect("write baseline definition");
    let suite = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/core.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .arg("--baseline")
        .arg(&definition)
        .arg(suite)
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid baseline configuration"))
        .stderr(predicate::str::contains("Hello world").not());
}

#[test]
fn checked_in_suite_passes_as_a_process() {
    let suite = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/core.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .arg(suite)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\": 49"))
        .stdout(predicate::str::contains("\"acceptable\": 9"))
        .stdout(predicate::str::contains("\"rewritten\": 4"))
        .stdout(predicate::str::contains("\"failures\": []"));
}

#[test]
fn checked_in_editorial_corpus_validates_as_a_process() {
    let corpus =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/editorial_quality_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--editorial-corpus"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 20"))
        .stdout(predicate::str::contains("\"finding_cases\": 10"))
        .stdout(predicate::str::contains("\"clean_controls\": 10"))
        .stdout(predicate::str::contains("Certainly").not());
}

#[test]
fn checked_in_slop_corpus_validates_as_a_process() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/editorial_slop_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--editorial-corpus"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 24"))
        .stdout(predicate::str::contains("\"finding_cases\": 12"))
        .stdout(predicate::str::contains("\"clean_controls\": 12"))
        .stdout(predicate::str::contains("rapidly evolving").not());
}

#[test]
fn checked_in_prose_corpus_validates_as_a_process() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/editorial_prose_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--editorial-corpus"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 40"))
        .stdout(predicate::str::contains("\"finding_cases\": 20"))
        .stdout(predicate::str::contains("\"clean_controls\": 20"))
        .stdout(predicate::str::contains("\"targeted_rules\": 20"))
        .stdout(predicate::str::contains("Everyone knows").not());
}

#[test]
fn checked_in_model_impression_corpus_validates_as_a_process() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/editorial_model_impressions_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--editorial-corpus"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 16"))
        .stdout(predicate::str::contains("\"finding_cases\": 8"))
        .stdout(predicate::str::contains("\"clean_controls\": 8"))
        .stdout(predicate::str::contains("Great question").not());
}

#[test]
fn checked_in_assistant_residue_corpus_validates_as_a_process() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/editorial_assistant_residue_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--editorial-corpus"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 20"))
        .stdout(predicate::str::contains("\"finding_cases\": 10"))
        .stdout(predicate::str::contains("\"clean_controls\": 10"))
        .stdout(predicate::str::contains("knowledge update").not());
}

#[test]
fn writing_sample_libraries_validate_without_printing_excerpts() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/writing_samples");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--writing-samples"])
        .arg(root.join("licensed_pre_ai_human_v1.json"))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"human_controls\": 8"))
        .stdout(predicate::str::contains("datagrams").not());
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--writing-samples"])
        .arg(root.join("synthetic_model_impressions_v1.json"))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"synthetic_impressions\": 7"))
        .stdout(predicate::str::contains("Certainly").not());
}

#[test]
fn checked_in_claim_shadow_calibration_passes_as_a_process() {
    let corpus =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/claim_shadow_calibration_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--claim-shadow-calibration"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"passed\": 12"))
        .stdout(predicate::str::contains("\"authority_violations\": 0"))
        .stdout(predicate::str::contains("\"failures\": []"))
        .stdout(predicate::str::contains("Hello world").not())
        .stdout(predicate::str::contains("available").not());
}

#[test]
fn claim_shadow_calibration_mismatch_fails_without_fixture_text() {
    let directory = tempdir().expect("temporary directory");
    let corpus = directory.path().join("calibration.json");
    fs::write(
        &corpus,
        r#"{
            "schema_version": 1,
            "corpus_id": "process-mismatch",
            "cases": [{
                "id": "expected-mismatch",
                "source": "private source",
                "candidate": "private source.",
                "expected_status": "abstained",
                "expected_reason": "semantic_uncertain",
                "expected_shadow": "absent"
            }]
        }"#,
    )
    .expect("write calibration fixture");

    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--claim-shadow-calibration"])
        .arg(corpus)
        .assert()
        .failure()
        .stdout(predicate::str::contains("expected-mismatch"))
        .stdout(predicate::str::contains("\"authority_violations\": 0"))
        .stdout(predicate::str::contains("private source").not());
}

#[test]
fn watermark_research_corpus_validates_without_mark_labels() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/watermark_research/style_is_not_a_watermark_v1.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--watermark-research"])
        .arg(corpus)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"refused_style_as_mark\": 4"))
        .stdout(predicate::str::contains("\"unmarked_controls\": 6"))
        .stdout(predicate::str::contains("delves").not());
}

#[test]
fn mismatch_fails_without_printing_fixture_content() {
    let directory = tempdir().expect("temporary directory");
    let suite = directory.path().join("suite.json");
    fs::write(
        &suite,
        r#"{
            "schema_version": 2,
            "cases": [{
                "id": "expected-mismatch",
                "category": "fixture",
                "source": "private source",
                "candidate": "private source.",
                "reference_judgment": "acceptable",
                "expected_status": "abstained",
                "expected_reason": null,
                "expected_output": "source"
            }]
        }"#,
    )
    .expect("write suite fixture");

    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .arg(suite)
        .assert()
        .failure()
        .stdout(predicate::str::contains("expected-mismatch"))
        .stdout(predicate::str::contains("private source").not());
}

fn direct_prompt_definition(artifact_id: &ArtifactId, digest: &Digest) -> String {
    let output = candidate_output_contract();
    let prompt_template = "Rewrite conservatively.";
    json!({
        "schema_version": 1,
        "id": "direct-prompt-v1",
        "kind": "direct_prompt",
        "inference": {
            "artifact_id": artifact_id,
            "artifact_digest": digest,
            "prompt_template": prompt_template,
            "prompt_template_digest": Digest::sha256(prompt_template.as_bytes()),
            "output": output,
            "source_byte_limit": 4096,
            "input_byte_limit": 8192,
            "context_token_limit": 8192,
            "output_token_limit": 512,
            "candidate_byte_limit": 4096,
            "sampling": SamplingParameters {
                temperature: 0.0,
                top_p: 1.0,
                seed: Some(1),
            },
            "reasoning": ReasoningPolicy::Disabled,
        }
    })
    .to_string()
}

#[test]
fn complete_generative_baseline_fails_closed_without_a_recovered_binding() {
    let directory = tempdir().expect("temporary directory");
    let digest = Digest::sha256(b"eval-conformance-baseline");
    let artifact_id = ArtifactId::from_digest(digest.clone());
    let definition = directory.path().join("direct.json");
    fs::write(&definition, direct_prompt_definition(&artifact_id, &digest))
        .expect("write baseline definition");
    let suite = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/core.json");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .arg("--baseline")
        .arg(&definition)
        .arg(suite)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "generative baseline requires an inference backend",
        ))
        .stderr(predicate::str::contains("Hello world").not());
}

fn activate_fake_generation(data: &Path) -> (ArtifactId, Digest) {
    use rewrite_app::{ArtifactImportLimits, ArtifactRepository, OfflineArtifactImportRequest};
    use rewrite_model::{
        ARTIFACT_MANIFEST_SCHEMA_VERSION, ActivationId, ArtifactManifest, ArtifactRole,
        ArtifactSource, DeclaredCapabilities, HardwareTier, InstalledArtifact, LicenseDecision,
        LicenseRecord, QUALIFICATION_SCHEMA_VERSION, QualificationRecord, QualificationStatus,
        RuntimeIdentity,
    };
    use rewrite_model_store::ArtifactStateStore;
    use rewrite_types::CancellationToken;

    const ARTIFACT_BYTES: &[u8] = b"eval-conformance-baseline";
    let source = data.parent().expect("temp parent").join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write artifact");
    let digest = Digest::sha256(ARTIFACT_BYTES);
    let artifact_id = ArtifactId::from_digest(digest.clone());
    ArtifactRepository::new(data)
        .expect("derive repository")
        .import(
            &OfflineArtifactImportRequest {
                source,
                manifest: ArtifactManifest {
                    schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
                    artifact_id: artifact_id.clone(),
                    source: ArtifactSource {
                        origin: "fixture/model".to_owned(),
                        revision: "fixture".to_owned(),
                    },
                    artifact_digest: digest.clone(),
                    byte_size: u64::try_from(ARTIFACT_BYTES.len()).expect("fixture size"),
                    format: "gguf".to_owned(),
                    family: "fixture".to_owned(),
                    architecture: None,
                    quantization: None,
                    tokenizer: None,
                    licenses: vec![LicenseRecord {
                        component: "weights".to_owned(),
                        identifier: "Apache-2.0".to_owned(),
                        text_digest: Digest::sha256(b"license"),
                    }],
                    declared_capabilities: DeclaredCapabilities {
                        roles: vec![ArtifactRole::Generation],
                        languages: vec!["en".to_owned()],
                        context_tokens: Some(8_192),
                    },
                },
            },
            ArtifactImportLimits {
                maximum_artifact_bytes: 1024,
                maximum_storage_entries: 8,
            },
            &CancellationToken::new(),
        )
        .expect("import");
    let installed = InstalledArtifact {
        artifact_id: artifact_id.clone(),
        artifact_digest: digest.clone(),
        byte_size: u64::try_from(ARTIFACT_BYTES.len()).expect("fixture size"),
        storage_key: format!("artifacts/{}", digest.as_str()),
    };
    let qualification = QualificationRecord {
        schema_version: QUALIFICATION_SCHEMA_VERSION,
        artifact_id: artifact_id.clone(),
        artifact_digest: digest.clone(),
        runtime: RuntimeIdentity {
            backend: "fake".to_owned(),
            version: "1.0.0".to_owned(),
            digest: Some(Digest::sha256(b"runtime")),
        },
        operating_system: "test".to_owned(),
        hardware_tier: HardwareTier {
            id: "test".to_owned(),
            memory_mib: 8_192,
            accelerator: "none".to_owned(),
        },
        supported_roles: vec![ArtifactRole::Generation],
        source_byte_limit: 4_096,
        context_token_limit: 8_192,
        prompt_template_digest: Digest::sha256(b"prompt"),
        request_policy_digest: Digest::sha256(b"request"),
        threshold_policy_digest: Digest::sha256(b"threshold"),
        license_decision: LicenseDecision::LocalUseOnly,
        status: QualificationStatus::Qualified,
    };
    let qualification_id = qualification
        .qualification_id()
        .expect("fixture qualification");
    let mut store =
        ArtifactStateStore::open_existing_writable_exact(&data.join("artifact-state.sqlite3"))
            .expect("open writable store");
    store
        .put_qualification(&qualification)
        .expect("store qualification");
    store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"eval-conformance")),
            ArtifactRole::Generation,
            &installed,
            &qualification_id,
        )
        .expect("activate generation");
    (artifact_id, digest)
}

#[test]
fn generative_baseline_runs_through_recovered_fake_conformance() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("data");
    let (artifact_id, digest) = activate_fake_generation(&data);
    let definition = directory.path().join("direct.json");
    fs::write(&definition, direct_prompt_definition(&artifact_id, &digest))
        .expect("write baseline definition");
    let suite = directory.path().join("suite.json");
    fs::write(
        &suite,
        r#"{
            "schema_version": 2,
            "cases": [{
                "id": "case-1",
                "category": "positive_literal",
                "source": "Hello world",
                "candidate": "Hello, world!",
                "reference_judgment": "acceptable",
                "expected_status": "rewritten",
                "expected_reason": null,
                "expected_output": "candidate"
            }]
        }"#,
    )
    .expect("write suite");
    Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .args(["--data-dir"])
        .arg(&data)
        .arg("--baseline")
        .arg(&definition)
        .arg(suite)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"kind\": \"direct_prompt\""))
        .stdout(predicate::str::contains("\"backend\": \"fake\""))
        .stdout(predicate::str::contains("\"unchanged\": 1"))
        .stdout(predicate::str::contains("\"failed\": 0"))
        .stdout(predicate::str::contains("Hello world").not());
}
