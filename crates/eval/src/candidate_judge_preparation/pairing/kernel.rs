use rewrite_app::ManagedJudgePrecursorCompilationError;
use rewrite_types::CancellationToken;

use super::{
    CandidateJudgePreparationError, CandidateJudgeRunnerPairingError,
    CandidateJudgeRunnerPairingRelationship, CandidateJudgeRunnerPairingValidationPhase,
};

const RELATIONSHIPS: [CandidateJudgeRunnerPairingRelationship; 4] = [
    CandidateJudgeRunnerPairingRelationship::JudgePlan,
    CandidateJudgeRunnerPairingRelationship::JudgeSchedule,
    CandidateJudgeRunnerPairingRelationship::RequestAggregate,
    CandidateJudgeRunnerPairingRelationship::JudgeSystem,
];

pub(super) trait PairingSubject {
    fn revalidate_eval(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateJudgePreparationError>;

    fn revalidate_app(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgePrecursorCompilationError>;

    fn relationship_matches(
        &mut self,
        relationship: CandidateJudgeRunnerPairingRelationship,
        cancellation: &CancellationToken,
    ) -> bool;
}

pub(super) fn validate_pairing(
    subject: &mut impl PairingSubject,
    cancellation: &CancellationToken,
) -> Result<(), CandidateJudgeRunnerPairingError> {
    ensure_active(cancellation)?;
    validate_authorities(
        subject,
        CandidateJudgeRunnerPairingValidationPhase::Initial,
        cancellation,
    )?;

    let primary = compare_relationships(subject, cancellation);
    let final_validation = validate_authorities(
        subject,
        CandidateJudgeRunnerPairingValidationPhase::Final,
        cancellation,
    );
    resolve_final_validation(primary, final_validation)
}

fn compare_relationships(
    subject: &mut impl PairingSubject,
    cancellation: &CancellationToken,
) -> Result<(), CandidateJudgeRunnerPairingError> {
    for relationship in RELATIONSHIPS {
        ensure_active(cancellation)?;
        let matches = subject.relationship_matches(relationship, cancellation);
        ensure_active(cancellation)?;
        if !matches {
            return Err(CandidateJudgeRunnerPairingError::Relationship(relationship));
        }
    }
    Ok(())
}

fn validate_authorities(
    subject: &mut impl PairingSubject,
    phase: CandidateJudgeRunnerPairingValidationPhase,
    cancellation: &CancellationToken,
) -> Result<(), CandidateJudgeRunnerPairingError> {
    // Eval-before-app is the frozen precedence. An eval failure does not prevent
    // the independent app readback unless cancellation has become terminal.
    if cancellation.is_cancelled() {
        return authority_validation_result(phase, None, None, true);
    }
    let eval = subject.revalidate_eval(cancellation).err().map(Box::new);
    if cancellation.is_cancelled() {
        return authority_validation_result(phase, eval, None, true);
    }
    let app = subject.revalidate_app(cancellation).err().map(Box::new);
    authority_validation_result(phase, eval, app, cancellation.is_cancelled())
}

fn authority_validation_result(
    phase: CandidateJudgeRunnerPairingValidationPhase,
    eval: Option<Box<CandidateJudgePreparationError>>,
    app: Option<Box<ManagedJudgePrecursorCompilationError>>,
    cancelled: bool,
) -> Result<(), CandidateJudgeRunnerPairingError> {
    if eval.is_none() && app.is_none() && !cancelled {
        Ok(())
    } else {
        Err(CandidateJudgeRunnerPairingError::AuthorityValidation {
            phase,
            eval,
            app,
            cancelled,
        })
    }
}

fn resolve_final_validation(
    primary: Result<(), CandidateJudgeRunnerPairingError>,
    final_validation: Result<(), CandidateJudgeRunnerPairingError>,
) -> Result<(), CandidateJudgeRunnerPairingError> {
    match (primary, final_validation) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), Err(final_validation)) => Err(final_validation),
        (Err(primary), Err(final_validation)) => Err(
            CandidateJudgeRunnerPairingError::PrimaryAndFinalValidation {
                primary: Box::new(primary),
                final_validation: Box::new(final_validation),
            },
        ),
    }
}

fn ensure_active(cancellation: &CancellationToken) -> Result<(), CandidateJudgeRunnerPairingError> {
    if cancellation.is_cancelled() {
        Err(CandidateJudgeRunnerPairingError::Cancelled)
    } else {
        Ok(())
    }
}
