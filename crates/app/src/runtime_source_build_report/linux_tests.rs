use std::fs;

use rewrite_ollama_package::{
    RuntimeSourceBuildReportError, RuntimeSourceBuildReportLimits,
    verify_runtime_source_build_report,
};
use rewrite_runtime_isolation::IsolationError;
use rewrite_types::CancellationToken;
use serde_json::{Value, json};
use tempfile::tempdir_in;

use super::*;
use crate::{
    ExecutableRuntimeSourceBuildBundleLease, RuntimeSourceBuildBundleError,
    RuntimeSourceBuildBundleLimits, RuntimeSourceBuildBundleVerifier,
    RuntimeSourceBuildEvidenceBundleDestination, RuntimeSourceBuildEvidenceBundleError,
    RuntimeSourceBuildEvidenceBundleLimits, RuntimeSourceBuildEvidenceBundlePublisher,
    RuntimeSourceBuildEvidenceBundleSource, RuntimeSourceBuildEvidenceBundleVerifier,
    RuntimeSourceBuildExecutionError, RuntimeSourceBuildExecutionPair, RuntimeSourceBuildExecutor,
    RuntimeSourceBuildOutputSource,
    runtime_source_build_execution::inject_controlled_build_output_substitution_once,
};

mod fixture;
mod review;

use fixture::{BundleFixture, write_bundle_fixture};

#[test]
fn retained_pair_compiles_and_reverifies_one_exact_report() {
    if std::env::var_os("RETONR_CONTROLLED_BUILD_CAPABILITY_ABI").is_some() {
        return;
    }
    let Some((helper, build_program)) = native_fixture_programs() else {
        return;
    };
    let fixture = write_bundle_fixture(&helper, &build_program);
    let cancellation = CancellationToken::new();
    let inert_bundle = RuntimeSourceBuildBundleVerifier::acquire(
        &fixture.selection,
        RuntimeSourceBuildBundleLimits::default(),
        &cancellation,
    )
    .expect("acquire retained source-build bundle");
    let bundle = ExecutableRuntimeSourceBuildBundleLease::from_inert_bundle_for_test(inert_bundle);
    let primary_root = tempdir_in("/tmp").expect("primary output root under host temporary root");
    let rebuild_root = tempdir_in("/tmp").expect("rebuild output root under host temporary root");
    let primary_output =
        RuntimeSourceBuildOutputSource::new(primary_root.path()).expect("select primary output");
    let rebuild_output =
        RuntimeSourceBuildOutputSource::new(rebuild_root.path()).expect("select rebuild output");
    assert_rejected_output_boundaries(&fixture, &bundle, &cancellation);
    let Some(pair) = execute_fixture_pair(&bundle, &primary_output, &rebuild_output, &cancellation)
    else {
        return;
    };
    assert_report_compiles_and_reverifies(&bundle, &pair, &cancellation);
    assert_evidence_publishes_and_detects_tampering(
        &bundle,
        &pair,
        primary_root.path(),
        &cancellation,
    );
    assert_manifest_semantic_relabeling_is_rejected_before_source_io(
        &helper,
        &build_program,
        &bundle,
        &pair,
        &cancellation,
    );
    assert_completed_output_tampering_is_detected(
        &bundle,
        &pair,
        primary_root.path(),
        rebuild_root.path(),
        &cancellation,
    );
}

fn native_fixture_programs() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let Some(helper) = std::env::var_os("REWRITE_ISOLATION_TEST_HELPER") else {
        assert!(
            std::env::var_os("REWRITE_ISOLATION_REQUIRE_NATIVE").is_none(),
            "required native isolation helper was not supplied"
        );
        return None;
    };
    let Some(build_program) = std::env::var_os("REWRITE_SOURCE_BUILD_TEST_PROGRAM") else {
        assert!(
            std::env::var_os("REWRITE_ISOLATION_REQUIRE_NATIVE").is_none(),
            "required native controlled-build fixture was not supplied"
        );
        return None;
    };
    Some((helper.into(), build_program.into()))
}

fn assert_rejected_output_boundaries(
    fixture: &BundleFixture,
    bundle: &ExecutableRuntimeSourceBuildBundleLease,
    cancellation: &CancellationToken,
) {
    let overlap_root = tempdir_in("/tmp").expect("overlap output under host temporary root");
    let overlap_child = overlap_root.path().join("child");
    fs::create_dir(&overlap_child).expect("overlap child output");
    let overlap_parent =
        RuntimeSourceBuildOutputSource::new(overlap_root.path()).expect("select overlap parent");
    let overlap_child =
        RuntimeSourceBuildOutputSource::new(overlap_child).expect("select overlap child");
    assert!(matches!(
        RuntimeSourceBuildExecutor::execute_pair(
            bundle,
            &overlap_parent,
            &overlap_child,
            cancellation,
        ),
        Err(RuntimeSourceBuildExecutionError::OutputOverlap)
    ));
    let source_overlap =
        RuntimeSourceBuildOutputSource::new(fixture.selection.component_root().join("scripts"))
            .expect("select overlapping source output");
    assert!(matches!(
        RuntimeSourceBuildExecutor::execute(bundle, &source_overlap, cancellation,),
        Err(RuntimeSourceBuildExecutionError::UnsafeOutput)
    ));
    let substituted_root = tempdir_in("/tmp").expect("substituted handoff output");
    let substituted_output = RuntimeSourceBuildOutputSource::new(substituted_root.path())
        .expect("select substituted handoff output");
    inject_controlled_build_output_substitution_once();
    let Err(substitution_error) =
        RuntimeSourceBuildExecutor::execute(bundle, &substituted_output, cancellation)
    else {
        panic!("substituted output was accepted");
    };
    assert!(
        matches!(
            substitution_error,
            RuntimeSourceBuildExecutionError::OutputChanged
        ),
        "unexpected substituted-output failure: {substitution_error:?}"
    );
}

fn execute_fixture_pair(
    bundle: &ExecutableRuntimeSourceBuildBundleLease,
    primary_output: &RuntimeSourceBuildOutputSource,
    rebuild_output: &RuntimeSourceBuildOutputSource,
    cancellation: &CancellationToken,
) -> Option<RuntimeSourceBuildExecutionPair> {
    match RuntimeSourceBuildExecutor::execute_pair(
        bundle,
        primary_output,
        rebuild_output,
        cancellation,
    ) {
        Ok(pair) => Some(pair),
        Err(RuntimeSourceBuildExecutionError::Isolation(IsolationError::HostPolicyDenied))
            if std::env::var_os("REWRITE_ISOLATION_REQUIRE_NATIVE").is_none() =>
        {
            None
        }
        Err(error) => panic!("controlled source-build pair failed: {error}"),
    }
}

fn assert_report_compiles_and_reverifies(
    bundle: &ExecutableRuntimeSourceBuildBundleLease,
    pair: &RuntimeSourceBuildExecutionPair,
    cancellation: &CancellationToken,
) {
    let compilation = RuntimeSourceBuildReportCompiler::compile(
        bundle,
        pair.primary(),
        pair.rebuild(),
        RuntimeSourceBuildReportCompilationLimits::default(),
        cancellation,
    )
    .expect("compile controlled source-build report");
    assert!(compilation.compiled().is_byte_identical());
    assert!(!compilation.canonical_report_bytes().is_empty());
    assert!(
        !compilation
            .managed_evidence(RuntimeSourceBuildAttempt::Primary)
            .execution_receipt_bytes()
            .is_empty()
    );

    let inputs = CompilationInputs {
        primary: pair.primary(),
        rebuild: pair.rebuild(),
        primary_managed: compilation.managed_evidence(RuntimeSourceBuildAttempt::Primary),
        rebuild_managed: compilation.managed_evidence(RuntimeSourceBuildAttempt::Rebuild),
        cancellation,
    };
    let verified = verify_runtime_source_build_report(
        compilation.canonical_report_bytes(),
        bundle.bundle().inputs().manifest(),
        bundle.plan(),
        &RuntimeSourceBuildReportLimits::default(),
        |attempt, path| open_evidence(&inputs, attempt, path),
        |attempt, path| open_member(&inputs, attempt, path),
        || cancellation.is_cancelled(),
    )
    .expect("independently verify compiled report");
    assert!(verified.is_byte_identical());
}

fn assert_evidence_publishes_and_detects_tampering(
    bundle: &ExecutableRuntimeSourceBuildBundleLease,
    pair: &RuntimeSourceBuildExecutionPair,
    primary_root: &std::path::Path,
    cancellation: &CancellationToken,
) {
    let overlapping_evidence =
        RuntimeSourceBuildEvidenceBundleDestination::new(primary_root.join("durable-evidence"))
            .expect("select overlapping evidence path");
    assert!(matches!(
        RuntimeSourceBuildEvidenceBundlePublisher::publish(
            bundle,
            pair.primary(),
            pair.rebuild(),
            &overlapping_evidence,
            RuntimeSourceBuildEvidenceBundleLimits::default(),
            cancellation,
        ),
        Err(RuntimeSourceBuildEvidenceBundleError::UnsafeBoundary)
    ));
    assert!(!overlapping_evidence.path().exists());

    let evidence_parent = tempdir_in("/tmp").expect("durable evidence under host temporary root");
    let destination = RuntimeSourceBuildEvidenceBundleDestination::new(
        evidence_parent.path().join("controlled-build-evidence"),
    )
    .expect("select durable evidence destination");
    let published = RuntimeSourceBuildEvidenceBundlePublisher::publish(
        bundle,
        pair.primary(),
        pair.rebuild(),
        &destination,
        RuntimeSourceBuildEvidenceBundleLimits::default(),
        cancellation,
    )
    .expect("publish durable controlled-build evidence");
    assert_eq!(published.manifest().members().len(), 48);
    assert!(published.report().is_byte_identical());
    assert_eq!(published.plan(), bundle.plan());
    published
        .revalidate(cancellation)
        .expect("revalidate published evidence");
    let duplicate_publication = RuntimeSourceBuildEvidenceBundlePublisher::publish(
        bundle,
        pair.primary(),
        pair.rebuild(),
        &destination,
        RuntimeSourceBuildEvidenceBundleLimits::default(),
        cancellation,
    );
    let Err(duplicate_error) = duplicate_publication else {
        panic!("duplicate publication replaced an existing destination");
    };
    assert!(
        matches!(
            duplicate_error,
            RuntimeSourceBuildEvidenceBundleError::Changed
        ),
        "unexpected duplicate-publication result: {duplicate_error:?}"
    );
    let source = RuntimeSourceBuildEvidenceBundleSource::new(destination.path())
        .expect("select published evidence");
    let reacquired = RuntimeSourceBuildEvidenceBundleVerifier::acquire(
        &source,
        RuntimeSourceBuildEvidenceBundleLimits::default(),
        cancellation,
    )
    .expect("independently reacquire published evidence");
    assert_eq!(
        reacquired.manifest().artifact_set_id(),
        published.manifest().artifact_set_id()
    );
    assert_eq!(reacquired.report(), published.report());
    review::assert_blocked_build_stage_review(&reacquired, cancellation);
    pair.primary()
        .revalidate_output(cancellation)
        .expect("revalidate primary output");
    pair.rebuild()
        .revalidate_output(cancellation)
        .expect("revalidate rebuild output");
    bundle
        .revalidate(cancellation)
        .expect("revalidate frozen input bundle");
    drop(reacquired);
    drop(published);
    let durable_sbom = destination.path().join("attempts/primary/sbom.json");
    let mut changed_sbom = fs::read(&durable_sbom).expect("read durable SBOM");
    changed_sbom[0] ^= 1;
    fs::write(&durable_sbom, changed_sbom).expect("change durable SBOM bytes");
    assert!(matches!(
        RuntimeSourceBuildEvidenceBundleVerifier::acquire(
            &source,
            RuntimeSourceBuildEvidenceBundleLimits::default(),
            cancellation,
        ),
        Err(RuntimeSourceBuildEvidenceBundleError::Report(
            RuntimeSourceBuildReportError::EvidenceDigestMismatch
        ))
    ));
}

fn assert_completed_output_tampering_is_detected(
    bundle: &ExecutableRuntimeSourceBuildBundleLease,
    pair: &RuntimeSourceBuildExecutionPair,
    primary_root: &std::path::Path,
    rebuild_root: &std::path::Path,
    cancellation: &CancellationToken,
) {
    for root in [primary_root, rebuild_root] {
        let sbom = root.join("sbom.json");
        let bytes = fs::read(&sbom).expect("read completed output SBOM");
        fs::remove_file(&sbom).expect("replace completed output SBOM");
        fs::write(&sbom, bytes).expect("write substituted output SBOM");
    }
    assert!(matches!(
        RuntimeSourceBuildReportCompiler::compile(
            bundle,
            pair.primary(),
            pair.rebuild(),
            RuntimeSourceBuildReportCompilationLimits::default(),
            cancellation,
        ),
        Err(RuntimeSourceBuildReportCompilationError::Output(
            RuntimeSourceBuildExecutionError::OutputChanged
        ))
    ));
}

fn assert_manifest_semantic_relabeling_is_rejected_before_source_io(
    helper: &std::path::Path,
    build_program: &std::path::Path,
    executed_bundle: &ExecutableRuntimeSourceBuildBundleLease,
    pair: &RuntimeSourceBuildExecutionPair,
    cancellation: &CancellationToken,
) {
    let relabeled_fixture = write_bundle_fixture(helper, build_program);
    let manifest_path = relabeled_fixture.selection.manifest_path();
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(manifest_path).expect("read relabeling source manifest"))
            .expect("parse relabeling source manifest");
    manifest["components"][0]["name"] = json!("relabeled-isolation-helper");
    manifest["components"][0]["revision"] = json!("fixture-v2");
    manifest["components"][0]["source_locator"] = json!("urn:fixture:relabeled-isolation-helper");
    manifest["components"][1]["roles"] = json!(["source_patch", "license_evidence"]);
    fs::write(
        manifest_path,
        serde_json::to_vec(&manifest).expect("encode canonical relabeling manifest"),
    )
    .expect("write relabeling source manifest");
    let relabeled_inert_bundle = RuntimeSourceBuildBundleVerifier::acquire(
        &relabeled_fixture.selection,
        RuntimeSourceBuildBundleLimits::default(),
        cancellation,
    )
    .expect("acquire semantic relabeling bundle");
    let relabeled_bundle =
        ExecutableRuntimeSourceBuildBundleLease::from_inert_bundle_for_test(relabeled_inert_bundle);
    assert_eq!(
        relabeled_bundle.plan().source_inputs_id(),
        executed_bundle.plan().source_inputs_id(),
        "semantic relabeling must preserve the exact source artifact set"
    );
    assert_eq!(
        relabeled_bundle.plan().policy(),
        executed_bundle.plan().policy(),
        "semantic relabeling must preserve the execution policy"
    );
    assert_ne!(
        relabeled_bundle.plan().source_manifest_digest(),
        executed_bundle.plan().source_manifest_digest(),
        "semantic relabeling must derive a distinct manifest identity"
    );
    assert_ne!(
        relabeled_bundle.plan().plan_digest(),
        executed_bundle.plan().plan_digest(),
        "semantic relabeling must derive a distinct plan"
    );

    fs::write(
        relabeled_fixture
            .selection
            .component_root()
            .join("metadata/build-parameters.json"),
        b"drift after relabeling bundle acquisition",
    )
    .expect("drift relabeling source after acquisition");
    assert!(matches!(
        relabeled_bundle.bundle().revalidate(cancellation),
        Err(RuntimeSourceBuildBundleError::SourceChanged)
    ));
    assert!(matches!(
        RuntimeSourceBuildReportCompiler::compile(
            &relabeled_bundle,
            pair.primary(),
            pair.rebuild(),
            RuntimeSourceBuildReportCompilationLimits::default(),
            cancellation,
        ),
        Err(RuntimeSourceBuildReportCompilationError::Output(
            RuntimeSourceBuildExecutionError::PlanMismatch
        ))
    ));
}
