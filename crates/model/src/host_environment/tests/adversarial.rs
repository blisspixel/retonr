use serde_json::json;

use super::{fixture, fixture_input, mutate_json};
use crate::{
    ComputeBackend, ExecutionPlacement, HostEnvironmentV1, HostEnvironmentV1Error,
    MAX_HOST_ENVIRONMENT_JSON_BYTES, RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem,
};

#[test]
fn decoder_rejects_malformed_unknown_duplicate_trailing_and_noncanonical_json() {
    let host = fixture();
    assert_eq!(
        HostEnvironmentV1::from_json_bytes(b""),
        Err(HostEnvironmentV1Error::InvalidEncoding)
    );
    assert_eq!(
        HostEnvironmentV1::from_json_bytes(b"not json"),
        Err(HostEnvironmentV1Error::InvalidEncoding)
    );
    let unknown = mutate_json(&host, |value| value["unknown"] = json!(true));
    assert_eq!(
        HostEnvironmentV1::from_json_bytes(&unknown),
        Err(HostEnvironmentV1Error::InvalidEncoding)
    );
    let duplicate = host
        .to_canonical_json_bytes()
        .strip_prefix(b"{")
        .map(|rest| {
            let mut bytes = br#"{"schema_version":1,"#.to_vec();
            bytes.extend_from_slice(rest);
            bytes
        })
        .expect("object prefix");
    assert_eq!(
        HostEnvironmentV1::from_json_bytes(&duplicate),
        Err(HostEnvironmentV1Error::InvalidEncoding)
    );
    let mut trailing = host.to_canonical_json_bytes();
    trailing.extend_from_slice(b" true");
    assert_eq!(
        HostEnvironmentV1::from_json_bytes(&trailing),
        Err(HostEnvironmentV1Error::InvalidEncoding)
    );
    let pretty = serde_json::to_vec_pretty(&host).expect("pretty host JSON");
    assert_eq!(
        HostEnvironmentV1::from_json_bytes(&pretty),
        Err(HostEnvironmentV1Error::NonCanonicalEncoding)
    );
    assert_eq!(
        HostEnvironmentV1::from_json_bytes(&vec![b' '; MAX_HOST_ENVIRONMENT_JSON_BYTES + 1]),
        Err(HostEnvironmentV1Error::EncodedRecordTooLarge)
    );
}

#[test]
fn every_schema_and_closed_vocabulary_substitution_is_rejected() {
    let host = fixture();
    for path in [
        "schema_version",
        "operating_system.schema_version",
        "architecture.schema_version",
        "execution_class.schema_version",
        "hardware_envelope.schema_version",
    ] {
        let bytes = mutate_json(&host, |value| set_path(value, path, json!(2)));
        assert_eq!(
            HostEnvironmentV1::from_json_bytes(&bytes),
            Err(HostEnvironmentV1Error::UnsupportedSchema),
            "schema path {path}"
        );
    }
    for (path, replacement) in [
        ("operating_system.family", json!("plan9")),
        ("architecture.instruction_set", json!("riscv64")),
        ("architecture.abi", json!("linux_unknown")),
        ("execution_class.profile", json!("container_cpu")),
        ("execution_class.compute_backend", json!("native-cpu")),
        ("execution_class.placement", json!("cpu")),
        (
            "execution_class.observer_binary_assertion_mode",
            json!("unknown"),
        ),
        ("execution_class.accelerator_scope", json!("assessed")),
    ] {
        let bytes = mutate_json(&host, |value| set_path(value, path, replacement));
        assert_eq!(
            HostEnvironmentV1::from_json_bytes(&bytes),
            Err(HostEnvironmentV1Error::InvalidEncoding),
            "closed field {path}"
        );
    }
}

#[test]
fn platform_and_execution_relationship_substitutions_fail_closed() {
    for mutate in 0_u8..5 {
        let mut input = fixture_input();
        match mutate {
            0 => input.operating_system.family = RuntimeOperatingSystem::Windows,
            1 => input.architecture.instruction_set = RuntimeArchitecture::Aarch64,
            2 => input.architecture.abi = RuntimeAbi::LinuxMusl,
            3 => input.execution_class.compute_backend = ComputeBackend::Cuda,
            4 => input.execution_class.placement = ExecutionPlacement::Hybrid,
            _ => unreachable!(),
        }
        let expected = if mutate < 3 {
            HostEnvironmentV1Error::UnsupportedPlatform
        } else {
            HostEnvironmentV1Error::InvalidExecutionClass
        };
        assert_eq!(HostEnvironmentV1::new(input), Err(expected));
    }
}

#[test]
fn operating_system_and_cpu_strings_are_bounded_normalized_and_privacy_safe() {
    for value in ["", &"a".repeat(129)] {
        let mut input = fixture_input();
        input.operating_system.version = value.to_owned();
        assert_eq!(
            HostEnvironmentV1::new(input),
            Err(HostEnvironmentV1Error::InvalidOperatingSystemVersion)
        );
    }
    for value in ["6.8.0 generic", "/etc/os-release", "6.8.0:owner", "版本"] {
        let mut input = fixture_input();
        input.operating_system.version = value.to_owned();
        assert_eq!(
            HostEnvironmentV1::new(input),
            Err(HostEnvironmentV1Error::PrivacyProhibitedCharacter)
        );
    }
    for value in ["", &"a".repeat(129), " leading", "trailing ", "two  spaces"] {
        let mut input = fixture_input();
        input.hardware_envelope.cpu_model = value.to_owned();
        assert_eq!(
            HostEnvironmentV1::new(input),
            Err(HostEnvironmentV1Error::InvalidCpuModel)
        );
    }
    for value in ["CPU/serial", r"CPU\owner", "CPU:owner", "CPU™"] {
        let mut input = fixture_input();
        input.hardware_envelope.cpu_model = value.to_owned();
        assert_eq!(
            HostEnvironmentV1::new(input),
            Err(HostEnvironmentV1Error::PrivacyProhibitedCharacter)
        );
    }
}

#[test]
fn core_and_memory_bounds_accept_exact_edges_and_reject_every_invalid_relation() {
    for (physical, logical, memory) in [(1, 1, 1_024), (4_096, 8_192, 16_777_216)] {
        let mut input = fixture_input();
        input.hardware_envelope.physical_core_count = physical;
        input.hardware_envelope.logical_core_count = logical;
        input.hardware_envelope.total_system_memory_mib = memory;
        HostEnvironmentV1::new(input).expect("exact numeric edge");
    }
    for (physical, logical) in [(0, 1), (4_097, 4_097), (1, 0), (8, 7), (4_096, 8_193)] {
        let mut input = fixture_input();
        input.hardware_envelope.physical_core_count = physical;
        input.hardware_envelope.logical_core_count = logical;
        assert_eq!(
            HostEnvironmentV1::new(input),
            Err(HostEnvironmentV1Error::InvalidCoreCount)
        );
    }
    for (memory, granularity) in [
        (0, 1_024),
        (1_023, 1_024),
        (1_025, 1_024),
        (16_778_240, 1_024),
        (65_536, 512),
    ] {
        let mut input = fixture_input();
        input.hardware_envelope.total_system_memory_mib = memory;
        input.hardware_envelope.memory_rounding_granularity_mib = granularity;
        assert_eq!(
            HostEnvironmentV1::new(input),
            Err(HostEnvironmentV1Error::InvalidMemoryEnvelope)
        );
    }
}

#[test]
fn decoder_rejects_numeric_overflow_before_construction() {
    let host = fixture();
    let core_overflow = mutate_json(&host, |value| {
        value["hardware_envelope"]["physical_core_count"] = json!(u64::from(u32::MAX) + 1);
    });
    assert_eq!(
        HostEnvironmentV1::from_json_bytes(&core_overflow),
        Err(HostEnvironmentV1Error::InvalidEncoding)
    );

    let memory_overflow = host
        .to_canonical_json_bytes()
        .windows(b"65536".len())
        .position(|window| window == b"65536")
        .map(|offset| {
            let mut bytes = host.to_canonical_json_bytes();
            bytes.splice(
                offset..offset + b"65536".len(),
                b"18446744073709551616".iter().copied(),
            );
            bytes
        })
        .expect("memory token");
    assert_eq!(
        HostEnvironmentV1::from_json_bytes(&memory_overflow),
        Err(HostEnvironmentV1Error::InvalidEncoding)
    );
}

fn set_path(value: &mut serde_json::Value, path: &str, replacement: serde_json::Value) {
    let mut segments = path.split('.');
    let first = segments.next().expect("nonempty fixture path");
    if let Some(second) = segments.next() {
        value[first][second] = replacement;
    } else {
        value[first] = replacement;
    }
}
