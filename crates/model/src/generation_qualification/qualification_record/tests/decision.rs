use super::super::validation::{DecisionFacts, derive_status_from_facts};
use super::*;

fn managed_facts(phase_statuses: [GenerationQualificationPhaseStatusV1; 4]) -> DecisionFacts {
    DecisionFacts {
        terminal_status: GenerationQualificationOperationTerminalStatusV1::Completed,
        finalization_status: GenerationQualificationOperationFinalizationStatusV1::Passed,
        peak_concurrent_attempts: 1,
        under_deadline: true,
        platform_status: GenerationQualificationPlatformStatusV1::Supported,
        license_decision: GenerationQualificationLicenseDecisionV1::LocalUseOnly,
        phase_statuses,
        ledger_has_actual_failure: false,
        repeatability_has_actual_failure: false,
    }
}

#[test]
fn managed_decision_truth_table_distinguishes_policy_failure_from_abort_prefix() {
    use GenerationQualificationPhaseStatusV1::{Failed, Passed, Skipped};
    assert_eq!(
        derive_status_from_facts(managed_facts([Passed, Passed, Passed, Passed])),
        Ok(GenerationQualificationStatusV1::Qualified)
    );

    let incomplete_ledger = managed_facts([Failed, Skipped, Skipped, Skipped]);
    assert_eq!(
        derive_status_from_facts(incomplete_ledger),
        Err(GenerationQualificationRecordV1Error::InvalidDecisionClosure)
    );
    assert_eq!(
        derive_status_from_facts(DecisionFacts {
            ledger_has_actual_failure: true,
            ..incomplete_ledger
        }),
        Ok(GenerationQualificationStatusV1::Rejected)
    );

    let incomplete_repeatability = managed_facts([Passed, Failed, Skipped, Skipped]);
    assert_eq!(
        derive_status_from_facts(incomplete_repeatability),
        Err(GenerationQualificationRecordV1Error::InvalidDecisionClosure)
    );
    assert_eq!(
        derive_status_from_facts(DecisionFacts {
            repeatability_has_actual_failure: true,
            ..incomplete_repeatability
        }),
        Ok(GenerationQualificationStatusV1::Rejected)
    );
    assert_eq!(
        derive_status_from_facts(managed_facts([Passed, Passed, Failed, Skipped])),
        Ok(GenerationQualificationStatusV1::Rejected)
    );
    assert_eq!(
        derive_status_from_facts(managed_facts([Passed, Passed, Passed, Failed])),
        Ok(GenerationQualificationStatusV1::Rejected)
    );
    assert_eq!(
        derive_status_from_facts(managed_facts([Passed, Passed, Passed, Skipped])),
        Err(GenerationQualificationRecordV1Error::InvalidDecisionClosure)
    );
}

#[test]
fn eligibility_gates_deadline_terminal_finalization_and_pretraffic_rejection() {
    use GenerationQualificationPhaseStatusV1::{Passed, Skipped};
    let qualified = managed_facts([Passed, Passed, Passed, Passed]);
    for invalid in [
        DecisionFacts {
            terminal_status: GenerationQualificationOperationTerminalStatusV1::Cancelled,
            ..qualified
        },
        DecisionFacts {
            finalization_status: GenerationQualificationOperationFinalizationStatusV1::Failed,
            ..qualified
        },
    ] {
        assert_eq!(
            derive_status_from_facts(invalid),
            Err(GenerationQualificationRecordV1Error::IneligibleOperation)
        );
    }
    for invalid in [
        DecisionFacts {
            under_deadline: false,
            ..qualified
        },
        DecisionFacts {
            peak_concurrent_attempts: 0,
            ..qualified
        },
        DecisionFacts {
            platform_status: GenerationQualificationPlatformStatusV1::Rejected,
            ..qualified
        },
        DecisionFacts {
            license_decision: GenerationQualificationLicenseDecisionV1::Rejected,
            ..qualified
        },
    ] {
        assert_eq!(
            derive_status_from_facts(invalid),
            Err(GenerationQualificationRecordV1Error::InvalidDecisionClosure)
        );
    }

    let pretraffic = DecisionFacts {
        finalization_status: GenerationQualificationOperationFinalizationStatusV1::NotRequired,
        peak_concurrent_attempts: 0,
        platform_status: GenerationQualificationPlatformStatusV1::Rejected,
        phase_statuses: [Skipped; 4],
        ..qualified
    };
    assert_eq!(
        derive_status_from_facts(pretraffic),
        Ok(GenerationQualificationStatusV1::Rejected)
    );
    assert_eq!(
        derive_status_from_facts(DecisionFacts {
            platform_status: GenerationQualificationPlatformStatusV1::Supported,
            license_decision: GenerationQualificationLicenseDecisionV1::Rejected,
            ..pretraffic
        }),
        Ok(GenerationQualificationStatusV1::Rejected)
    );
    for invalid in [
        DecisionFacts {
            platform_status: GenerationQualificationPlatformStatusV1::Supported,
            license_decision: GenerationQualificationLicenseDecisionV1::LocalUseOnly,
            ..pretraffic
        },
        DecisionFacts {
            phase_statuses: [Passed, Skipped, Skipped, Skipped],
            ..pretraffic
        },
        DecisionFacts {
            under_deadline: false,
            ..pretraffic
        },
    ] {
        assert_eq!(
            derive_status_from_facts(invalid),
            Err(GenerationQualificationRecordV1Error::InvalidDecisionClosure)
        );
    }
}
