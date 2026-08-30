use std::{ffi::OsString, io, io::Read as _, io::Seek as _, io::Write as _};

use rewrite_model_store::CandidateGenerationEvidenceStorageRootId;
use rewrite_types::{CancellationToken, Digest};
use serde::{Deserialize, Serialize};

use crate::artifact_storage::{ManagedFile, PinnedDirectory, fingerprint_std_file};

use super::{
    CandidateGenerationEvidenceRepositoryError, MAX_ROOT_MARKER_BYTES, ROOT_ID_DOMAIN,
    ROOT_LAYOUT_ENTRY_COUNT, ROOT_MARKER_FILE, ROOT_MARKER_SCHEMA_VERSION, ROOT_NONCE_BYTES,
    map_active_storage, map_initial_storage,
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RootMarkerV1 {
    schema_version: u16,
    root_nonce: String,
}

pub(super) fn write_new_root_marker(
    root: &PinnedDirectory,
) -> Result<CandidateGenerationEvidenceStorageRootId, CandidateGenerationEvidenceRepositoryError> {
    let mut nonce = [0_u8; ROOT_NONCE_BYTES];
    getrandom::fill(&mut nonce).map_err(|error| {
        CandidateGenerationEvidenceRepositoryError::StorageIo(io::Error::other(error))
    })?;
    let marker = RootMarkerV1 {
        schema_version: ROOT_MARKER_SCHEMA_VERSION,
        root_nonce: encode_lower_hex(&nonce),
    };
    let bytes = serde_json::to_vec(&marker)
        .map_err(|_| CandidateGenerationEvidenceRepositoryError::InvalidRootMarker)?;
    if u64::try_from(bytes.len())
        .ok()
        .is_none_or(|length| length > MAX_ROOT_MARKER_BYTES)
    {
        return Err(CandidateGenerationEvidenceRepositoryError::InvalidRootMarker);
    }

    let (_, mut file) = root
        .create_staging_file_with(|| Ok(OsString::from(ROOT_MARKER_FILE)))
        .map_err(map_initial_storage)?;
    file.write_all(&bytes)
        .map_err(CandidateGenerationEvidenceRepositoryError::StorageIo)?;
    file.sync_all()
        .map_err(CandidateGenerationEvidenceRepositoryError::StorageIo)?;
    root.sync().map_err(map_initial_storage)?;
    derive_root_id(&nonce)
}

pub(super) fn read_root_marker(
    root: &PinnedDirectory,
) -> Result<
    (ManagedFile, CandidateGenerationEvidenceStorageRootId),
    CandidateGenerationEvidenceRepositoryError,
> {
    let marker = root
        .open_managed_file(
            std::ffi::OsStr::new(ROOT_MARKER_FILE),
            ROOT_LAYOUT_ENTRY_COUNT + 1,
            &CancellationToken::new(),
        )
        .map_err(map_initial_storage)?
        .ok_or(CandidateGenerationEvidenceRepositoryError::InvalidRootMarker)?;
    if !marker.fingerprint.has_single_link() || marker.byte_size > MAX_ROOT_MARKER_BYTES {
        return Err(CandidateGenerationEvidenceRepositoryError::InvalidRootMarker);
    }
    let bytes = read_marker_bytes_from_handle(&marker)?;
    let root_id = decode_root_marker_bytes(&bytes)?;
    root.recheck_managed_file_for_lifecycle(
        std::ffi::OsStr::new(ROOT_MARKER_FILE),
        &marker.fingerprint,
        ROOT_LAYOUT_ENTRY_COUNT + 1,
        &CancellationToken::new(),
    )
    .map_err(map_active_storage)?;
    Ok((marker, root_id))
}

pub(super) fn read_marker_bytes_from_handle(
    marker: &ManagedFile,
) -> Result<Vec<u8>, CandidateGenerationEvidenceRepositoryError> {
    if marker.byte_size > MAX_ROOT_MARKER_BYTES {
        return Err(CandidateGenerationEvidenceRepositoryError::InvalidRootMarker);
    }
    let mut file = marker
        .file
        .try_clone()
        .map_err(CandidateGenerationEvidenceRepositoryError::StorageIo)?;
    file.rewind()
        .map_err(CandidateGenerationEvidenceRepositoryError::StorageIo)?;
    let capacity = usize::try_from(marker.byte_size)
        .map_err(|_| CandidateGenerationEvidenceRepositoryError::InvalidRootMarker)?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(MAX_ROOT_MARKER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(CandidateGenerationEvidenceRepositoryError::StorageIo)?;
    if bytes.len() != capacity {
        return Err(CandidateGenerationEvidenceRepositoryError::StorageChanged);
    }
    let after = fingerprint_std_file(&marker.file).map_err(map_active_storage)?;
    if after != marker.fingerprint {
        return Err(CandidateGenerationEvidenceRepositoryError::StorageChanged);
    }
    Ok(bytes)
}

pub(super) fn decode_root_marker_bytes(
    bytes: &[u8],
) -> Result<CandidateGenerationEvidenceStorageRootId, CandidateGenerationEvidenceRepositoryError> {
    if bytes.is_empty()
        || u64::try_from(bytes.len())
            .ok()
            .is_none_or(|length| length > MAX_ROOT_MARKER_BYTES)
    {
        return Err(CandidateGenerationEvidenceRepositoryError::InvalidRootMarker);
    }
    let marker: RootMarkerV1 = serde_json::from_slice(bytes)
        .map_err(|_| CandidateGenerationEvidenceRepositoryError::InvalidRootMarker)?;
    if marker.schema_version != ROOT_MARKER_SCHEMA_VERSION
        || serde_json::to_vec(&marker).ok().as_deref() != Some(bytes)
    {
        return Err(CandidateGenerationEvidenceRepositoryError::InvalidRootMarker);
    }
    let nonce = decode_lower_hex::<ROOT_NONCE_BYTES>(&marker.root_nonce)
        .ok_or(CandidateGenerationEvidenceRepositoryError::InvalidRootMarker)?;
    derive_root_id(&nonce)
}

fn derive_root_id(
    nonce: &[u8; ROOT_NONCE_BYTES],
) -> Result<CandidateGenerationEvidenceStorageRootId, CandidateGenerationEvidenceRepositoryError> {
    let mut material = Vec::with_capacity(ROOT_ID_DOMAIN.len() + nonce.len());
    material.extend_from_slice(ROOT_ID_DOMAIN);
    material.extend_from_slice(nonce);
    CandidateGenerationEvidenceStorageRootId::new(Digest::sha256(&material).as_str())
        .map_err(CandidateGenerationEvidenceRepositoryError::StorageContract)
}

fn encode_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn decode_lower_hex<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut decoded = [0_u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = decode_lower_nibble(pair[0])?
            .checked_mul(16)?
            .checked_add(decode_lower_nibble(pair[1])?)?;
    }
    Some(decoded)
}

const fn decode_lower_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
