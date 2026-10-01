use std::{os::unix::process::ExitStatusExt as _, process::ExitStatus};

use super::*;
use crate::{MAXIMUM_STARTUP_STREAM_BYTES, ManagedStartupOutput};

mod subprocess;

fn result(status: ControlledBuildProcessStatus, bytes: Vec<u8>) -> ControlledBuildOutput {
    ControlledBuildOutput::new(
        status,
        ManagedStartupOutput::new(bytes.clone(), bytes, false, false),
    )
}

#[test]
fn bootstrap_routes_only_four_fixed_builds_with_the_canonical_environment() {
    let recipe_root = tempfile::tempdir().expect("recipe root");
    let directory = recipe_root.path().join("lineage");
    std::fs::create_dir(&directory).expect("lineage");
    std::fs::write(
        directory.join("build-recipe-v2.json"),
        recipe::EXPECTED_RECIPE,
    )
    .expect("recipe");
    let environment = recipe::read_environment(&File::open(recipe_root.path()).expect("root"))
        .expect("environment");
    let mut calls = Vec::new();
    let completed = execute_builds(&environment, |build, supplied| {
        assert_eq!(supplied, environment);
        assert!(
            build
                .arguments()
                .iter()
                .all(|argument| argument != "--build-runtime")
        );
        calls.push(build);
        Ok(result(
            ControlledBuildProcessStatus::Success,
            b"compiled\n".to_vec(),
        ))
    })
    .expect("fixed builds");
    assert_eq!(calls, recipe::BUILDS);
    assert_eq!(completed.status(), ControlledBuildProcessStatus::Success);
    assert_eq!(
        completed.streams().standard_error(),
        b"compiled\ncompiled\ncompiled\ncompiled\n"
    );
}

#[test]
fn failed_or_signalled_command_stops_before_later_builds_and_retains_bounded_diagnostics() {
    for failure in [
        ControlledBuildProcessStatus::ExitCode(70),
        ControlledBuildProcessStatus::Signal(9),
    ] {
        let mut calls = 0;
        let completed = execute_builds(&[], |_, _| {
            calls += 1;
            Ok(result(
                if calls == 2 {
                    failure
                } else {
                    ControlledBuildProcessStatus::Success
                },
                vec![b'x'; MAXIMUM_STARTUP_STREAM_BYTES],
            ))
        })
        .expect("terminal output");
        assert_eq!(calls, 2);
        assert_eq!(completed.status(), failure);
        assert_eq!(
            completed.streams().standard_error().len(),
            MAXIMUM_STARTUP_STREAM_BYTES
        );
        assert!(completed.streams().standard_error_truncated());
        assert!(completed.streams().standard_output_truncated());
    }
    let mut calls = 0;
    assert_eq!(
        execute_builds(&[], |_, _| {
            calls += 1;
            Err(HelperFailure::InvalidLaunch)
        }),
        Err(HelperFailure::InvalidLaunch)
    );
    assert_eq!(calls, 1);
}

#[test]
fn exit_status_preserves_success_exit_code_and_signal() {
    assert_eq!(
        process_status(ExitStatus::from_raw(0)),
        Ok(ControlledBuildProcessStatus::Success)
    );
    assert_eq!(
        process_status(ExitStatus::from_raw(70 << 8)),
        Ok(ControlledBuildProcessStatus::ExitCode(70))
    );
    assert_eq!(
        process_status(ExitStatus::from_raw(9)),
        Ok(ControlledBuildProcessStatus::Signal(9))
    );
}
