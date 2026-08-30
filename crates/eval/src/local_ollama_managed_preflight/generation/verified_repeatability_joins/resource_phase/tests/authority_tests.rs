use rewrite_app::GenerationQualificationPhasePolicySourceDisposition;

use super::super::authority::{ResourceAuthorityBindingView, validate_resource_authority_bindings};
use super::super::{
    GenerationQualificationResourcePhaseAuthorityError as AuthorityError,
    GenerationQualificationResourcePhaseCompilationError as CompilationError,
    GenerationQualificationResourcePhaseDerivationError as DerivationError, finish_bracket,
    with_snapshot_bracket,
};

#[test]
fn bracket_preserves_every_possible_initial_callback_and_final_combination() {
    assert!(matches!(
        finish_bracket::<(), _>(Err(AuthorityError::PolicyDenied), None, Ok(1)),
        Err(CompilationError::InitialAuthority(
            AuthorityError::PolicyDenied
        ))
    ));
    assert!(matches!(
        finish_bracket::<(), i32>(
            Err(AuthorityError::PolicyDenied),
            None,
            Err(AuthorityError::Cancelled),
        ),
        Err(CompilationError::InitialAndFinalAuthority {
            initial: AuthorityError::PolicyDenied,
            final_validation: AuthorityError::Cancelled,
        })
    ));
    assert!(matches!(
        finish_bracket::<(), _>(Ok(1), Some(Err(DerivationError::Relationship)), Ok(1),),
        Err(CompilationError::Compilation(DerivationError::Relationship))
    ));
    assert!(matches!(
        finish_bracket::<(), _>(
            Ok(1),
            Some(Err(DerivationError::InvalidCount)),
            Err(AuthorityError::SnapshotChanged),
        ),
        Err(CompilationError::CompilationAndFinalAuthority {
            compilation: DerivationError::InvalidCount,
            final_validation: AuthorityError::SnapshotChanged,
        })
    ));
    assert!(matches!(
        finish_bracket(Ok(1), Some(Ok(())), Err(AuthorityError::Cancelled)),
        Err(CompilationError::FinalAuthority(AuthorityError::Cancelled))
    ));
    assert!(finish_bracket(Ok(1), Some(Ok(())), Ok(1)).is_ok());
}

#[test]
fn snapshot_bracket_runs_positive_callback_and_rejects_final_drift() {
    let mut stable_calls = 0_u8;
    let value = with_snapshot_bracket(
        &mut stable_calls,
        |calls| {
            *calls += 1;
            Ok(vec![1_u8, 2])
        },
        |calls, snapshot| Ok((usize::from(*calls), snapshot.len())),
    )
    .expect("stable snapshot bracket");
    assert_eq!(value, (1, 2));
    assert_eq!(stable_calls, 2);

    let mut drift_calls = 0_u8;
    let error = with_snapshot_bracket(
        &mut drift_calls,
        |calls| {
            *calls += 1;
            Ok(vec![*calls])
        },
        |_calls, snapshot| Ok(snapshot.len()),
    )
    .expect_err("changed final snapshot");
    assert!(matches!(
        error,
        CompilationError::FinalAuthority(AuthorityError::SnapshotChanged)
    ));
    assert_eq!(drift_calls, 2);
}

#[test]
fn pure_authority_binding_kernel_preserves_closed_precedence_and_success() {
    let view = |source_disposition, policy_binding_matches, operation_scope_matches| {
        ResourceAuthorityBindingView {
            source_disposition,
            policy_binding_matches,
            operation_scope_matches,
        }
    };
    assert!(matches!(
        validate_resource_authority_bindings(view(
            GenerationQualificationPhasePolicySourceDisposition::Denied,
            false,
            false,
        )),
        Err(AuthorityError::PolicyDenied)
    ));
    assert!(matches!(
        validate_resource_authority_bindings(view(
            GenerationQualificationPhasePolicySourceDisposition::Approved,
            false,
            false,
        )),
        Err(AuthorityError::PolicyBinding)
    ));
    assert!(matches!(
        validate_resource_authority_bindings(view(
            GenerationQualificationPhasePolicySourceDisposition::Approved,
            true,
            false,
        )),
        Err(AuthorityError::OperationScope)
    ));
    assert!(
        validate_resource_authority_bindings(view(
            GenerationQualificationPhasePolicySourceDisposition::Approved,
            true,
            true,
        ))
        .is_ok()
    );
}
