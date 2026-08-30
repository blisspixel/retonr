use std::cell::RefCell;

use super::*;

fn failure_for(finalizer: MandatoryFinalizer) -> MandatoryFinalizerFailure {
    match finalizer {
        MandatoryFinalizer::RequestProjection => MandatoryFinalizerFailure::RequestProjection(
            GenerationQualificationRequestProjectionError::ProjectionDrift,
        ),
        MandatoryFinalizer::CanonicalReadback => MandatoryFinalizerFailure::CanonicalReadback(
            GenerationQualificationPreparationError::ReadbackMismatch,
        ),
        MandatoryFinalizer::PlatformAuthority => MandatoryFinalizerFailure::PlatformAuthority(
            GenerationQualificationPlatformAssessmentError::InvalidPortableClosure,
        ),
        MandatoryFinalizer::LicenseAuthority => MandatoryFinalizerFailure::LicenseAuthority(
            GenerationQualificationLicenseAssessmentError::Cancelled,
        ),
    }
}

#[test]
fn every_failure_combination_runs_in_order_with_fresh_tokens() {
    for failure_mask in 0_u8..16 {
        let order = RefCell::new(Vec::new());
        let report = run_all_finalizers(|finalizer, cancellation| {
            assert!(!cancellation.is_cancelled());
            order.borrow_mut().push(finalizer);
            cancellation.cancel();
            let bit = match finalizer {
                MandatoryFinalizer::RequestProjection => 1,
                MandatoryFinalizer::CanonicalReadback => 2,
                MandatoryFinalizer::PlatformAuthority => 4,
                MandatoryFinalizer::LicenseAuthority => 8,
            };
            if failure_mask & bit == 0 {
                Ok(())
            } else {
                Err(failure_for(finalizer))
            }
        });
        assert_eq!(
            *order.borrow(),
            [
                MandatoryFinalizer::RequestProjection,
                MandatoryFinalizer::CanonicalReadback,
                MandatoryFinalizer::PlatformAuthority,
                MandatoryFinalizer::LicenseAuthority,
            ]
        );
        assert_eq!(report.request_projection_failed(), failure_mask & 1 != 0);
        assert_eq!(report.canonical_readback_failed(), failure_mask & 2 != 0);
        assert_eq!(report.platform_authority_failed(), failure_mask & 4 != 0);
        assert_eq!(report.license_authority_failed(), failure_mask & 8 != 0);
    }
}

fn empty_report() -> PreparedGenerationQualificationMandatoryFinalizationFailures {
    run_all_finalizers(|_, _| Ok(()))
}

#[test]
fn validation_combiner_preserves_every_reachable_bracket_outcome() {
    assert!(matches!(
        combine_validation_results::<(), ()>(Err(empty_report()), None, Err(empty_report())),
        Err(PreparedGenerationQualificationMandatoryFinalizationError::InitialAndFinal { .. })
    ));
    assert!(matches!(
        combine_validation_results::<(), ()>(Err(empty_report()), None, Ok(())),
        Err(PreparedGenerationQualificationMandatoryFinalizationError::Initial(_))
    ));
    match combine_validation_results(Ok(()), Some(Err::<(), _>("callback")), Err(empty_report())) {
        Err(PreparedGenerationQualificationMandatoryFinalizationError::CallbackAndFinal {
            callback,
            ..
        }) => assert_eq!(callback, "callback"),
        _ => panic!("callback and final failure must both be retained"),
    }
    assert!(matches!(
        combine_validation_results(Ok(()), Some(Err::<(), _>("callback")), Ok(())),
        Err(PreparedGenerationQualificationMandatoryFinalizationError::Callback("callback"))
    ));
    assert!(matches!(
        combine_validation_results(Ok(()), Some(Ok::<_, ()>(7)), Err(empty_report())),
        Err(PreparedGenerationQualificationMandatoryFinalizationError::Final(_))
    ));
    assert_eq!(
        combine_validation_results(Ok(()), Some(Ok::<_, ()>(7)), Ok(())).expect("valid bracket"),
        7
    );
}

#[test]
fn report_and_bracket_debug_are_content_redacted() {
    let report = run_all_finalizers(|finalizer, _| Err(failure_for(finalizer)));
    let report_debug = format!("{report:?}");
    assert!(report_debug.contains("request_projection_failed: true"));
    assert!(!report_debug.contains("ProjectionDrift"));

    let bracket = PreparedGenerationQualificationMandatoryFinalizationError::CallbackAndFinal {
        callback: "sensitive callback content",
        final_validation: Box::new(empty_report()),
    };
    let bracket_debug = format!("{bracket:?}");
    assert!(bracket_debug.contains("<redacted>"));
    assert!(!bracket_debug.contains("sensitive callback content"));
}
