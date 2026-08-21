use crate::{IsolationError, IsolationResult, NamespaceIdentity};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReadyMessage {
    pub(super) guardian_pid: u32,
    pub(super) namespace_init_pid: u32,
    pub(super) network_namespace: NamespaceIdentity,
    pub(super) user_namespace: NamespaceIdentity,
    pub(super) process_namespace: NamespaceIdentity,
    pub(super) loopback_index: u32,
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
    if fields.len() != 11 || fields[0] != "READY" || fields[1] != "1" {
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
        u32::try_from(values[8]).map_err(|_error| IsolationError::HelperProtocol)?;
    if guardian_pid == 0 || namespace_init_pid == 0 || loopback_index == 0 {
        return Err(IsolationError::HelperProtocol);
    }
    Ok(ReadyMessage {
        guardian_pid,
        namespace_init_pid,
        network_namespace: NamespaceIdentity::new(values[2], values[3]),
        user_namespace: NamespaceIdentity::new(values[4], values[5]),
        process_namespace: NamespaceIdentity::new(values[6], values[7]),
        loopback_index,
    })
}

fn parse_helper_error(fields: &[&str]) -> IsolationResult<ReadyMessage> {
    if fields.len() != 3 || fields[1] != "1" {
        return Err(IsolationError::HelperProtocol);
    }
    let error = match fields[2] {
        "host-policy-denied" => IsolationError::HostPolicyDenied,
        "namespace-setup" => IsolationError::NamespaceSetup,
        "loopback-setup" => IsolationError::LoopbackSetup,
        "network-canary" => IsolationError::NetworkCanary,
        "descriptor-leak" => IsolationError::DescriptorLeak,
        "privilege-drop" => IsolationError::PrivilegeDrop,
        "socket-policy-compile" => IsolationError::SocketPolicyCompile,
        "socket-policy-install" => IsolationError::SocketPolicyInstall,
        "socket-policy-inactive" => IsolationError::SocketPolicyInactive,
        "socket-policy-behavior" => IsolationError::SocketPolicyBehavior,
        "invalid-launch" => IsolationError::InvalidLaunch("helper validation"),
        _ => IsolationError::HelperProtocol,
    };
    Err(error)
}

#[cfg(test)]
mod tests {
    use super::parse_ready;
    use crate::{IsolationError, NamespaceIdentity};

    #[test]
    fn ready_protocol_is_exact_and_bounded() {
        let parsed = parse_ready(b"READY 1 10 11 1 2 3 4 5 6 7\n").expect("parse ready");
        assert_eq!(parsed.guardian_pid, 10);
        assert_eq!(parsed.namespace_init_pid, 11);
        assert_eq!(parsed.network_namespace, NamespaceIdentity::new(1, 2));
        assert_eq!(
            parse_ready(b"READY 2 10 11 1 2 3 4 5 6 7\n"),
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
    }
}
