use super::*;

fn values_at(limit: u64) -> ResourceAttemptValues {
    ResourceAttemptValues {
        prompt_token_count: 1,
        generated_token_count: 1,
        total_duration_nanoseconds: limit,
        load_duration_nanoseconds: 0,
        prompt_evaluation_duration_nanoseconds: 0,
        evaluation_duration_nanoseconds: limit,
        attempt_elapsed_nanoseconds: limit,
        first_response_elapsed_nanoseconds: limit,
        cleanup_elapsed_nanoseconds: limit,
        worker_high_water_resident_bytes: limit,
        runtime_installed_payload_bytes: limit / 2,
        model_installed_payload_bytes: limit - (limit / 2),
        installed_footprint_bytes: limit,
    }
}

fn limits(value: u64) -> GenerationQualificationResourcePolicyLimitsV1 {
    GenerationQualificationResourcePolicyLimitsV1 {
        maximum_attempt_elapsed_nanoseconds: value,
        maximum_first_response_nanoseconds: value,
        maximum_cleanup_nanoseconds: value,
        maximum_worker_high_water_resident_bytes: value,
        maximum_installed_footprint_bytes: value,
    }
}

#[test]
fn equality_is_within_every_limit_and_strict_exceedance_is_semantically_ordered() {
    assert!(exceeded_limits(values_at(10), limits(10)).is_empty());
    assert_eq!(
        exceeded_limits(values_at(11), limits(10)),
        vec![
            GenerationResourceExceededLimitV1::AttemptElapsed,
            GenerationResourceExceededLimitV1::FirstResponse,
            GenerationResourceExceededLimitV1::Cleanup,
            GenerationResourceExceededLimitV1::WorkerHighWaterResident,
            GenerationResourceExceededLimitV1::InstalledFootprint,
        ]
    );
}

#[test]
fn each_limit_comparison_is_independent() {
    let baseline = values_at(10);
    let expected = [
        GenerationResourceExceededLimitV1::AttemptElapsed,
        GenerationResourceExceededLimitV1::FirstResponse,
        GenerationResourceExceededLimitV1::Cleanup,
        GenerationResourceExceededLimitV1::WorkerHighWaterResident,
        GenerationResourceExceededLimitV1::InstalledFootprint,
    ];
    for (index, limit) in expected.into_iter().enumerate() {
        let mut changed = baseline;
        match index {
            0 => changed.attempt_elapsed_nanoseconds = 11,
            1 => changed.first_response_elapsed_nanoseconds = 11,
            2 => changed.cleanup_elapsed_nanoseconds = 11,
            3 => changed.worker_high_water_resident_bytes = 11,
            4 => changed.installed_footprint_bytes = 11,
            _ => unreachable!(),
        }
        assert_eq!(exceeded_limits(changed, limits(10)), vec![limit]);
    }
}

#[test]
fn duration_and_package_size_conversion_fail_closed_on_overflow() {
    assert_eq!(duration_nanoseconds(Duration::from_nanos(7)), Ok(7));
    assert_eq!(
        duration_nanoseconds(Duration::from_secs(u64::MAX)),
        Err(Error::MeasurementOverflow)
    );
    assert_eq!(installed_footprint(7, 11), Ok(18));
    assert_eq!(
        installed_footprint(u64::MAX, 1),
        Err(Error::MeasurementOverflow)
    );
}

#[test]
fn provider_and_monotonic_observation_inconsistency_is_rejected() {
    let valid = values_at(10);
    assert_eq!(validate_observation_consistency(valid), Ok(()));
    for changed in [
        ResourceAttemptValues {
            load_duration_nanoseconds: 11,
            ..valid
        },
        ResourceAttemptValues {
            total_duration_nanoseconds: 11,
            ..valid
        },
        ResourceAttemptValues {
            first_response_elapsed_nanoseconds: 11,
            ..valid
        },
        ResourceAttemptValues {
            cleanup_elapsed_nanoseconds: 11,
            ..valid
        },
    ] {
        assert_eq!(
            validate_observation_consistency(changed),
            Err(Error::ObservationInconsistent)
        );
    }
    let overflow = ResourceAttemptValues {
        load_duration_nanoseconds: u64::MAX,
        prompt_evaluation_duration_nanoseconds: 1,
        ..valid
    };
    assert_eq!(
        validate_observation_consistency(overflow),
        Err(Error::MeasurementOverflow)
    );
}
