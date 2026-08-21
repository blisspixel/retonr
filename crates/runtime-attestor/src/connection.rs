use std::net::SocketAddr;

#[cfg(any(target_os = "linux", windows))]
use std::net::IpAddr;

use rewrite_types::Digest;
use serde::Serialize;

use crate::AttachedProcessWitnessError;

/// Current retained TCP connection evidence contract version.
pub const RETAINED_TCP_CONNECTION_EVIDENCE_SCHEMA_VERSION: u32 = 1;

/// Exact client and server endpoints read from one caller-retained TCP stream.
///
/// This value deliberately has no serialization or debug implementation because
/// endpoints are sensitive observation inputs rather than evidence output.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct RetainedTcpConnection {
    client: SocketAddr,
    server: SocketAddr,
}

impl RetainedTcpConnection {
    /// Validates the exact endpoints from one connected loopback TCP stream.
    ///
    /// # Errors
    ///
    /// Returns [`AttachedProcessWitnessError::InvalidConnectionEndpoints`] for
    /// non-loopback, zero-port, mixed-family, equal, or scoped IPv6 endpoints.
    pub fn new(
        client: SocketAddr,
        server: SocketAddr,
    ) -> Result<Self, AttachedProcessWitnessError> {
        if !valid_endpoint(client)
            || !valid_endpoint(server)
            || client.is_ipv4() != server.is_ipv4()
            || client == server
        {
            return Err(AttachedProcessWitnessError::InvalidConnectionEndpoints);
        }
        Ok(Self { client, server })
    }

    /// Returns the exact client endpoint.
    #[must_use]
    pub const fn client(self) -> SocketAddr {
        self.client
    }

    /// Returns the exact server endpoint.
    #[must_use]
    pub const fn server(self) -> SocketAddr {
        self.server
    }
}

/// Native meaning of one successful exact connection attribution.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TcpConnectionAttributionKind {
    /// Windows reported the context-binding PID for the exact established row.
    WindowsContextBindingPid,
    /// Linux reported the exact socket inode and one visible same-UID holder.
    LinuxSocketInodeVisibleSameUidHolder,
}

/// Explicit limit on what socket-sharing fact the platform observation proves.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TcpConnectionSharingLimitation {
    /// Public Windows connection tables do not enumerate duplicated handles.
    WindowsDuplicatedHandlesNotObservable,
    /// Linux checks only descriptor holders visible under the current policy.
    LinuxOnlyVisibleSameUidDescriptorHoldersChecked,
}

/// Provider-neutral input for one inert retained connection evidence record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedTcpConnectionEvidenceInput {
    /// Native attribution meaning.
    pub attribution_kind: TcpConnectionAttributionKind,
    /// Explicit socket-sharing limitation paired with the attribution kind.
    pub sharing_limitation: TcpConnectionSharingLimitation,
    /// Digest of the retained process evidence used by the observer.
    pub process_evidence_digest: Digest,
    /// Domain-separated digest of platform-specific connection facts.
    pub platform_connection_digest: Digest,
}

/// Redacted evidence that one exact retained TCP connection reached the leased process.
///
/// The evidence does not claim exclusive socket ownership or identify the code
/// that produced an application response. It contains no endpoints, ports,
/// paths, native errors, or raw socket identifiers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RetainedTcpConnectionEvidence {
    schema_version: u32,
    attribution_kind: TcpConnectionAttributionKind,
    sharing_limitation: TcpConnectionSharingLimitation,
    evidence_digest: Digest,
}

impl RetainedTcpConnectionEvidence {
    /// Builds one redacted inert record from typed observer facts.
    ///
    /// # Errors
    ///
    /// Returns [`AttachedProcessWitnessError::InvalidEvidence`] when the
    /// attribution kind and sharing limitation do not describe the same
    /// admitted platform observation.
    pub fn new(
        input: &RetainedTcpConnectionEvidenceInput,
    ) -> Result<Self, AttachedProcessWitnessError> {
        if !valid_attribution_pair(input.attribution_kind, input.sharing_limitation) {
            return Err(AttachedProcessWitnessError::InvalidEvidence);
        }
        let evidence_digest = connection_evidence_digest(input);
        Ok(Self {
            schema_version: RETAINED_TCP_CONNECTION_EVIDENCE_SCHEMA_VERSION,
            attribution_kind: input.attribution_kind,
            sharing_limitation: input.sharing_limitation,
            evidence_digest,
        })
    }

    /// Returns the retained connection evidence contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the native attribution meaning.
    #[must_use]
    pub const fn attribution_kind(&self) -> TcpConnectionAttributionKind {
        self.attribution_kind
    }

    /// Returns the explicit socket-sharing limitation.
    #[must_use]
    pub const fn sharing_limitation(&self) -> TcpConnectionSharingLimitation {
        self.sharing_limitation
    }

    /// Returns the domain-separated digest of the redacted connection evidence.
    #[must_use]
    pub const fn evidence_digest(&self) -> &Digest {
        &self.evidence_digest
    }
}

fn valid_attribution_pair(
    kind: TcpConnectionAttributionKind,
    limitation: TcpConnectionSharingLimitation,
) -> bool {
    matches!(
        (kind, limitation),
        (
            TcpConnectionAttributionKind::WindowsContextBindingPid,
            TcpConnectionSharingLimitation::WindowsDuplicatedHandlesNotObservable
        ) | (
            TcpConnectionAttributionKind::LinuxSocketInodeVisibleSameUidHolder,
            TcpConnectionSharingLimitation::LinuxOnlyVisibleSameUidDescriptorHoldersChecked
        )
    )
}

fn connection_evidence_digest(input: &RetainedTcpConnectionEvidenceInput) -> Digest {
    let mut material = Vec::with_capacity(160);
    material.extend_from_slice(b"retonr:retained-tcp-connection-evidence:v1\0");
    material.push(match input.attribution_kind {
        TcpConnectionAttributionKind::WindowsContextBindingPid => 0,
        TcpConnectionAttributionKind::LinuxSocketInodeVisibleSameUidHolder => 1,
    });
    material.push(match input.sharing_limitation {
        TcpConnectionSharingLimitation::WindowsDuplicatedHandlesNotObservable => 0,
        TcpConnectionSharingLimitation::LinuxOnlyVisibleSameUidDescriptorHoldersChecked => 1,
    });
    material.extend_from_slice(input.process_evidence_digest.as_str().as_bytes());
    material.extend_from_slice(input.platform_connection_digest.as_str().as_bytes());
    Digest::sha256(&material)
}

#[cfg(any(target_os = "linux", windows))]
pub(crate) fn connection_digest_material(
    domain: &[u8],
    connection: RetainedTcpConnection,
) -> Vec<u8> {
    let mut material = Vec::with_capacity(domain.len().saturating_add(64));
    material.extend_from_slice(domain);
    append_endpoint(&mut material, connection.client());
    append_endpoint(&mut material, connection.server());
    material
}

#[cfg(any(target_os = "linux", windows))]
fn append_endpoint(material: &mut Vec<u8>, endpoint: SocketAddr) {
    match endpoint.ip() {
        IpAddr::V4(address) => {
            material.push(4);
            material.extend_from_slice(&address.octets());
        }
        IpAddr::V6(address) => {
            material.push(6);
            material.extend_from_slice(&address.octets());
        }
    }
    material.extend_from_slice(&endpoint.port().to_be_bytes());
}

fn valid_endpoint(endpoint: SocketAddr) -> bool {
    if !endpoint.ip().is_loopback() || endpoint.port() == 0 {
        return false;
    }
    match endpoint {
        SocketAddr::V4(_) => true,
        SocketAddr::V6(address) => address.flowinfo() == 0 && address.scope_id() == 0,
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6};

    use rewrite_types::Digest;

    use super::{
        RetainedTcpConnection, RetainedTcpConnectionEvidence, RetainedTcpConnectionEvidenceInput,
        TcpConnectionAttributionKind, TcpConnectionSharingLimitation,
    };
    use crate::AttachedProcessWitnessError;

    #[test]
    fn retained_connection_rejects_invalid_endpoint_pairs() {
        let loopback = SocketAddr::from((Ipv4Addr::LOCALHOST, 41_000));
        assert!(matches!(
            RetainedTcpConnection::new(loopback, loopback),
            Err(AttachedProcessWitnessError::InvalidConnectionEndpoints)
        ));
        assert!(matches!(
            RetainedTcpConnection::new(
                SocketAddr::from((Ipv4Addr::UNSPECIFIED, 41_001)),
                SocketAddr::from((Ipv4Addr::LOCALHOST, 11_434)),
            ),
            Err(AttachedProcessWitnessError::InvalidConnectionEndpoints)
        ));
        assert!(matches!(
            RetainedTcpConnection::new(
                SocketAddr::from((Ipv4Addr::LOCALHOST, 41_001)),
                SocketAddr::from((Ipv6Addr::LOCALHOST, 11_434)),
            ),
            Err(AttachedProcessWitnessError::InvalidConnectionEndpoints)
        ));
        assert!(matches!(
            RetainedTcpConnection::new(
                SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 41_001, 0, 2)),
                SocketAddr::from((Ipv6Addr::LOCALHOST, 11_434)),
            ),
            Err(AttachedProcessWitnessError::InvalidConnectionEndpoints)
        ));
    }

    #[test]
    fn connection_evidence_is_redacted_and_domain_separated() {
        let process = Digest::sha256(b"process");
        let platform = Digest::sha256(b"platform");
        let evidence = RetainedTcpConnectionEvidence::new(&RetainedTcpConnectionEvidenceInput {
            attribution_kind: TcpConnectionAttributionKind::WindowsContextBindingPid,
            sharing_limitation:
                TcpConnectionSharingLimitation::WindowsDuplicatedHandlesNotObservable,
            process_evidence_digest: process,
            platform_connection_digest: platform.clone(),
        })
        .expect("valid connection evidence");
        let encoded = serde_json::to_string(&evidence).expect("serialize connection evidence");
        assert!(encoded.contains("windows_context_binding_pid"));
        assert!(encoded.contains("windows_duplicated_handles_not_observable"));
        assert!(!encoded.contains("client"));
        assert!(!encoded.contains("server"));
        assert!(!encoded.contains("address"));
        assert!(!encoded.contains("port"));
        assert!(!encoded.contains("error"));
        assert_ne!(evidence.evidence_digest(), &platform);
    }

    #[test]
    fn connection_evidence_rejects_mismatched_platform_semantics() {
        assert_eq!(
            RetainedTcpConnectionEvidence::new(&RetainedTcpConnectionEvidenceInput {
                attribution_kind: TcpConnectionAttributionKind::WindowsContextBindingPid,
                sharing_limitation:
                    TcpConnectionSharingLimitation::LinuxOnlyVisibleSameUidDescriptorHoldersChecked,
                process_evidence_digest: Digest::sha256(b"process"),
                platform_connection_digest: Digest::sha256(b"platform"),
            }),
            Err(AttachedProcessWitnessError::InvalidEvidence)
        );
    }
}
