use serde::Serialize;

use rewrite_types::Digest;

use super::GenerationQualificationContractError;

pub(super) fn append_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_be_bytes());
}

pub(super) fn append_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_be_bytes());
}

pub(super) fn append_digest(output: &mut Vec<u8>, value: &Digest) {
    output.extend_from_slice(value.as_str().as_bytes());
}

pub(super) fn append_text(output: &mut Vec<u8>, value: &str) {
    append_u64(output, value.len() as u64);
    output.extend_from_slice(value.as_bytes());
}

pub(super) fn append_count(
    output: &mut Vec<u8>,
    count: usize,
) -> Result<(), GenerationQualificationContractError> {
    append_u64(
        output,
        u64::try_from(count).map_err(|_| GenerationQualificationContractError::EncodingOverflow)?,
    );
    Ok(())
}

pub(super) fn validate_canonical_json<T: Serialize>(
    bytes: &[u8],
    value: &T,
) -> Result<(), GenerationQualificationContractError> {
    let canonical = serde_json::to_vec(value)
        .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
    if canonical == bytes {
        Ok(())
    } else {
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    }
}

pub(super) fn valid_machine_key(value: &str, maximum: usize) -> bool {
    if value.is_empty() || value.len() > maximum {
        return false;
    }
    let mut previous_separator = false;
    for byte in value.bytes() {
        let separator = matches!(byte, b'-' | b'_');
        if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || separator)
            || (separator && previous_separator)
        {
            return false;
        }
        previous_separator = separator;
    }
    !matches!(value.as_bytes().first(), Some(b'-' | b'_'))
        && !matches!(value.as_bytes().last(), Some(b'-' | b'_'))
}
