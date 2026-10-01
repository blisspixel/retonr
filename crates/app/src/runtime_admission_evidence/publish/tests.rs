use std::fs;

use super::super::assembly::tests::{FakeSource, compilation};
use super::*;
use crate::{RuntimeAdmissionEvidenceBundleSource, RuntimeAdmissionEvidenceBundleVerifier};

fn destination(root: &std::path::Path) -> RuntimeAdmissionEvidenceBundleDestination {
    RuntimeAdmissionEvidenceBundleDestination::new(root.join("published"))
        .expect("portable destination")
}

#[test]
fn first_root_publish_is_exact_and_cold_reacquisition_matches() {
    let parent = tempfile::tempdir().expect("fixture destination parent");
    let source = FakeSource::new(parent.path().join("source"));
    let assembly = compilation(&source);
    let destination = destination(parent.path());
    let lease = publish_view(
        &assembly,
        &source,
        &destination,
        RuntimeAdmissionEvidenceBundleLimits::default(),
        &source.cancellation,
    )
    .expect("publish inert closure");
    assert_eq!(lease.manifest(), assembly.manifest());
    assert_eq!(lease.foundation(), assembly.foundation());
    assert_eq!(lease.tree_plan(), assembly.tree_plan());
    assert_eq!(source.checks.get(), 7);
    for (path, expected) in &assembly.bytes {
        assert_eq!(
            fs::read(destination.path().join(path)).expect("published exact bytes"),
            *expected
        );
    }
    let cold = RuntimeAdmissionEvidenceBundleVerifier::acquire(
        &RuntimeAdmissionEvidenceBundleSource::new(destination.path()).expect("cold source"),
        RuntimeAdmissionEvidenceBundleLimits::default(),
        &CancellationToken::new(),
    )
    .expect("cold verifier");
    assert_eq!(cold.manifest(), lease.manifest());
    assert_eq!(
        fs::read_dir(parent.path())
            .expect("destination entries")
            .count(),
        1
    );
}

#[test]
fn cancellation_and_broadened_or_lowered_limits_precede_mutation() {
    let parent = tempfile::tempdir().expect("fixture destination parent");
    let source = FakeSource::new(parent.path().join("source"));
    let assembly = compilation(&source);
    for limits in [
        RuntimeAdmissionEvidenceBundleLimits {
            maximum_tree_entries: 0,
            ..RuntimeAdmissionEvidenceBundleLimits::default()
        },
        RuntimeAdmissionEvidenceBundleLimits {
            maximum_total_bytes: 1_048_576,
            ..RuntimeAdmissionEvidenceBundleLimits::default()
        },
    ] {
        assert!(
            publish_view(
                &assembly,
                &source,
                &destination(parent.path()),
                limits,
                &source.cancellation
            )
            .is_err()
        );
        assert_eq!(
            fs::read_dir(parent.path())
                .expect("destination entries")
                .count(),
            0
        );
    }
    source.cancellation.cancel();
    assert!(matches!(
        publish_view(
            &assembly,
            &source,
            &destination(parent.path()),
            RuntimeAdmissionEvidenceBundleLimits::default(),
            &source.cancellation
        ),
        Err(RuntimeAdmissionEvidenceAssemblyError::Bundle(
            RuntimeAdmissionEvidenceBundleError::Cancelled
        ))
    ));
    assert_eq!(
        fs::read_dir(parent.path())
            .expect("destination entries")
            .count(),
        0
    );
}

#[test]
fn existing_destination_and_overlap_fail_without_replacement() {
    let parent = tempfile::tempdir().expect("fixture destination parent");
    let source = FakeSource::new(parent.path().join("source"));
    let assembly = compilation(&source);
    let destination = destination(parent.path());
    fs::create_dir(destination.path()).expect("existing destination");
    fs::write(destination.path().join("keep"), b"existing").expect("existing bytes");
    assert!(
        publish_view(
            &assembly,
            &source,
            &destination,
            RuntimeAdmissionEvidenceBundleLimits::default(),
            &source.cancellation
        )
        .is_err()
    );
    assert_eq!(
        fs::read(destination.path().join("keep")).expect("retained bytes"),
        b"existing"
    );
    let source = FakeSource::new(destination.path().to_owned());
    assert!(matches!(
        publish_view(
            &assembly,
            &source,
            &destination,
            RuntimeAdmissionEvidenceBundleLimits::default(),
            &source.cancellation
        ),
        Err(RuntimeAdmissionEvidenceAssemblyError::Bundle(
            RuntimeAdmissionEvidenceBundleError::UnsafeBoundary
        ))
    ));
}

#[test]
fn source_failure_at_every_precommit_boundary_leaves_no_root_or_staging() {
    for check in 1..=4 {
        let parent = tempfile::tempdir().expect("fixture destination parent");
        let source = FakeSource::new(parent.path().join("source"));
        let assembly = compilation(&source);
        source.fail_at.set(Some(check));
        assert!(matches!(
            publish_view(
                &assembly,
                &source,
                &destination(parent.path()),
                RuntimeAdmissionEvidenceBundleLimits::default(),
                &source.cancellation
            ),
            Err(RuntimeAdmissionEvidenceAssemblyError::Foundation(_))
        ));
        assert_eq!(
            fs::read_dir(parent.path())
                .expect("clean destination parent")
                .count(),
            0,
            "check {check}"
        );
    }
}

#[test]
fn cancellation_during_precommit_work_cleans_with_fresh_context() {
    for check in 1..=4 {
        let parent = tempfile::tempdir().expect("fixture destination parent");
        let source = FakeSource::new(parent.path().join("source"));
        let assembly = compilation(&source);
        source.cancel_at.set(Some(check));
        assert!(
            publish_view(
                &assembly,
                &source,
                &destination(parent.path()),
                RuntimeAdmissionEvidenceBundleLimits::default(),
                &source.cancellation
            )
            .is_err()
        );
        assert_eq!(
            fs::read_dir(parent.path())
                .expect("clean destination parent")
                .count(),
            0,
            "check {check}"
        );
    }
}

#[test]
fn destination_race_fails_no_replace_and_preserves_competing_root() {
    let parent = tempfile::tempdir().expect("fixture destination parent");
    let mut source = FakeSource::new(parent.path().join("source"));
    let assembly = compilation(&source);
    let destination = destination(parent.path());
    source.inject_at.set(Some(4));
    source.injection = Some(destination.path().to_owned());
    assert!(
        publish_view(
            &assembly,
            &source,
            &destination,
            RuntimeAdmissionEvidenceBundleLimits::default(),
            &source.cancellation
        )
        .is_err()
    );
    assert_eq!(
        fs::read(destination.path().join("keep")).expect("competitor preserved"),
        b"independent existing bytes"
    );
    assert_eq!(
        fs::read_dir(parent.path()).expect("parent entries").count(),
        1
    );
}

#[test]
fn postcommit_source_failure_reports_inert_committed_root() {
    for check in [5, 6, 7] {
        let parent = tempfile::tempdir().expect("fixture destination parent");
        let source = FakeSource::new(parent.path().join("source"));
        let assembly = compilation(&source);
        source.fail_at.set(Some(check));
        let destination = destination(parent.path());
        assert!(matches!(
            publish_view(
                &assembly,
                &source,
                &destination,
                RuntimeAdmissionEvidenceBundleLimits::default(),
                &source.cancellation
            ),
            Err(RuntimeAdmissionEvidenceAssemblyError::CommittedFinalization { .. })
        ));
        assert!(
            RuntimeAdmissionEvidenceBundleVerifier::acquire(
                &RuntimeAdmissionEvidenceBundleSource::new(destination.path())
                    .expect("inert committed source"),
                RuntimeAdmissionEvidenceBundleLimits::default(),
                &CancellationToken::new()
            )
            .is_ok()
        );
    }
}

#[test]
fn independent_primary_and_fresh_source_failures_are_both_retained() {
    let parent = tempfile::tempdir().expect("fixture destination parent");
    let source = FakeSource::new(parent.path().join("source"));
    let assembly = compilation(&source);
    source.fail_all.set(true);
    let error = publish_view(
        &assembly,
        &source,
        &destination(parent.path()),
        RuntimeAdmissionEvidenceBundleLimits::default(),
        &source.cancellation,
    )
    .expect_err("source operation and finalization fail");
    match error {
        RuntimeAdmissionEvidenceAssemblyError::FinalizationAfterFailure {
            operation,
            finalization,
        } => {
            assert!(matches!(
                *operation,
                RuntimeAdmissionEvidenceAssemblyError::Foundation(_)
            ));
            assert!(matches!(
                *finalization,
                RuntimeAdmissionEvidenceAssemblyError::Foundation(_)
            ));
        }
        other => panic!("unexpected failure class: {other:?}"),
    }
    assert_eq!(source.checks.get(), 2);
    assert_eq!(
        fs::read_dir(parent.path())
            .expect("untouched parent")
            .count(),
        0
    );
}

#[test]
fn caller_cancellation_after_commit_is_reported_after_finalization() {
    let parent = tempfile::tempdir().expect("fixture destination parent");
    let source = FakeSource::new(parent.path().join("source"));
    let assembly = compilation(&source);
    source.cancel_at.set(Some(5));
    assert!(matches!(
        publish_view(
            &assembly,
            &source,
            &destination(parent.path()),
            RuntimeAdmissionEvidenceBundleLimits::default(),
            &source.cancellation
        ),
        Err(RuntimeAdmissionEvidenceAssemblyError::CommittedFinalization { .. })
    ));
    assert!(source.cancellation.is_cancelled());
    assert_eq!(source.checks.get(), 7);
}

#[test]
fn corrupted_private_assembly_fails_staged_verification_and_cleanup() {
    let parent = tempfile::tempdir().expect("fixture destination parent");
    let source = FakeSource::new(parent.path().join("source"));
    let mut assembly = compilation(&source);
    assembly.bytes.insert(
        "controls/license-v2.json",
        b"changed private snapshot".to_vec(),
    );
    assert!(
        publish_view(
            &assembly,
            &source,
            &destination(parent.path()),
            RuntimeAdmissionEvidenceBundleLimits::default(),
            &source.cancellation
        )
        .is_err()
    );
    assert_eq!(
        fs::read_dir(parent.path()).expect("clean parent").count(),
        0
    );
}

#[test]
fn destination_parent_bound_is_checked_by_empty_publication_probe() {
    let parent = tempfile::tempdir().expect("fixture destination parent");
    let source = FakeSource::new(parent.path().join("source"));
    let assembly = compilation(&source);
    fs::write(parent.path().join("existing"), b"existing").expect("existing parent member");
    let limits = RuntimeAdmissionEvidenceBundleLimits {
        maximum_destination_entries: 1,
        ..RuntimeAdmissionEvidenceBundleLimits::default()
    };
    assert!(
        publish_view(
            &assembly,
            &source,
            &destination(parent.path()),
            limits,
            &source.cancellation
        )
        .is_err()
    );
    assert_eq!(
        fs::read_dir(parent.path())
            .expect("retained parent")
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn indirect_parent_is_rejected_without_writes() {
    let parent = tempfile::tempdir().expect("fixture destination parent");
    let source = FakeSource::new(parent.path().join("source"));
    let assembly = compilation(&source);
    std::os::unix::fs::symlink(parent.path(), parent.path().join("alias"))
        .expect("indirect parent");
    let destination = destination(&parent.path().join("alias"));
    assert!(matches!(
        publish_view(
            &assembly,
            &source,
            &destination,
            RuntimeAdmissionEvidenceBundleLimits::default(),
            &source.cancellation
        ),
        Err(RuntimeAdmissionEvidenceAssemblyError::Bundle(
            RuntimeAdmissionEvidenceBundleError::UnsafeBoundary
        ))
    ));
}
