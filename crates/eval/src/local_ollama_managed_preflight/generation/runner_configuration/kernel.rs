use rewrite_types::CancellationToken;

use super::{
    CandidateJudgePreparationError, ManagedJudgePrecursorCompilationError,
    ManagedJudgeRunnerConfigurationError, ManagedJudgeRunnerConfigurationValidationPhase,
};

pub(super) trait ConfigurationSubject: Sized {
    type Output;

    fn revalidate_eval(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateJudgePreparationError>;

    fn revalidate_app(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgePrecursorCompilationError>;

    fn validate_caller_configuration(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeRunnerConfigurationError>;

    fn release(
        self,
        cancellation: &CancellationToken,
    ) -> Result<Self::Output, ManagedJudgeRunnerConfigurationError>;
}

pub(super) fn validate_configuration<S: ConfigurationSubject>(
    mut subject: S,
    cancellation: &CancellationToken,
) -> Result<S::Output, ManagedJudgeRunnerConfigurationError> {
    validate_authorities(
        &mut subject,
        ManagedJudgeRunnerConfigurationValidationPhase::Initial,
        cancellation,
    )?;

    let primary = subject.validate_caller_configuration(cancellation);
    let final_validation = validate_authorities(
        &mut subject,
        ManagedJudgeRunnerConfigurationValidationPhase::Final,
        cancellation,
    );
    match (primary, final_validation) {
        (Ok(()), Ok(())) => subject.release(cancellation),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), Err(final_validation)) => Err(final_validation),
        (Err(primary), Err(final_validation)) => Err(
            ManagedJudgeRunnerConfigurationError::PrimaryAndFinalValidation {
                primary: Box::new(primary),
                final_validation: Box::new(final_validation),
            },
        ),
    }
}

fn validate_authorities<S: ConfigurationSubject>(
    subject: &mut S,
    phase: ManagedJudgeRunnerConfigurationValidationPhase,
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeRunnerConfigurationError> {
    if cancellation.is_cancelled() {
        return if matches!(
            phase,
            ManagedJudgeRunnerConfigurationValidationPhase::Initial
        ) {
            Err(ManagedJudgeRunnerConfigurationError::Cancelled)
        } else {
            Err(ManagedJudgeRunnerConfigurationError::AuthorityValidation {
                phase,
                eval: None,
                app: None,
                cancelled: true,
            })
        };
    }
    let eval = subject.revalidate_eval(cancellation).err().map(Box::new);
    if cancellation.is_cancelled() {
        return Err(ManagedJudgeRunnerConfigurationError::AuthorityValidation {
            phase,
            eval,
            app: None,
            cancelled: true,
        });
    }
    let app = subject.revalidate_app(cancellation).err().map(Box::new);
    let cancelled = cancellation.is_cancelled();
    if eval.is_some() || app.is_some() || cancelled {
        Err(ManagedJudgeRunnerConfigurationError::AuthorityValidation {
            phase,
            eval,
            app,
            cancelled,
        })
    } else {
        Ok(())
    }
}
