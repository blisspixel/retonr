use std::io::Cursor;

use rewrite_model::ArtifactSetRelativePath;

use crate::{MemberOpenError, RuntimeSourceBuildInputOpenError};

use super::*;

#[test]
fn compiler_derives_the_exact_admitted_golden_review() {
    let fixture = fixture();
    let input = compilation_input(all_passed());
    let compiled = compile(&fixture, &input).expect("compile review");
    assert_eq!(compiled.canonical_bytes(), fixture.review);
    assert_eq!(
        compiled.verified(),
        &verify(&fixture).expect("independently verify golden review")
    );
    assert_eq!(
        compiled
            .verified()
            .reconstructed_runtime()
            .expect("admitted runtime")
            .runtime_package()
            .runtime_package_manifest_id(),
        fixture.runtime_package_manifest_id
    );
}

#[test]
fn compiler_derives_blockers_and_keeps_candidate_reconstruction_inert() {
    let mut fixture = fixture();
    let statuses = [
        RuntimePackageReviewCheckStatus::Passed,
        RuntimePackageReviewCheckStatus::Passed,
        RuntimePackageReviewCheckStatus::NotRun,
        RuntimePackageReviewCheckStatus::NotRun,
        RuntimePackageReviewCheckStatus::NotRun,
        RuntimePackageReviewCheckStatus::NotRun,
    ];
    let input = compilation_input(statuses);
    let compiled = compile(&fixture, &input).expect("compile blocked review");
    assert!(compiled.verified().reconstructed_runtime().is_none());
    assert!(matches!(
        compiled.verified().review().disposition(),
        RuntimePackageReviewDispositionV2::NotAdmitted { blockers }
            if blockers == &[
                RuntimePackageReviewCheck::License,
                RuntimePackageReviewCheck::NativeClosure,
                RuntimePackageReviewCheck::ManagedStartup,
                RuntimePackageReviewCheck::CloudDisable,
            ]
    ));
    fixture.review = compiled.canonical_bytes().to_vec();
    assert_eq!(
        compiled.verified(),
        &verify(&fixture).expect("independently verify blocked review")
    );
}

#[test]
fn compiler_rejects_invalid_shape_and_cancellation_before_opening() {
    let mut reordered_evidence = evidence_inputs();
    reordered_evidence.swap(0, 1);
    let reordered = RuntimePackageReviewV2CompilationInput::new(
        path(SOURCE_PATH),
        path(LAYOUT_PATH),
        reordered_evidence,
        check_inputs(all_passed()),
    );
    assert_eq!(
        compile_runtime_package_review_v2(
            &reordered,
            &RuntimePackageReviewV2Limits::default(),
            |_path| -> Result<Cursor<Vec<u8>>, RuntimePackageReviewEvidenceOpenError> {
                panic!("invalid shape must fail before evidence opens")
            },
            |_path| -> Result<Cursor<Vec<u8>>, RuntimeSourceBuildInputOpenError> {
                panic!("invalid shape must fail before source opens")
            },
            |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> {
                panic!("invalid shape must fail before member opens")
            },
            || false,
        ),
        Err(RuntimePackageReviewV2Error::InvalidEvidence)
    );
    assert_eq!(
        compile_runtime_package_review_v2(
            &compilation_input(all_passed()),
            &RuntimePackageReviewV2Limits::default(),
            |_path| -> Result<Cursor<Vec<u8>>, RuntimePackageReviewEvidenceOpenError> {
                panic!("cancellation must fail before evidence opens")
            },
            |_path| -> Result<Cursor<Vec<u8>>, RuntimeSourceBuildInputOpenError> {
                panic!("cancellation must fail before source opens")
            },
            |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> {
                panic!("cancellation must fail before member opens")
            },
            || true,
        ),
        Err(RuntimePackageReviewV2Error::Cancelled)
    );
}

fn compile(
    fixture: &ReviewFixture,
    input: &RuntimePackageReviewV2CompilationInput,
) -> Result<CompiledRuntimePackageReviewV2, RuntimePackageReviewV2Error> {
    compile_runtime_package_review_v2(
        input,
        &RuntimePackageReviewV2Limits::default(),
        |path| {
            fixture
                .evidence
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(RuntimePackageReviewEvidenceOpenError)
        },
        |path| {
            fixture
                .source_inputs
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(RuntimeSourceBuildInputOpenError)
        },
        |path| {
            fixture
                .members
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(MemberOpenError)
        },
        || false,
    )
}

fn compilation_input(
    statuses: [RuntimePackageReviewCheckStatus; 6],
) -> RuntimePackageReviewV2CompilationInput {
    RuntimePackageReviewV2CompilationInput::new(
        path(SOURCE_PATH),
        path(LAYOUT_PATH),
        evidence_inputs(),
        check_inputs(statuses),
    )
}

fn evidence_inputs() -> Vec<RuntimePackageReviewV2EvidenceInput> {
    let source = path(SOURCE_PATH);
    let layout = path(LAYOUT_PATH);
    let build_tools = path("evidence/build-tools.json");
    let execution = path("evidence/execution.json");
    vec![
        RuntimePackageReviewV2EvidenceInput::new(
            RuntimePackageReviewEvidenceClass::BuildTool,
            build_tools.clone(),
        ),
        RuntimePackageReviewV2EvidenceInput::new(
            RuntimePackageReviewEvidenceClass::Execution,
            execution.clone(),
        ),
        RuntimePackageReviewV2EvidenceInput::new(
            RuntimePackageReviewEvidenceClass::FetchedInput,
            source.clone(),
        ),
        RuntimePackageReviewV2EvidenceInput::new(
            RuntimePackageReviewEvidenceClass::BuildOutput,
            layout.clone(),
        ),
    ]
}

fn check_inputs(
    statuses: [RuntimePackageReviewCheckStatus; 6],
) -> Vec<RuntimePackageReviewV2CheckInput> {
    let source = path(SOURCE_PATH);
    let layout = path(LAYOUT_PATH);
    let build_tools = path("evidence/build-tools.json");
    let execution = path("evidence/execution.json");
    vec![
        check(
            RuntimePackageReviewCheck::SourceLineage,
            statuses[0],
            vec![build_tools.clone(), source.clone()],
        ),
        check(
            RuntimePackageReviewCheck::Transformation,
            statuses[1],
            vec![build_tools, layout.clone()],
        ),
        check(
            RuntimePackageReviewCheck::License,
            statuses[2],
            vec![source],
        ),
        check(
            RuntimePackageReviewCheck::NativeClosure,
            statuses[3],
            vec![execution.clone(), layout.clone()],
        ),
        check(
            RuntimePackageReviewCheck::ManagedStartup,
            statuses[4],
            vec![execution.clone()],
        ),
        check(
            RuntimePackageReviewCheck::CloudDisable,
            statuses[5],
            vec![execution],
        ),
    ]
}

fn check(
    check: RuntimePackageReviewCheck,
    status: RuntimePackageReviewCheckStatus,
    evidence: Vec<ArtifactSetRelativePath>,
) -> RuntimePackageReviewV2CheckInput {
    RuntimePackageReviewV2CheckInput::new(check, status, evidence)
}

const fn all_passed() -> [RuntimePackageReviewCheckStatus; 6] {
    [RuntimePackageReviewCheckStatus::Passed; 6]
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value.to_owned()).expect("fixture path")
}
