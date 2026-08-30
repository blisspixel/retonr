use rewrite_types::Digest;

use crate::{
    IsolationError, IsolationResult, ManagedDeviceBoundaryEvidence, ManagedDeviceVisibilityPolicy,
    ManagedRuntimeInputEvidence, NamespaceIdentity,
    contract::{
        RetainedProgramBootstrapRootObservation, RetainedProgramBootstrapRootPostconditions,
    },
};

use super::linux_helper_setup::HelperFailure;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReadyMessage {
    pub(super) guardian_pid: u32,
    pub(super) namespace_init_pid: u32,
    pub(super) network_namespace: NamespaceIdentity,
    pub(super) user_namespace: NamespaceIdentity,
    pub(super) process_namespace: NamespaceIdentity,
    pub(super) mount_namespace: NamespaceIdentity,
    pub(super) device_boundary: ManagedDeviceBoundaryEvidence,
    pub(super) runtime_inputs: ManagedRuntimeInputEvidence,
    pub(super) loopback_index: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BuildReadyMessage {
    pub(super) guardian_pid: u32,
    pub(super) namespace_init_pid: u32,
    pub(super) network_namespace: NamespaceIdentity,
    pub(super) user_namespace: NamespaceIdentity,
    pub(super) process_namespace: NamespaceIdentity,
    pub(super) mount_namespace: NamespaceIdentity,
    pub(super) loopback_index: u32,
    pub(super) landlock_abi: u32,
}

pub(super) struct BootstrapReadyMessage {
    pub(super) guardian_pid: u32,
    pub(super) namespace_init_pid: u32,
    pub(super) network_namespace: NamespaceIdentity,
    pub(super) user_namespace: NamespaceIdentity,
    pub(super) process_namespace: NamespaceIdentity,
    pub(super) mount_namespace: NamespaceIdentity,
    pub(super) loopback_index: u32,
    pub(super) landlock_abi: u32,
    pub(super) root: RetainedProgramBootstrapRootObservation,
}

pub(super) fn parse_ready(bytes: &[u8]) -> IsolationResult<ReadyMessage> {
    if bytes.len() > 1_024 || !bytes.ends_with(b"\n") {
        return Err(IsolationError::HelperProtocol);
    }
    let text = std::str::from_utf8(bytes).map_err(|_error| IsolationError::HelperProtocol)?;
    let fields = text.split_ascii_whitespace().collect::<Vec<_>>();
    if fields.first() == Some(&"ERROR") {
        return parse_helper_error(&fields);
    }
    if fields.len() != 34 || fields[0] != "READY" || fields[1] != "3" {
        return Err(IsolationError::HelperProtocol);
    }
    let values = fields[2..31]
        .iter()
        .map(|field| field.parse::<u64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| IsolationError::HelperProtocol)?;
    let guardian_pid = u32::try_from(values[0]).map_err(|_error| IsolationError::HelperProtocol)?;
    let namespace_init_pid =
        u32::try_from(values[1]).map_err(|_error| IsolationError::HelperProtocol)?;
    let loopback_index =
        u32::try_from(values[10]).map_err(|_error| IsolationError::HelperProtocol)?;
    let policy = u8::try_from(values[11])
        .ok()
        .and_then(ManagedDeviceVisibilityPolicy::from_code)
        .ok_or(IsolationError::HelperProtocol)?;
    let postconditions =
        u8::try_from(values[12]).map_err(|_error| IsolationError::HelperProtocol)?;
    let major = u32::try_from(values[19]).map_err(|_error| IsolationError::HelperProtocol)?;
    let minor = u32::try_from(values[20]).map_err(|_error| IsolationError::HelperProtocol)?;
    let entries = u32::try_from(values[21]).map_err(|_error| IsolationError::HelperProtocol)?;
    let device_boundary = ManagedDeviceBoundaryEvidence::verified(
        NamespaceIdentity::new(values[13], values[14]),
        NamespaceIdentity::new(values[15], values[16]),
        NamespaceIdentity::new(values[17], values[18]),
        major,
        minor,
        entries,
    );
    let device_digest = Digest::from_sha256_hex(fields[31].to_owned())
        .map_err(|_error| IsolationError::HelperProtocol)?;
    let layout_digest = Digest::from_sha256_hex(fields[32].to_owned())
        .map_err(|_error| IsolationError::HelperProtocol)?;
    let runtime_digest = Digest::from_sha256_hex(fields[33].to_owned())
        .map_err(|_error| IsolationError::HelperProtocol)?;
    let member_count = u32::try_from(values[26]).map_err(|_| IsolationError::HelperProtocol)?;
    let runtime_inputs = ManagedRuntimeInputEvidence::verified(
        NamespaceIdentity::new(values[22], values[23]),
        NamespaceIdentity::new(values[24], values[25]),
        member_count,
        values[27],
        layout_digest,
    );
    if guardian_pid == 0
        || namespace_init_pid == 0
        || loopback_index == 0
        || !matches!(policy, ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1)
        || postconditions != ManagedDeviceBoundaryEvidence::REQUIRED_POSTCONDITIONS
        || values[8] == 0
        || values[9] == 0
        || !device_boundary.all_visibility_canaries_passed()
        || u8::try_from(values[28]).ok()
            != Some(ManagedRuntimeInputEvidence::REQUIRED_POSTCONDITIONS)
        || device_digest != device_boundary.redacted_digest()
        || !runtime_inputs.all_postconditions_passed()
        || runtime_digest != runtime_inputs.redacted_digest()
    {
        return Err(IsolationError::HelperProtocol);
    }
    Ok(ReadyMessage {
        guardian_pid,
        namespace_init_pid,
        network_namespace: NamespaceIdentity::new(values[2], values[3]),
        user_namespace: NamespaceIdentity::new(values[4], values[5]),
        process_namespace: NamespaceIdentity::new(values[6], values[7]),
        mount_namespace: NamespaceIdentity::new(values[8], values[9]),
        device_boundary,
        runtime_inputs,
        loopback_index,
    })
}

pub(super) fn parse_build_ready(bytes: &[u8]) -> IsolationResult<BuildReadyMessage> {
    if bytes.len() > 1_024 || !bytes.ends_with(b"\n") {
        return Err(IsolationError::HelperProtocol);
    }
    let text = std::str::from_utf8(bytes).map_err(|_error| IsolationError::HelperProtocol)?;
    let fields = text.split_ascii_whitespace().collect::<Vec<_>>();
    if fields.first() == Some(&"ERROR") {
        return parse_helper_error(&fields).map(|_ready| unreachable!());
    }
    if fields.len() != 14 || fields[0] != "BUILD_READY" || fields[1] != "2" {
        return Err(IsolationError::HelperProtocol);
    }
    let values = fields[2..]
        .iter()
        .map(|field| field.parse::<u64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| IsolationError::HelperProtocol)?;
    let guardian_pid = u32::try_from(values[0]).map_err(|_error| IsolationError::HelperProtocol)?;
    let namespace_init_pid =
        u32::try_from(values[1]).map_err(|_error| IsolationError::HelperProtocol)?;
    let loopback_index =
        u32::try_from(values[10]).map_err(|_error| IsolationError::HelperProtocol)?;
    let landlock_abi =
        u32::try_from(values[11]).map_err(|_error| IsolationError::HelperProtocol)?;
    if guardian_pid == 0 || namespace_init_pid == 0 || loopback_index == 0 || landlock_abi < 3 {
        return Err(IsolationError::HelperProtocol);
    }
    Ok(BuildReadyMessage {
        guardian_pid,
        namespace_init_pid,
        network_namespace: NamespaceIdentity::new(values[2], values[3]),
        user_namespace: NamespaceIdentity::new(values[4], values[5]),
        process_namespace: NamespaceIdentity::new(values[6], values[7]),
        mount_namespace: NamespaceIdentity::new(values[8], values[9]),
        loopback_index,
        landlock_abi,
    })
}

pub(super) fn parse_bootstrap_ready(bytes: &[u8]) -> IsolationResult<BootstrapReadyMessage> {
    if bytes.len() > 1_024 || !bytes.ends_with(b"\n") {
        return Err(IsolationError::HelperProtocol);
    }
    let text = std::str::from_utf8(bytes).map_err(|_error| IsolationError::HelperProtocol)?;
    let fields = text.split_ascii_whitespace().collect::<Vec<_>>();
    if fields.first() == Some(&"ERROR") {
        if fields.len() == 3 && fields[1] == "1" {
            return Err(decode_helper_failure_code(fields[2].as_bytes())
                .unwrap_or(IsolationError::HelperProtocol));
        }
        return Err(IsolationError::HelperProtocol);
    }
    if fields.len() != 24 || fields[0] != "BOOTSTRAP_READY" || fields[1] != "3" {
        return Err(IsolationError::HelperProtocol);
    }
    let numbers = fields[2..16]
        .iter()
        .map(|field| field.parse::<u64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| IsolationError::HelperProtocol)?;
    let guardian_pid =
        u32::try_from(numbers[0]).map_err(|_error| IsolationError::HelperProtocol)?;
    let namespace_init_pid =
        u32::try_from(numbers[1]).map_err(|_error| IsolationError::HelperProtocol)?;
    let loopback_index =
        u32::try_from(numbers[10]).map_err(|_error| IsolationError::HelperProtocol)?;
    let landlock_abi =
        u32::try_from(numbers[11]).map_err(|_error| IsolationError::HelperProtocol)?;
    if guardian_pid == 0
        || namespace_init_pid == 0
        || loopback_index == 0
        || landlock_abi < 3
        || numbers[12] == 0
        || numbers[13] == 0
    {
        return Err(IsolationError::HelperProtocol);
    }
    let digests = fields[16..]
        .iter()
        .map(|field| Digest::from_sha256_hex((*field).to_owned()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| IsolationError::HelperProtocol)?;
    Ok(BootstrapReadyMessage {
        guardian_pid,
        namespace_init_pid,
        network_namespace: NamespaceIdentity::new(numbers[2], numbers[3]),
        user_namespace: NamespaceIdentity::new(numbers[4], numbers[5]),
        process_namespace: NamespaceIdentity::new(numbers[6], numbers[7]),
        mount_namespace: NamespaceIdentity::new(numbers[8], numbers[9]),
        loopback_index,
        landlock_abi,
        root: RetainedProgramBootstrapRootObservation {
            root_mount: NamespaceIdentity::new(numbers[12], numbers[13]),
            alpine_installed_database_digest: digests[0].clone(),
            alpine_loader_digest: digests[1].clone(),
            alpine_libc_linker_name_digest: digests[2].clone(),
            alpine_libgcc_digest: digests[3].clone(),
            alpine_libgcc_linker_name_digest: digests[4].clone(),
            alpine_busybox_digest: digests[5].clone(),
            rust_toolchain_layout_digest: digests[6].clone(),
            normalized_alpine_link_plan_digest: digests[7].clone(),
            postconditions: RetainedProgramBootstrapRootPostconditions::from_observations([
                true, true, true, true, true,
            ]),
        },
    })
}

fn parse_helper_error(fields: &[&str]) -> IsolationResult<ReadyMessage> {
    if fields.len() != 3 || fields[1] != "1" {
        return Err(IsolationError::HelperProtocol);
    }
    Err(decode_helper_failure_code(fields[2].as_bytes()).unwrap_or(IsolationError::HelperProtocol))
}

pub(super) fn decode_helper_failure_code(code: &[u8]) -> Option<IsolationError> {
    HelperFailure::from_code(code).map(HelperFailure::into_isolation_error)
}

#[cfg(test)]
mod tests {
    use super::{
        decode_helper_failure_code, parse_bootstrap_ready, parse_build_ready, parse_ready,
    };
    use crate::contract::runtime_input_layout_digest;
    use crate::{
        IsolationError, ManagedDeviceBoundaryEvidence, ManagedRuntimeInputEvidence,
        NamespaceIdentity,
    };

    #[test]
    fn ready_protocol_is_exact_and_bounded() {
        let boundary = ManagedDeviceBoundaryEvidence::verified(
            NamespaceIdentity::new(10, 12),
            NamespaceIdentity::new(13, 14),
            NamespaceIdentity::new(15, 16),
            1,
            3,
            1,
        );
        let layout_digest = runtime_input_layout_digest(std::iter::empty());
        let runtime_inputs = ManagedRuntimeInputEvidence::verified(
            NamespaceIdentity::new(17, 18),
            NamespaceIdentity::new(19, 20),
            0,
            0,
            layout_digest,
        );
        let record = format!(
            "READY 3 10 11 1 2 3 4 5 6 7 8 9 1 63 10 12 13 14 15 16 1 3 1 17 18 19 20 0 0 63 {} {} {}\n",
            boundary.redacted_digest(),
            runtime_inputs.layout_digest(),
            runtime_inputs.redacted_digest(),
        );
        let parsed = parse_ready(record.as_bytes()).expect("parse ready");
        assert_eq!(parsed.guardian_pid, 10);
        assert_eq!(parsed.namespace_init_pid, 11);
        assert_eq!(parsed.network_namespace, NamespaceIdentity::new(1, 2));
        assert_eq!(parsed.mount_namespace, NamespaceIdentity::new(7, 8));
        assert_eq!(parsed.device_boundary, boundary);
        assert_eq!(parsed.runtime_inputs, runtime_inputs);
        assert_eq!(
            parse_ready(record.replacen("READY 3", "READY 2", 1).as_bytes()),
            Err(IsolationError::HelperProtocol)
        );
        assert_eq!(
            parse_ready(record.replacen(" 63 ", " 62 ", 1).as_bytes()),
            Err(IsolationError::HelperProtocol)
        );
        assert_eq!(
            parse_ready(record.replacen(" 7 8 9 ", " 0 8 9 ", 1).as_bytes()),
            Err(IsolationError::HelperProtocol)
        );
        assert_eq!(
            parse_ready(
                record
                    .replacen(boundary.redacted_digest().as_str(), &"0".repeat(64), 1)
                    .as_bytes()
            ),
            Err(IsolationError::HelperProtocol)
        );
        assert_eq!(
            parse_ready(&vec![b'a'; 1_025]),
            Err(IsolationError::HelperProtocol)
        );
        assert_eq!(
            parse_ready(b"ERROR 1 host-policy-denied\n"),
            Err(IsolationError::HostPolicyDenied)
        );
        assert_eq!(
            parse_ready(b"ERROR 1 socket-policy-behavior\n"),
            Err(IsolationError::SocketPolicyBehavior)
        );
        let build = parse_build_ready(b"BUILD_READY 2 10 11 1 2 3 4 5 6 7 8 9 3\n")
            .expect("parse build ready");
        assert_eq!(build.guardian_pid, 10);
        assert_eq!(build.namespace_init_pid, 11);
        assert_eq!(build.landlock_abi, 3);
        assert_eq!(build.mount_namespace, NamespaceIdentity::new(7, 8));
        assert_eq!(
            parse_build_ready(b"BUILD_READY 2 10 11 1 2 3 4 5 6 7 8 9 2\n"),
            Err(IsolationError::HelperProtocol)
        );
        assert_eq!(
            decode_helper_failure_code(b"controlled-build-object-mismatch"),
            Some(IsolationError::ControlledBuildObjectMismatch)
        );
        assert_eq!(decode_helper_failure_code(b"unknown"), None);
        assert_eq!(
            decode_helper_failure_code(b"controlled-build-object-mismatch\n"),
            None
        );
    }

    #[test]
    fn bootstrap_ready_binds_root_observations_and_rejects_weak_records() {
        let digest = "0".repeat(64);
        let record = format!(
            "BOOTSTRAP_READY 3 10 11 1 2 3 4 5 6 7 8 9 3 10 12 {digest} {digest} {digest} {digest} {digest} {digest} {digest} {digest}\n"
        );
        let parsed = parse_bootstrap_ready(record.as_bytes()).expect("bootstrap ready");
        assert_eq!(parsed.guardian_pid, 10);
        assert_eq!(parsed.namespace_init_pid, 11);
        assert_eq!(parsed.root.root_mount, NamespaceIdentity::new(10, 12));
        assert_eq!(
            parsed.root.postconditions,
            crate::contract::RetainedProgramBootstrapRootPostconditions::from_observations([
                true, true, true, true, true,
            ])
        );
        assert_eq!(
            parse_bootstrap_ready(record.replacen(" 3 10 12 ", " 2 10 12 ", 1).as_bytes())
                .map(|_| ()),
            Err(IsolationError::HelperProtocol)
        );
        assert_eq!(
            parse_bootstrap_ready(
                record
                    .replacen("BOOTSTRAP_READY 3", "BOOTSTRAP_READY 2", 1)
                    .as_bytes()
            )
            .map(|_| ()),
            Err(IsolationError::HelperProtocol)
        );
        assert_eq!(
            parse_bootstrap_ready(b"ERROR 1 bootstrap-root-verification\n").map(|_| ()),
            Err(IsolationError::BootstrapRootVerification)
        );
    }
}
