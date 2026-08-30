use serde_json::Value;

use super::*;

#[path = "tests/adversarial.rs"]
mod adversarial;

const OPERATING_SYSTEM_JSON: &str =
    r#"{"schema_version":1,"family":"linux","version":"6.8.0-52-generic"}"#;
const ARCHITECTURE_JSON: &str =
    r#"{"schema_version":1,"instruction_set":"x86_64","abi":"linux_gnu_libc"}"#;
const EXECUTION_CLASS_JSON: &str = r#"{"schema_version":1,"profile":"managed_linux_native_cpu","compute_backend":"native_cpu","placement":"cpu_only","observer_binary_assertion_mode":"disabled","accelerator_scope":"not_assessed_for_managed_native_cpu"}"#;
const HARDWARE_ENVELOPE_JSON: &str = r#"{"schema_version":1,"cpu_model":"AMD Ryzen 7 7840U","physical_core_count":8,"logical_core_count":16,"total_system_memory_mib":65536,"memory_rounding_granularity_mib":1024}"#;
const HOST_JSON: &str = r#"{"schema_version":1,"operating_system":{"schema_version":1,"family":"linux","version":"6.8.0-52-generic"},"architecture":{"schema_version":1,"instruction_set":"x86_64","abi":"linux_gnu_libc"},"execution_class":{"schema_version":1,"profile":"managed_linux_native_cpu","compute_backend":"native_cpu","placement":"cpu_only","observer_binary_assertion_mode":"disabled","accelerator_scope":"not_assessed_for_managed_native_cpu"},"hardware_envelope":{"schema_version":1,"cpu_model":"AMD Ryzen 7 7840U","physical_core_count":8,"logical_core_count":16,"total_system_memory_mib":65536,"memory_rounding_granularity_mib":1024}}"#;

pub(super) fn fixture_input() -> HostEnvironmentV1Input {
    HostEnvironmentV1Input {
        operating_system: HostOperatingSystemV1Input {
            family: RuntimeOperatingSystem::Linux,
            version: "6.8.0-52-generic".to_owned(),
        },
        architecture: HostArchitectureV1Input {
            instruction_set: RuntimeArchitecture::X86_64,
            abi: RuntimeAbi::LinuxGnuLibc,
        },
        execution_class: HostExecutionClassV1Input {
            profile: HostExecutionProfileV1::ManagedLinuxNativeCpu,
            compute_backend: ComputeBackend::NativeCpu,
            placement: ExecutionPlacement::CpuOnly,
            observer_binary_assertion_mode: ObserverBinaryAssertionModeV1::Disabled,
            accelerator_scope: HostAcceleratorScopeV1::NotAssessedForManagedNativeCpu,
        },
        hardware_envelope: HostHardwareEnvelopeV1Input {
            cpu_model: "AMD Ryzen 7 7840U".to_owned(),
            physical_core_count: 8,
            logical_core_count: 16,
            total_system_memory_mib: 65_536,
            memory_rounding_granularity_mib: 1_024,
        },
    }
}

pub(super) fn fixture() -> HostEnvironmentV1 {
    HostEnvironmentV1::new(fixture_input()).expect("host fixture")
}

#[test]
fn exact_json_and_all_five_identities_are_stable() {
    let host = fixture();

    assert_eq!(host.to_canonical_json_bytes(), HOST_JSON.as_bytes());
    assert_eq!(
        host.operating_system().to_canonical_json_bytes(),
        OPERATING_SYSTEM_JSON.as_bytes()
    );
    assert_eq!(
        host.architecture().to_canonical_json_bytes(),
        ARCHITECTURE_JSON.as_bytes()
    );
    assert_eq!(
        host.execution_class().to_canonical_json_bytes(),
        EXECUTION_CLASS_JSON.as_bytes()
    );
    assert_eq!(
        host.hardware_envelope().to_canonical_json_bytes(),
        HARDWARE_ENVELOPE_JSON.as_bytes()
    );
    assert_eq!(
        host.operating_system()
            .operating_system_digest()
            .digest()
            .as_str(),
        "d85a813469c4f8b0eb5a313cd5b9c61b43759424ffbeb902cdb8700fec013bd9"
    );
    assert_eq!(
        host.architecture().architecture_digest().digest().as_str(),
        "dee3ef936d7b526ea355036d76fd221af9f05b75d43f88edb8f8938cd19e8118"
    );
    assert_eq!(
        host.execution_class()
            .execution_class_digest()
            .digest()
            .as_str(),
        "f808e2f7570c0e9002d85e87aa08b8112ae4e817a189eb9b50aedaaee8e5f763"
    );
    assert_eq!(
        host.hardware_envelope()
            .hardware_envelope_digest()
            .digest()
            .as_str(),
        "6d153caf106c0550550198ed3755ca914c20cd81704137e0b52059f032d9f829"
    );
    assert_eq!(
        host.host_environment_id().digest().as_str(),
        "9877de813c1f6bf059e7847bb50116e22eddf9f94f65c4988712e7cca3c726fc"
    );
    let digest_set = host.digest_set();
    assert_eq!(digest_set.host_environment_id(), host.host_environment_id());
    assert_eq!(
        digest_set.operating_system_digest(),
        host.operating_system().operating_system_digest()
    );
    assert_eq!(
        digest_set.architecture_digest(),
        host.architecture().architecture_digest()
    );
    assert_eq!(
        digest_set.execution_class_digest(),
        host.execution_class().execution_class_digest()
    );
    assert_eq!(
        digest_set.hardware_envelope_digest(),
        host.hardware_envelope().hardware_envelope_digest()
    );
}

#[test]
fn canonical_round_trip_preserves_every_accessor() {
    let host = fixture();
    let decoded = HostEnvironmentV1::from_json_bytes(&host.to_canonical_json_bytes())
        .expect("canonical round trip");

    assert_eq!(decoded, host);
    assert_eq!(decoded.schema_version(), HOST_ENVIRONMENT_SCHEMA_VERSION);
    assert_eq!(
        decoded.operating_system().family(),
        RuntimeOperatingSystem::Linux
    );
    assert_eq!(decoded.operating_system().version(), "6.8.0-52-generic");
    assert_eq!(
        decoded.architecture().instruction_set(),
        RuntimeArchitecture::X86_64
    );
    assert_eq!(decoded.architecture().abi(), RuntimeAbi::LinuxGnuLibc);
    assert_eq!(
        decoded.execution_class().profile(),
        HostExecutionProfileV1::ManagedLinuxNativeCpu
    );
    assert_eq!(
        decoded.execution_class().compute_backend(),
        ComputeBackend::NativeCpu
    );
    assert_eq!(
        decoded.execution_class().placement(),
        ExecutionPlacement::CpuOnly
    );
    assert_eq!(
        decoded.execution_class().observer_binary_assertion_mode(),
        ObserverBinaryAssertionModeV1::Disabled
    );
    assert_eq!(decoded.hardware_envelope().physical_core_count(), 8);
    assert_eq!(decoded.hardware_envelope().logical_core_count(), 16);
    assert_eq!(
        decoded.hardware_envelope().total_system_memory_mib(),
        65_536
    );
    assert_eq!(
        decoded
            .hardware_envelope()
            .memory_rounding_granularity_mib(),
        1_024
    );
}

#[test]
fn assertion_mode_is_observed_but_not_rejected_by_the_model_contract() {
    let disabled = fixture();
    let mut input = fixture_input();
    input.execution_class.observer_binary_assertion_mode = ObserverBinaryAssertionModeV1::Enabled;
    let enabled = HostEnvironmentV1::new(input).expect("debug assertion mode is observable");

    assert_ne!(
        enabled.host_environment_id(),
        disabled.host_environment_id()
    );
    assert_ne!(
        enabled.execution_class().execution_class_digest(),
        disabled.execution_class().execution_class_digest()
    );
    assert_eq!(
        enabled.operating_system().operating_system_digest(),
        disabled.operating_system().operating_system_digest()
    );
    assert_eq!(
        enabled.architecture().architecture_digest(),
        disabled.architecture().architecture_digest()
    );
    assert_eq!(
        enabled.hardware_envelope().hardware_envelope_digest(),
        disabled.hardware_envelope().hardware_envelope_digest()
    );
}

#[test]
fn each_mutable_projection_is_isolated_and_full_identity_is_sensitive() {
    let base = fixture();
    for mutation in 0_u8..3 {
        let mut input = fixture_input();
        match mutation {
            0 => input.operating_system.version = "6.8.0-53-generic".to_owned(),
            1 => {
                input.execution_class.observer_binary_assertion_mode =
                    ObserverBinaryAssertionModeV1::Enabled;
            }
            2 => input.hardware_envelope.total_system_memory_mib += 1_024,
            _ => unreachable!(),
        }
        let changed = HostEnvironmentV1::new(input).expect("valid isolated mutation");
        let equality = [
            changed.operating_system().operating_system_digest()
                == base.operating_system().operating_system_digest(),
            changed.architecture().architecture_digest()
                == base.architecture().architecture_digest(),
            changed.execution_class().execution_class_digest()
                == base.execution_class().execution_class_digest(),
            changed.hardware_envelope().hardware_envelope_digest()
                == base.hardware_envelope().hardware_envelope_digest(),
        ];
        let changed_index = match mutation {
            0 => 0,
            1 => 2,
            2 => 3,
            _ => unreachable!(),
        };
        assert_eq!(equality.iter().filter(|same| !**same).count(), 1);
        assert!(!equality[changed_index]);
        assert_ne!(changed.host_environment_id(), base.host_environment_id());
    }

    let architecture_json = base.architecture().to_canonical_json_bytes();
    let changed_architecture = architecture_json
        .windows(b"x86_64".len())
        .position(|window| window == b"x86_64")
        .map(|offset| {
            let mut bytes = architecture_json.clone();
            bytes[offset..offset + b"x86_64".len()].copy_from_slice(b"arm_64");
            bytes
        })
        .expect("architecture token");
    assert_ne!(
        length_framed_digest(
            HOST_ENVIRONMENT_ARCHITECTURE_DIGEST_DOMAIN,
            &architecture_json
        ),
        length_framed_digest(
            HOST_ENVIRONMENT_ARCHITECTURE_DIGEST_DOMAIN,
            &changed_architecture
        )
    );
}

#[test]
fn all_projection_domains_are_pairwise_noninterchangeable() {
    let bytes = b"same canonical projection";
    let digests = [
        length_framed_digest(HOST_ENVIRONMENT_OPERATING_SYSTEM_DIGEST_DOMAIN, bytes),
        length_framed_digest(HOST_ENVIRONMENT_ARCHITECTURE_DIGEST_DOMAIN, bytes),
        length_framed_digest(HOST_ENVIRONMENT_EXECUTION_CLASS_DIGEST_DOMAIN, bytes),
        length_framed_digest(HOST_ENVIRONMENT_HARDWARE_ENVELOPE_DIGEST_DOMAIN, bytes),
    ];
    for (index, digest) in digests.iter().enumerate() {
        assert!(!digests[index + 1..].contains(digest));
    }
}

#[test]
fn debug_is_redacted() {
    let host = fixture();
    let debug = format!("{host:?}");
    assert_eq!(debug, "HostEnvironmentV1 { schema_version: 1, .. }");
    assert!(!debug.contains(host.operating_system().version()));
    assert!(!debug.contains(host.hardware_envelope().cpu_model()));
    assert!(!debug.contains(host.host_environment_id().digest().as_str()));
}

pub(super) fn mutate_json(host: &HostEnvironmentV1, mutate: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut value: Value =
        serde_json::from_slice(&host.to_canonical_json_bytes()).expect("fixture JSON must decode");
    mutate(&mut value);
    serde_json::to_vec(&value).expect("mutated JSON must encode")
}
