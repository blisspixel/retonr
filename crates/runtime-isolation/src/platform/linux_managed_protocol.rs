use rewrite_types::Digest;

use crate::{
    ManagedDeviceBoundaryEvidence, ManagedDeviceVisibilityPolicy, ManagedRuntimeInputEvidence,
    NamespaceIdentity,
};

use super::linux_helper_setup::{
    HelperFailure, NamespaceEvidence, RawNamespaceIdentity, mount_namespace_identity,
};

const MANAGED_EVIDENCE_VERSION: u8 = 2;
pub(super) const MANAGED_EVIDENCE_BYTES: usize = 248;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ManagedReadyEvidence {
    pub(super) namespace: NamespaceEvidence,
    pub(super) mount: RawNamespaceIdentity,
    pub(super) device_boundary: ManagedDeviceBoundaryEvidence,
    pub(super) runtime_inputs: ManagedRuntimeInputEvidence,
}

impl ManagedReadyEvidence {
    pub(super) fn current(
        device_boundary: ManagedDeviceBoundaryEvidence,
        runtime_inputs: ManagedRuntimeInputEvidence,
    ) -> Result<Self, HelperFailure> {
        if !device_boundary.all_visibility_canaries_passed()
            || !runtime_inputs.all_postconditions_passed()
        {
            return Err(HelperFailure::ManagedDeviceBoundaryBehavior);
        }
        Ok(Self {
            namespace: NamespaceEvidence::current()?,
            mount: mount_namespace_identity()?,
            device_boundary,
            runtime_inputs,
        })
    }
}

pub(super) fn encode(evidence: &ManagedReadyEvidence) -> [u8; MANAGED_EVIDENCE_BYTES] {
    let mut encoded = [0_u8; MANAGED_EVIDENCE_BYTES];
    encoded[0] = MANAGED_EVIDENCE_VERSION;
    encoded[1] = evidence.device_boundary.policy().code();
    encoded[2] = evidence.device_boundary.postconditions();
    encoded[3] = evidence.runtime_inputs.postconditions();
    let identities = [
        evidence.namespace.network,
        evidence.namespace.user,
        evidence.namespace.process,
        evidence.mount,
        raw(evidence.device_boundary.device_mount()),
        raw(evidence.device_boundary.proc_mount()),
        raw(evidence.device_boundary.null_device()),
    ];
    let mut offset = 8;
    for identity in identities {
        for value in [identity.device, identity.inode] {
            encoded[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
            offset += 8;
        }
    }
    let (major, minor) = evidence.device_boundary.null_device_numbers();
    for value in [
        major,
        minor,
        evidence.device_boundary.visible_device_entries(),
    ] {
        encoded[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        offset += 4;
    }
    for identity in [
        evidence.runtime_inputs.scratch_mount(),
        evidence.runtime_inputs.input_mount(),
    ] {
        for value in [identity.device(), identity.inode()] {
            encoded[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
            offset += 8;
        }
    }
    encoded[offset..offset + 4]
        .copy_from_slice(&evidence.runtime_inputs.member_count().to_be_bytes());
    offset += 4;
    encoded[offset..offset + 8]
        .copy_from_slice(&evidence.runtime_inputs.total_bytes().to_be_bytes());
    offset += 8;
    encoded[offset..offset + 64]
        .copy_from_slice(evidence.runtime_inputs.layout_digest().as_str().as_bytes());
    encoded
}

pub(super) fn decode(payload: &[u8]) -> Result<ManagedReadyEvidence, HelperFailure> {
    if payload.len() != MANAGED_EVIDENCE_BYTES
        || payload[0] != MANAGED_EVIDENCE_VERSION
        || ManagedDeviceVisibilityPolicy::from_code(payload[1])
            != Some(ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1)
        || payload[2] != ManagedDeviceBoundaryEvidence::REQUIRED_POSTCONDITIONS
        || payload[3] != ManagedRuntimeInputEvidence::REQUIRED_POSTCONDITIONS
        || payload[4..8].iter().any(|byte| *byte != 0)
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    let mut offset = 8;
    let mut identities = [RawNamespaceIdentity {
        device: 0,
        inode: 0,
    }; 7];
    for identity in &mut identities {
        identity.device = read_u64(payload, &mut offset)?;
        identity.inode = read_u64(payload, &mut offset)?;
        if identity.device == 0 || identity.inode == 0 {
            return Err(HelperFailure::InvalidLaunch);
        }
    }
    let major = read_u32(payload, &mut offset)?;
    let minor = read_u32(payload, &mut offset)?;
    let entries = read_u32(payload, &mut offset)?;
    if major != 1 || minor != 3 || entries != 1 {
        return Err(HelperFailure::InvalidLaunch);
    }
    let device_boundary = ManagedDeviceBoundaryEvidence::verified(
        public(identities[4]),
        public(identities[5]),
        public(identities[6]),
        major,
        minor,
        entries,
    );
    let scratch_mount = NamespaceIdentity::new(
        read_u64(payload, &mut offset)?,
        read_u64(payload, &mut offset)?,
    );
    let input_mount = NamespaceIdentity::new(
        read_u64(payload, &mut offset)?,
        read_u64(payload, &mut offset)?,
    );
    let member_count = read_u32(payload, &mut offset)?;
    let total_bytes = read_u64(payload, &mut offset)?;
    let digest_end = offset.checked_add(64).ok_or(HelperFailure::InvalidLaunch)?;
    let layout_digest = Digest::from_sha256_hex(
        std::str::from_utf8(
            payload
                .get(offset..digest_end)
                .ok_or(HelperFailure::InvalidLaunch)?,
        )
        .map_err(|_| HelperFailure::InvalidLaunch)?
        .to_owned(),
    )
    .map_err(|_| HelperFailure::InvalidLaunch)?;
    offset = digest_end;
    if payload[offset..].iter().any(|byte| *byte != 0) {
        return Err(HelperFailure::InvalidLaunch);
    }
    let runtime_inputs = ManagedRuntimeInputEvidence::verified(
        scratch_mount,
        input_mount,
        member_count,
        total_bytes,
        layout_digest,
    );
    if !runtime_inputs.all_postconditions_passed() {
        return Err(HelperFailure::InvalidLaunch);
    }
    Ok(ManagedReadyEvidence {
        namespace: NamespaceEvidence {
            network: identities[0],
            user: identities[1],
            process: identities[2],
        },
        mount: identities[3],
        device_boundary,
        runtime_inputs,
    })
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, HelperFailure> {
    let end = offset.checked_add(8).ok_or(HelperFailure::InvalidLaunch)?;
    let value = u64::from_be_bytes(
        payload
            .get(*offset..end)
            .ok_or(HelperFailure::InvalidLaunch)?
            .try_into()
            .map_err(|_| HelperFailure::InvalidLaunch)?,
    );
    *offset = end;
    Ok(value)
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, HelperFailure> {
    let end = offset.checked_add(4).ok_or(HelperFailure::InvalidLaunch)?;
    let value = u32::from_be_bytes(
        payload
            .get(*offset..end)
            .ok_or(HelperFailure::InvalidLaunch)?
            .try_into()
            .map_err(|_| HelperFailure::InvalidLaunch)?,
    );
    *offset = end;
    Ok(value)
}

const fn public(identity: RawNamespaceIdentity) -> NamespaceIdentity {
    NamespaceIdentity::new(identity.device, identity.inode)
}

const fn raw(identity: NamespaceIdentity) -> RawNamespaceIdentity {
    RawNamespaceIdentity {
        device: identity.device(),
        inode: identity.inode(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence() -> ManagedReadyEvidence {
        ManagedReadyEvidence {
            namespace: NamespaceEvidence {
                network: RawNamespaceIdentity {
                    device: 1,
                    inode: 2,
                },
                user: RawNamespaceIdentity {
                    device: 3,
                    inode: 4,
                },
                process: RawNamespaceIdentity {
                    device: 5,
                    inode: 6,
                },
            },
            mount: RawNamespaceIdentity {
                device: 7,
                inode: 8,
            },
            device_boundary: ManagedDeviceBoundaryEvidence::verified(
                NamespaceIdentity::new(9, 10),
                NamespaceIdentity::new(11, 12),
                NamespaceIdentity::new(13, 14),
                1,
                3,
                1,
            ),
            runtime_inputs: ManagedRuntimeInputEvidence::verified(
                NamespaceIdentity::new(15, 16),
                NamespaceIdentity::new(17, 18),
                0,
                0,
                Digest::sha256(b"empty-input"),
            ),
        }
    }

    #[test]
    fn managed_codec_is_fixed_width_and_rejects_every_header_drift() {
        let expected = evidence();
        let encoded = encode(&expected);
        assert_eq!(encoded.len(), MANAGED_EVIDENCE_BYTES);
        assert_eq!(decode(&encoded), Ok(expected));
        for index in 0..8 {
            let mut drifted = encoded;
            drifted[index] ^= 0xff;
            assert_eq!(decode(&drifted), Err(HelperFailure::InvalidLaunch));
        }
        assert_eq!(
            decode(&encoded[..encoded.len() - 1]),
            Err(HelperFailure::InvalidLaunch)
        );
    }

    #[test]
    fn managed_codec_rejects_zero_identity_and_device_shape_drift() {
        let encoded = encode(&evidence());
        for offset in [8, 16, 120, 124, 128, 136, 144, 152, 160] {
            let mut drifted = encoded;
            let width = if offset < 120 { 8 } else { 4 };
            drifted[offset..offset + width].fill(0);
            assert_eq!(decode(&drifted), Err(HelperFailure::InvalidLaunch));
        }
    }
}
