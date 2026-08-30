use rewrite_types::CancellationToken;

use super::{
    ManagedJudgeRunnerConfigurationError, ManagedJudgeRunnerConfigurationRelationship,
    ensure_not_cancelled,
};

pub(super) trait RelationshipValidationSubject {
    fn validate_relationship(
        &mut self,
        relationship: ManagedJudgeRunnerConfigurationRelationship,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeRunnerConfigurationError>;
}

pub(super) fn validate_relationships(
    subject: &mut impl RelationshipValidationSubject,
    relationships: &[ManagedJudgeRunnerConfigurationRelationship],
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeRunnerConfigurationError> {
    ensure_not_cancelled(cancellation)?;
    for relationship in relationships {
        subject.validate_relationship(*relationship, cancellation)?;
        ensure_not_cancelled(cancellation)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELATIONSHIPS: [ManagedJudgeRunnerConfigurationRelationship; 8] = [
        ManagedJudgeRunnerConfigurationRelationship::PreflightProfile,
        ManagedJudgeRunnerConfigurationRelationship::PreflightLimits,
        ManagedJudgeRunnerConfigurationRelationship::WorkerLimits,
        ManagedJudgeRunnerConfigurationRelationship::Runtime,
        ManagedJudgeRunnerConfigurationRelationship::Isolation,
        ManagedJudgeRunnerConfigurationRelationship::Model,
        ManagedJudgeRunnerConfigurationRelationship::PortableJudge,
        ManagedJudgeRunnerConfigurationRelationship::InstallationGeneration,
    ];

    #[derive(Default)]
    struct FakeSubject {
        observed: Vec<ManagedJudgeRunnerConfigurationRelationship>,
        fail_on: Option<ManagedJudgeRunnerConfigurationRelationship>,
        cancel_on: Option<ManagedJudgeRunnerConfigurationRelationship>,
    }

    impl RelationshipValidationSubject for FakeSubject {
        fn validate_relationship(
            &mut self,
            relationship: ManagedJudgeRunnerConfigurationRelationship,
            cancellation: &CancellationToken,
        ) -> Result<(), ManagedJudgeRunnerConfigurationError> {
            self.observed.push(relationship);
            if self.cancel_on == Some(relationship) {
                cancellation.cancel();
            }
            if self.fail_on == Some(relationship) {
                Err(ManagedJudgeRunnerConfigurationError::Relationship(
                    relationship,
                ))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn every_relationship_runs_once_in_frozen_order() {
        let mut subject = FakeSubject::default();
        validate_relationships(&mut subject, &RELATIONSHIPS, &CancellationToken::new())
            .expect("all relationships");
        assert_eq!(subject.observed, RELATIONSHIPS);
    }

    #[test]
    fn every_relationship_failure_is_primary_and_stops_later_checks() {
        for (index, relationship) in RELATIONSHIPS.into_iter().enumerate() {
            let mut subject = FakeSubject {
                fail_on: Some(relationship),
                ..FakeSubject::default()
            };
            let error =
                validate_relationships(&mut subject, &RELATIONSHIPS, &CancellationToken::new())
                    .expect_err("relationship must fail");
            assert!(matches!(
                error,
                ManagedJudgeRunnerConfigurationError::Relationship(observed)
                    if observed == relationship
            ));
            assert_eq!(subject.observed, RELATIONSHIPS[..=index]);
        }
    }

    #[test]
    fn cancellation_is_checked_before_and_after_every_relationship() {
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let mut untouched = FakeSubject::default();
        assert!(matches!(
            validate_relationships(&mut untouched, &RELATIONSHIPS, &cancelled),
            Err(ManagedJudgeRunnerConfigurationError::Cancelled)
        ));
        assert!(untouched.observed.is_empty());

        for (index, relationship) in RELATIONSHIPS.into_iter().enumerate() {
            let cancellation = CancellationToken::new();
            let mut subject = FakeSubject {
                cancel_on: Some(relationship),
                ..FakeSubject::default()
            };
            assert!(matches!(
                validate_relationships(&mut subject, &RELATIONSHIPS, &cancellation),
                Err(ManagedJudgeRunnerConfigurationError::Cancelled)
            ));
            assert_eq!(subject.observed, RELATIONSHIPS[..=index]);
        }
    }

    #[test]
    fn empty_relationship_set_is_valid_only_while_active() {
        let mut subject = FakeSubject::default();
        validate_relationships(&mut subject, &[], &CancellationToken::new())
            .expect("empty active set");
        assert!(subject.observed.is_empty());

        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            validate_relationships(&mut subject, &[], &cancellation),
            Err(ManagedJudgeRunnerConfigurationError::Cancelled)
        ));
    }
}
