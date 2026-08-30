use rewrite_runtime_isolation::IsolationError;

use crate::PackageAttestationError;

use super::super::{
    ManagedOllamaLaunchError, ManagedOllamaLaunchFinalizationFailures,
    ManagedOllamaModelAuthorityError, ManagedOllamaPostAcquisitionFailure,
};
use std::time::Instant;

pub(in crate::package_attestation::ollama_input::launch) fn post_acquisition_failure(
    binding_failed: bool,
    cancelled: bool,
    operation_deadline: Option<Instant>,
    now: Instant,
) -> Option<ManagedOllamaPostAcquisitionFailure> {
    if operation_deadline.is_some_and(|deadline| now >= deadline) {
        Some(ManagedOllamaPostAcquisitionFailure::DeadlineExceeded)
    } else if cancelled {
        Some(ManagedOllamaPostAcquisitionFailure::Cancelled)
    } else if binding_failed {
        Some(ManagedOllamaPostAcquisitionFailure::InputBindingMismatch)
    } else {
        None
    }
}

pub(super) fn finalize_post_acquisition_with(
    primary: ManagedOllamaPostAcquisitionFailure,
    cleanup: impl FnOnce() -> Result<(), IsolationError>,
    model: impl FnOnce() -> Result<(), ManagedOllamaModelAuthorityError>,
    runtime: impl FnOnce() -> Result<(), PackageAttestationError>,
) -> ManagedOllamaLaunchError {
    let failures = run_finalizers(cleanup, model, runtime);
    ManagedOllamaLaunchError::PostAcquisitionFailure {
        primary,
        finalization: ManagedOllamaLaunchFinalizationFailures::new(
            failures.cleanup.map(Box::new),
            failures.model.map(Box::new),
            failures.runtime.map(Box::new),
        ),
    }
}

struct FinalizerFailures<C, M, R> {
    cleanup: Option<C>,
    model: Option<M>,
    runtime: Option<R>,
}

fn run_finalizers<C, M, R>(
    cleanup: impl FnOnce() -> Result<(), C>,
    model: impl FnOnce() -> Result<(), M>,
    runtime: impl FnOnce() -> Result<(), R>,
) -> FinalizerFailures<C, M, R> {
    FinalizerFailures {
        cleanup: cleanup().err(),
        model: model().err(),
        runtime: runtime().err(),
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc, time::Instant};

    use rewrite_runtime_isolation::IsolationError;

    use crate::{
        ManagedOllamaLaunchError, ManagedOllamaModelAuthorityError,
        ManagedOllamaPostAcquisitionFailure, ModelLicenseControlError, PackageAttestationError,
    };

    use super::{finalize_post_acquisition_with, post_acquisition_failure};

    #[test]
    fn simultaneous_post_acquisition_deadline_precedes_cancellation_and_binding_failure() {
        let now = Instant::now();
        assert_eq!(
            post_acquisition_failure(true, true, Some(now), now),
            Some(ManagedOllamaPostAcquisitionFailure::DeadlineExceeded)
        );
    }

    #[test]
    fn post_acquisition_cancellation_precedes_binding_failure_before_deadline() {
        let now = Instant::now();
        assert_eq!(
            post_acquisition_failure(true, true, None, now),
            Some(ManagedOllamaPostAcquisitionFailure::Cancelled)
        );
    }

    #[test]
    fn every_primary_preserves_all_eight_independent_finalizer_combinations() {
        for (primary, mask) in [
            ManagedOllamaPostAcquisitionFailure::InputBindingMismatch,
            ManagedOllamaPostAcquisitionFailure::Cancelled,
            ManagedOllamaPostAcquisitionFailure::DeadlineExceeded,
        ]
        .into_iter()
        .flat_map(|primary| (0_u8..8).map(move |mask| (primary, mask)))
        {
            let order = Rc::new(RefCell::new(Vec::new()));
            let cleanup_order = Rc::clone(&order);
            let model_order = Rc::clone(&order);
            let runtime_order = Rc::clone(&order);
            let error = finalize_post_acquisition_with(
                primary,
                || {
                    observed_result(
                        &cleanup_order,
                        "cleanup",
                        mask & 1 != 0,
                        IsolationError::Cancelled,
                    )
                },
                || {
                    observed_result(
                        &model_order,
                        "model",
                        mask & 2 != 0,
                        ManagedOllamaModelAuthorityError::Revalidation(
                            ModelLicenseControlError::Cancelled,
                        ),
                    )
                },
                || {
                    observed_result(
                        &runtime_order,
                        "runtime",
                        mask & 4 != 0,
                        PackageAttestationError::Cancelled,
                    )
                },
            );

            assert_eq!(&*order.borrow(), &["cleanup", "model", "runtime"]);
            let ManagedOllamaLaunchError::PostAcquisitionFailure {
                primary: observed_primary,
                finalization,
            } = error
            else {
                panic!("expected post-acquisition failure")
            };
            assert_eq!(observed_primary, primary);
            assert_eq!(finalization.cleanup().is_some(), mask & 1 != 0);
            assert_eq!(finalization.model().is_some(), mask & 2 != 0);
            assert_eq!(finalization.runtime().is_some(), mask & 4 != 0);
        }
    }

    #[test]
    fn post_acquisition_error_preserves_each_failure_and_redacts_debug() {
        let error = finalize_post_acquisition_with(
            ManagedOllamaPostAcquisitionFailure::InputBindingMismatch,
            || Err(IsolationError::InvalidPolicy("sensitive-cleanup")),
            || {
                Err(ManagedOllamaModelAuthorityError::Revalidation(
                    ModelLicenseControlError::Cancelled,
                ))
            },
            || Err(PackageAttestationError::Cancelled),
        );
        let ManagedOllamaLaunchError::PostAcquisitionFailure {
            primary,
            finalization,
        } = &error
        else {
            panic!("expected post-acquisition failure")
        };

        assert_eq!(
            *primary,
            ManagedOllamaPostAcquisitionFailure::InputBindingMismatch
        );
        assert!(matches!(
            finalization.cleanup(),
            Some(IsolationError::InvalidPolicy("sensitive-cleanup"))
        ));
        assert!(matches!(
            finalization.model(),
            Some(ManagedOllamaModelAuthorityError::Revalidation(
                ModelLicenseControlError::Cancelled
            ))
        ));
        assert!(matches!(
            finalization.runtime(),
            Some(PackageAttestationError::Cancelled)
        ));
        assert!(finalization.has_failures());
        let debug = format!("{error:?}");
        assert!(!debug.contains("sensitive-cleanup"));
        assert_eq!(
            debug,
            "ManagedOllamaLaunchError { kind: \"post_acquisition_failure\", primary: InputBindingMismatch, finalization: ManagedOllamaLaunchFinalizationFailures { cleanup_failed: true, model_failed: true, runtime_failed: true }, .. }"
        );
    }

    #[test]
    fn successful_finalization_still_preserves_redacted_cancellation_primary() {
        let error = finalize_post_acquisition_with(
            ManagedOllamaPostAcquisitionFailure::Cancelled,
            || Ok(()),
            || Ok(()),
            || Ok(()),
        );
        let ManagedOllamaLaunchError::PostAcquisitionFailure {
            primary,
            finalization,
        } = &error
        else {
            panic!("expected post-acquisition failure")
        };

        assert_eq!(*primary, ManagedOllamaPostAcquisitionFailure::Cancelled);
        assert!(!finalization.has_failures());
        assert!(finalization.cleanup().is_none());
        assert!(finalization.model().is_none());
        assert!(finalization.runtime().is_none());
        assert_eq!(
            format!("{error:?}"),
            "ManagedOllamaLaunchError { kind: \"post_acquisition_failure\", primary: Cancelled, finalization: ManagedOllamaLaunchFinalizationFailures { cleanup_failed: false, model_failed: false, runtime_failed: false }, .. }"
        );
    }

    fn observed_result<E>(
        order: &Rc<RefCell<Vec<&'static str>>>,
        label: &'static str,
        fail: bool,
        error: E,
    ) -> Result<(), E> {
        order.borrow_mut().push(label);
        if fail { Err(error) } else { Ok(()) }
    }
}
