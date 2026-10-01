use super::*;

#[test]
fn cleanup_outcome_never_masks_the_primary_failure() {
    let primary = RuntimeAdmissionRunnerError::Cancelled;
    let cleanup = super::super::IsolationError::UnsupportedPlatform;
    let error = finish_with_fresh_cleanup::<(), _>(Err(primary), |_fresh| Err(cleanup))
        .expect_err("both failures must be retained");
    assert!(matches!(
        error,
        RuntimeAdmissionRunnerError::CleanupAfterFailure {
            operation,
            cleanup: super::super::IsolationError::UnsupportedPlatform,
        } if matches!(*operation, RuntimeAdmissionRunnerError::Cancelled)
    ));
}

#[test]
fn cleanup_failure_blocks_an_apparent_success() {
    let error = finish_with_fresh_cleanup(Ok(()), |_fresh| {
        Err(super::super::IsolationError::UnsupportedPlatform)
    })
    .expect_err("cleanup is part of success");
    assert!(matches!(
        error,
        RuntimeAdmissionRunnerError::Cleanup(super::super::IsolationError::UnsupportedPlatform)
    ));
}

#[test]
fn cleanup_receives_a_fresh_token_after_operation_cancellation() {
    let caller = CancellationToken::new();
    caller.cancel();
    let mut cleanup_called = false;

    let error =
        finish_with_fresh_cleanup::<(), _>(Err(RuntimeAdmissionRunnerError::Cancelled), |fresh| {
            cleanup_called = true;
            assert!(!fresh.is_cancelled());
            Ok(())
        })
        .expect_err("primary cancellation is retained");

    assert!(cleanup_called);
    assert!(caller.is_cancelled());
    assert!(matches!(error, RuntimeAdmissionRunnerError::Cancelled));
}

#[test]
fn operation_cleanup_and_package_failure_matrix_runs_both_finalizers() {
    use std::cell::Cell;
    for operation_failed in [false, true] {
        for cleanup_failed in [false, true] {
            for package_failed in [false, true] {
                let finalizers = Cell::new(0);
                let operation = if operation_failed {
                    Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding)
                } else {
                    Ok(7)
                };
                let result = finish_with_mandatory_finalization(
                    operation,
                    |fresh| {
                        assert!(!fresh.is_cancelled());
                        assert_eq!(finalizers.get(), 0);
                        finalizers.set(1);
                        if cleanup_failed {
                            Err(super::super::IsolationError::UnsupportedPlatform)
                        } else {
                            Ok(())
                        }
                    },
                    |fresh| {
                        assert!(!fresh.is_cancelled());
                        assert_eq!(finalizers.get(), 1);
                        finalizers.set(2);
                        if package_failed {
                            Err(RuntimeAdmissionRunnerError::InvalidPackageBinding)
                        } else {
                            Ok(())
                        }
                    },
                    &CancellationToken::new(),
                );
                assert_eq!(finalizers.get(), 2);
                if package_failed && (operation_failed || cleanup_failed) {
                    let error = result.expect_err("primary and finalization errors retained");
                    let RuntimeAdmissionRunnerError::FinalizationAfterFailure {
                        operation,
                        finalization,
                    } = error
                    else {
                        panic!("expected combined finalization error")
                    };
                    assert!(matches!(
                        *finalization,
                        RuntimeAdmissionRunnerError::InvalidPackageBinding
                    ));
                    match (operation_failed, cleanup_failed) {
                        (true, true) => assert!(
                            matches!(*operation, RuntimeAdmissionRunnerError::CleanupAfterFailure { operation, cleanup: super::super::IsolationError::UnsupportedPlatform } if matches!(*operation, RuntimeAdmissionRunnerError::InvalidEvidenceBinding))
                        ),
                        (true, false) => assert!(matches!(
                            *operation,
                            RuntimeAdmissionRunnerError::InvalidEvidenceBinding
                        )),
                        (false, true) => assert!(matches!(
                            *operation,
                            RuntimeAdmissionRunnerError::Cleanup(
                                super::super::IsolationError::UnsupportedPlatform
                            )
                        )),
                        (false, false) => {
                            unreachable!("combined error requires a primary failure")
                        }
                    }
                } else {
                    assert_eq!(
                        result.is_ok(),
                        !operation_failed && !cleanup_failed && !package_failed
                    );
                }
            }
        }
    }
}

#[test]
fn late_cancellation_is_sampled_only_after_independent_finalizers() {
    use std::cell::Cell;
    for cancel_in_cleanup in [false, true] {
        let caller = CancellationToken::new();
        let finalizers = Cell::new(0);
        let result = finish_with_mandatory_finalization(
            Ok(7),
            |fresh| {
                assert!(!fresh.is_cancelled());
                finalizers.set(1);
                if cancel_in_cleanup {
                    caller.cancel();
                }
                Ok(())
            },
            |fresh| {
                assert!(!fresh.is_cancelled());
                assert_eq!(finalizers.get(), 1);
                finalizers.set(2);
                caller.cancel();
                Ok(())
            },
            &caller,
        );
        assert_eq!(finalizers.get(), 2);
        assert!(matches!(
            result,
            Err(RuntimeAdmissionRunnerError::Cancelled)
        ));
    }
}

#[test]
fn primary_cancellation_retains_failed_cleanup_and_package_revalidation() {
    let caller = CancellationToken::new();
    caller.cancel();
    let error = finish_with_mandatory_finalization::<()>(
        Err(RuntimeAdmissionRunnerError::Cancelled),
        |fresh| {
            assert!(!fresh.is_cancelled());
            Err(super::super::IsolationError::UnsupportedPlatform)
        },
        |fresh| {
            assert!(!fresh.is_cancelled());
            Err(RuntimeAdmissionRunnerError::InvalidPackageBinding)
        },
        &caller,
    )
    .expect_err("all three independent failures remain visible");
    assert!(
        matches!(error, RuntimeAdmissionRunnerError::FinalizationAfterFailure { operation, finalization } if matches!(operation.as_ref(), RuntimeAdmissionRunnerError::CleanupAfterFailure { operation, .. } if matches!(operation.as_ref(), RuntimeAdmissionRunnerError::Cancelled)) && matches!(*finalization, RuntimeAdmissionRunnerError::InvalidPackageBinding))
    );
}
