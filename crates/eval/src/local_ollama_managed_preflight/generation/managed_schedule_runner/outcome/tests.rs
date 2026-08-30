use std::cell::Cell;

use super::{
    ManagedJudgeScheduleAuthorityFailures, ManagedJudgeScheduleExecutionAuthorityError,
    VerifiedManagedJudgeScheduleExecution, combine_bracket, finish_failed_initial_validation,
    run_revalidation_kernel,
};

fn failures() -> ManagedJudgeScheduleAuthorityFailures {
    ManagedJudgeScheduleAuthorityFailures {
        eval: None,
        app: None,
        preflight: None,
        model: None,
        runtime: None,
        cancelled: true,
    }
}

#[test]
fn callback_and_final_validation_matrix_preserves_both_failures() {
    assert!(matches!(combine_bracket::<u8, &str>(Ok(7), Ok(())), Ok(7)));
    assert!(matches!(
        combine_bracket::<u8, &str>(Err("callback"), Ok(())),
        Err(ManagedJudgeScheduleExecutionAuthorityError::Callback(
            "callback"
        ))
    ));
    assert!(matches!(
        combine_bracket::<u8, &str>(Ok(7), Err(failures())),
        Err(ManagedJudgeScheduleExecutionAuthorityError::Final(_))
    ));
    assert!(matches!(
        combine_bracket::<u8, &str>(Err("callback"), Err(failures())),
        Err(
            ManagedJudgeScheduleExecutionAuthorityError::CallbackAndFinal {
                callback: "callback",
                ..
            }
        )
    ));
}

#[test]
fn failed_initial_validation_always_invokes_terminal_validation() {
    let calls = Cell::new(0_u8);
    for final_validation_fails in [false, true] {
        let result = finish_failed_initial_validation::<u8, &str>(failures(), || {
            calls.set(calls.get() + 1);
            if final_validation_fails {
                Err(failures())
            } else {
                Ok(())
            }
        });
        if final_validation_fails {
            assert!(matches!(
                result,
                Err(ManagedJudgeScheduleExecutionAuthorityError::InitialAndFinal { .. })
            ));
        } else {
            assert!(matches!(
                result,
                Err(ManagedJudgeScheduleExecutionAuthorityError::Initial(_))
            ));
        }
    }
    assert_eq!(calls.get(), 2);
}

#[test]
fn authority_failure_accessors_and_debug_are_redacted() {
    let failures = failures();
    assert!(failures.eval().is_none());
    assert!(failures.app().is_none());
    assert!(failures.preflight().is_none());
    assert!(failures.model().is_none());
    assert!(failures.runtime().is_none());
    assert!(failures.cancelled());
    assert_eq!(
        failures.to_string(),
        "managed judge schedule authority revalidation failed"
    );
    let debug = format!("{failures:?}");
    assert!(debug.contains("cancelled: true"));
    assert!(!debug.contains("example.invalid"));
}

#[test]
fn final_validation_uses_fresh_independent_tokens_after_callback_cancellation() {
    let operation = rewrite_types::CancellationToken::new();
    operation.cancel();
    let calls = Cell::new(0_u8);
    let validate = |token: &rewrite_types::CancellationToken| {
        assert!(!token.is_cancelled());
        token.cancel();
        calls.set(calls.get() + 1);
        Ok::<(), &'static str>(())
    };
    let result = run_revalidation_kernel(
        &operation, true, validate, validate, validate, validate, validate,
    );

    assert_eq!(calls.get(), 5);
    assert!(result.operation_cancelled);
    assert!(result.eval.is_ok());
    assert!(result.app.is_ok());
    assert!(result.preflight.is_ok());
    assert!(result.model.is_ok());
    assert!(result.runtime.is_ok());
}

#[test]
fn initial_validation_uses_the_shared_operation_token_and_records_cancellation() {
    let operation = rewrite_types::CancellationToken::new();
    let calls = Cell::new(0_u8);
    let first = |token: &rewrite_types::CancellationToken| {
        assert!(!token.is_cancelled());
        token.cancel();
        calls.set(calls.get() + 1);
        Ok::<(), &'static str>(())
    };
    let later = |token: &rewrite_types::CancellationToken| {
        assert!(token.is_cancelled());
        calls.set(calls.get() + 1);
        Ok::<(), &'static str>(())
    };
    let result = run_revalidation_kernel(&operation, false, first, later, later, later, later);

    assert_eq!(calls.get(), 5);
    assert!(result.operation_cancelled);
}

#[test]
fn revalidation_kernel_exercises_both_token_modes_in_one_instantiation() {
    #[expect(
        clippy::unnecessary_wraps,
        reason = "the shared function pointer must retain the validator Result signature"
    )]
    fn validate(_token: &rewrite_types::CancellationToken) -> Result<(), &'static str> {
        Ok(())
    }

    let operation = rewrite_types::CancellationToken::new();
    let validator: fn(&rewrite_types::CancellationToken) -> Result<(), &'static str> = validate;
    for fresh_tokens in [false, true] {
        let result = run_revalidation_kernel(
            &operation,
            fresh_tokens,
            validator,
            validator,
            validator,
            validator,
            validator,
        );
        assert!(result.eval.is_ok());
        assert!(result.app.is_ok());
        assert!(result.preflight.is_ok());
        assert!(result.model.is_ok());
        assert!(result.runtime.is_ok());
        assert!(!result.operation_cancelled);
    }
}

#[test]
fn execution_authority_is_noncloneable_and_nonserializable() {
    trait AmbiguousIfClone<A> {
        fn marker() {}
    }
    impl<T: ?Sized> AmbiguousIfClone<()> for T {}
    impl<T: Clone> AmbiguousIfClone<u8> for T {}

    trait AmbiguousIfSerialize<A> {
        fn marker() {}
    }
    impl<T: ?Sized> AmbiguousIfSerialize<()> for T {}
    impl<T: ?Sized + serde::Serialize> AmbiguousIfSerialize<u8> for T {}

    type Authority = VerifiedManagedJudgeScheduleExecution<'static, 'static, 'static, 'static>;
    let _ = <Authority as AmbiguousIfClone<_>>::marker;
    let _ = <Authority as AmbiguousIfSerialize<_>>::marker;
}
