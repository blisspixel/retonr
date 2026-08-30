use rewrite_types::Digest;
use std::time::Duration;

use crate::contract::{
    RetainedRuntimeInputDeclaration, RuntimeInputObjectIdentity, runtime_input_file_digest,
    runtime_input_layout_digest,
};
use crate::{
    IsolationError, MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES, MAXIMUM_MANAGED_RUNTIME_INPUT_FILES,
};

use super::linux_helper_setup::HelperFailure;

pub(super) const MANAGED_LAUNCH_CAPABILITY_VERSION: u8 = 2;
pub(super) const MANAGED_INPUT_VERIFICATION_TIMEOUT: Duration = Duration::from_hours(4);
pub(super) const MANAGED_INPUT_HEADER_BYTES: usize = 80;
const MANAGED_INPUT_DECLARATION_VERSION: u8 = 1;
const DECLARATION_FIXED_BYTES: usize = 96;
const MAXIMUM_ALIAS_BYTES: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ManagedInputHeader {
    pub(super) count: usize,
    pub(super) total_bytes: u64,
    pub(super) layout_digest: Digest,
}

pub(super) fn header(
    declarations: &[RetainedRuntimeInputDeclaration],
) -> Result<ManagedInputHeader, HelperFailure> {
    if declarations.len() > MAXIMUM_MANAGED_RUNTIME_INPUT_FILES {
        return Err(HelperFailure::RuntimeInputObjectMismatch);
    }
    let total_bytes = declarations.iter().try_fold(0_u64, |total, declaration| {
        total
            .checked_add(declaration.expected_bytes)
            .filter(|bytes| *bytes <= MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES)
            .ok_or(HelperFailure::RuntimeInputObjectMismatch)
    })?;
    Ok(ManagedInputHeader {
        count: declarations.len(),
        total_bytes,
        layout_digest: runtime_input_layout_digest(declarations),
    })
}

pub(super) fn encode_header(header: &ManagedInputHeader) -> [u8; MANAGED_INPUT_HEADER_BYTES] {
    let mut encoded = [0_u8; MANAGED_INPUT_HEADER_BYTES];
    encoded[0] = MANAGED_LAUNCH_CAPABILITY_VERSION;
    encoded[2..4].copy_from_slice(
        &u16::try_from(header.count)
            .unwrap_or(u16::MAX)
            .to_be_bytes(),
    );
    encoded[8..16].copy_from_slice(&header.total_bytes.to_be_bytes());
    encoded[16..80].copy_from_slice(header.layout_digest.as_str().as_bytes());
    encoded
}

pub(super) fn decode_header(payload: &[u8]) -> Result<ManagedInputHeader, HelperFailure> {
    if payload.len() != MANAGED_INPUT_HEADER_BYTES
        || payload[0] != MANAGED_LAUNCH_CAPABILITY_VERSION
        || payload[1] != 0
        || payload[4..8].iter().any(|byte| *byte != 0)
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    let count = usize::from(u16::from_be_bytes(
        payload[2..4]
            .try_into()
            .map_err(|_| HelperFailure::InvalidLaunch)?,
    ));
    let total_bytes = u64::from_be_bytes(
        payload[8..16]
            .try_into()
            .map_err(|_| HelperFailure::InvalidLaunch)?,
    );
    let layout_digest = parse_digest(&payload[16..80])?;
    if count > MAXIMUM_MANAGED_RUNTIME_INPUT_FILES
        || total_bytes > MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES
        || (count == 0) != (total_bytes == 0)
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    Ok(ManagedInputHeader {
        count,
        total_bytes,
        layout_digest,
    })
}

pub(super) fn encode_declaration(
    index: usize,
    declaration: &RetainedRuntimeInputDeclaration,
) -> Result<Vec<u8>, HelperFailure> {
    let index = u16::try_from(index).map_err(|_| HelperFailure::RuntimeInputObjectMismatch)?;
    let alias_bytes = declaration.relative_alias.as_bytes();
    let alias_length =
        u16::try_from(alias_bytes.len()).map_err(|_| HelperFailure::RuntimeInputObjectMismatch)?;
    let mut encoded = Vec::with_capacity(DECLARATION_FIXED_BYTES + alias_bytes.len());
    encoded.push(MANAGED_INPUT_DECLARATION_VERSION);
    encoded.push(0);
    encoded.extend_from_slice(&index.to_be_bytes());
    encoded.extend_from_slice(&alias_length.to_be_bytes());
    encoded.extend_from_slice(&[0_u8; 2]);
    encoded.extend_from_slice(&declaration.expected_bytes.to_be_bytes());
    encoded.extend_from_slice(&declaration.identity.device.to_be_bytes());
    encoded.extend_from_slice(&declaration.identity.inode.to_be_bytes());
    encoded.extend_from_slice(declaration.expected_digest.as_str().as_bytes());
    encoded.extend_from_slice(alias_bytes);
    Ok(encoded)
}

pub(super) fn decode_declaration(
    payload: &[u8],
    expected_index: usize,
) -> Result<RetainedRuntimeInputDeclaration, HelperFailure> {
    if payload.len() < DECLARATION_FIXED_BYTES
        || payload[0] != MANAGED_INPUT_DECLARATION_VERSION
        || payload[1] != 0
        || payload[6..8].iter().any(|byte| *byte != 0)
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    let index = usize::from(read_u16(payload, 2)?);
    let alias_length = usize::from(read_u16(payload, 4)?);
    if index != expected_index
        || alias_length == 0
        || alias_length > MAXIMUM_ALIAS_BYTES
        || payload.len() != DECLARATION_FIXED_BYTES.saturating_add(alias_length)
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    let expected_bytes = read_u64(payload, 8)?;
    let identity = RuntimeInputObjectIdentity {
        device: read_u64(payload, 16)?,
        inode: read_u64(payload, 24)?,
    };
    let expected_digest = parse_digest(&payload[32..96])?;
    let relative_alias = std::str::from_utf8(&payload[96..])
        .map_err(|_| HelperFailure::InvalidLaunch)?
        .to_owned();
    RetainedRuntimeInputDeclaration::from_wire(
        relative_alias,
        expected_digest,
        expected_bytes,
        identity,
    )
    .map_err(|error| map_contract_error(&error))
}

pub(super) fn validate_complete(
    expected: &ManagedInputHeader,
    declarations: &[RetainedRuntimeInputDeclaration],
) -> Result<(), HelperFailure> {
    if declarations
        .windows(2)
        .any(|pair| pair[0].relative_alias >= pair[1].relative_alias)
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    let observed = header(declarations)?;
    if &observed == expected {
        Ok(())
    } else {
        Err(HelperFailure::InvalidLaunch)
    }
}

pub(super) fn validate_descriptor(
    declaration: &RetainedRuntimeInputDeclaration,
    file: &std::fs::File,
    cancellation: Option<&rewrite_types::CancellationToken>,
) -> Result<(), HelperFailure> {
    use std::os::unix::fs::MetadataExt as _;

    use rustix::fs::{OFlags, fcntl_getfl};

    let metadata = file
        .metadata()
        .map_err(|_| HelperFailure::RuntimeInputObjectMismatch)?;
    let flags = fcntl_getfl(file).map_err(|_| HelperFailure::RuntimeInputObjectMismatch)?;
    if metadata.is_file()
        && metadata.nlink() <= 1
        && metadata.len() == declaration.expected_bytes
        && metadata.dev() == declaration.identity.device
        && metadata.ino() == declaration.identity.inode
        && !flags.intersects(OFlags::WRONLY | OFlags::RDWR | OFlags::PATH)
        && runtime_input_file_digest(file, declaration.expected_bytes, cancellation)
            .is_ok_and(|digest| digest == declaration.expected_digest)
    {
        Ok(())
    } else {
        Err(HelperFailure::RuntimeInputObjectMismatch)
    }
}

fn parse_digest(bytes: &[u8]) -> Result<Digest, HelperFailure> {
    Digest::from_sha256_hex(
        std::str::from_utf8(bytes)
            .map_err(|_| HelperFailure::InvalidLaunch)?
            .to_owned(),
    )
    .map_err(|_| HelperFailure::InvalidLaunch)
}

fn read_u16(payload: &[u8], offset: usize) -> Result<u16, HelperFailure> {
    Ok(u16::from_be_bytes(
        payload
            .get(offset..offset + 2)
            .ok_or(HelperFailure::InvalidLaunch)?
            .try_into()
            .map_err(|_| HelperFailure::InvalidLaunch)?,
    ))
}

fn read_u64(payload: &[u8], offset: usize) -> Result<u64, HelperFailure> {
    Ok(u64::from_be_bytes(
        payload
            .get(offset..offset + 8)
            .ok_or(HelperFailure::InvalidLaunch)?
            .try_into()
            .map_err(|_| HelperFailure::InvalidLaunch)?,
    ))
}

fn map_contract_error(error: &IsolationError) -> HelperFailure {
    match error {
        IsolationError::RuntimeInputObjectMismatch => HelperFailure::RuntimeInputObjectMismatch,
        _ => HelperFailure::InvalidLaunch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(alias: &str, byte: u8) -> RetainedRuntimeInputDeclaration {
        RetainedRuntimeInputDeclaration::from_wire(
            alias.to_owned(),
            Digest::sha256(&[byte]),
            1,
            RuntimeInputObjectIdentity {
                device: 10 + u64::from(byte),
                inode: 20 + u64::from(byte),
            },
        )
        .expect("declaration")
    }

    #[test]
    fn managed_header_is_versioned_fixed_and_closed() {
        let declarations = [declaration("config/a", 1), declaration("model/b", 2)];
        let expected = header(&declarations).expect("header");
        let encoded = encode_header(&expected);
        assert_eq!(decode_header(&encoded), Ok(expected));
        for index in [0, 1, 4, 7] {
            let mut drifted = encoded;
            drifted[index] ^= 1;
            assert_eq!(decode_header(&drifted), Err(HelperFailure::InvalidLaunch));
        }
    }

    #[test]
    fn declaration_codec_rejects_index_reorder_and_shape_drift() {
        let declaration = declaration("model/model.gguf", 3);
        let encoded = encode_declaration(0, &declaration).expect("encode");
        assert_eq!(
            decode_declaration(&encoded, 0)
                .expect("decode")
                .relative_alias,
            "model/model.gguf"
        );
        assert_eq!(
            decode_declaration(&encoded, 1),
            Err(HelperFailure::InvalidLaunch)
        );
        for index in [0, 1, 6, 7] {
            let mut drifted = encoded.clone();
            drifted[index] ^= 1;
            assert_eq!(
                decode_declaration(&drifted, 0),
                Err(HelperFailure::InvalidLaunch)
            );
        }
        assert_eq!(
            decode_declaration(&encoded[..95], 0),
            Err(HelperFailure::InvalidLaunch)
        );
    }

    #[test]
    fn complete_contract_rejects_reorder_and_header_substitution() {
        let first = declaration("config/a", 1);
        let second = declaration("model/b", 2);
        let expected = header(&[first.clone(), second.clone()]).expect("header");
        assert!(validate_complete(&expected, &[first.clone(), second.clone()]).is_ok());
        assert_eq!(
            validate_complete(&expected, &[second, first]),
            Err(HelperFailure::InvalidLaunch)
        );
        let mut changed = expected;
        changed.total_bytes += 1;
        assert_eq!(
            validate_complete(
                &changed,
                &[declaration("config/a", 1), declaration("model/b", 2)]
            ),
            Err(HelperFailure::InvalidLaunch)
        );
    }
}
