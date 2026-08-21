use sha2::{Digest as _, Sha256};

use rewrite_types::Digest;

use super::{ExactReader, GgufLimits, digest_from_hasher};
use crate::error::{ReconstructionError, ReconstructionResult};

pub(super) struct ParsedValue {
    pub(super) digest: Digest,
    pub(super) u32_value: Option<u32>,
    pub(super) string_value: Option<Vec<u8>>,
}

pub(super) fn parse_metadata_value<R, C>(
    input: &mut ExactReader<'_, R, C>,
    value_type: u32,
    limits: &GgufLimits,
    total_array_elements: &mut u64,
) -> ReconstructionResult<ParsedValue>
where
    R: std::io::Read,
    C: FnMut() -> bool,
{
    let mut hasher = Sha256::new();
    hasher.update(b"retonr:gguf-metadata-value:v1\0");
    hasher.update(value_type.to_le_bytes());
    let (u32_value, string_value) = if value_type == 9 {
        parse_array(input, limits, total_array_elements, &mut hasher)?;
        (None, None)
    } else {
        parse_scalar(input, value_type, limits, &mut hasher)?
    };
    Ok(ParsedValue {
        digest: digest_from_hasher(hasher)?,
        u32_value,
        string_value,
    })
}

pub(super) fn u32_value_digest(value: u32) -> ReconstructionResult<Digest> {
    let mut hasher = Sha256::new();
    hasher.update(b"retonr:gguf-metadata-value:v1\0");
    hasher.update(4_u32.to_le_bytes());
    hasher.update(value.to_le_bytes());
    digest_from_hasher(hasher)
}

fn parse_array<R, C>(
    input: &mut ExactReader<'_, R, C>,
    limits: &GgufLimits,
    total_array_elements: &mut u64,
    hasher: &mut Sha256,
) -> ReconstructionResult<()>
where
    R: std::io::Read,
    C: FnMut() -> bool,
{
    let element_type = input.read_u32()?;
    if element_type == 9 || element_type > 12 {
        return Err(ReconstructionError::UnsupportedGguf);
    }
    let count = input.read_u64()?;
    if count > limits.array_elements {
        return Err(ReconstructionError::LimitExceeded);
    }
    *total_array_elements = total_array_elements
        .checked_add(count)
        .filter(|value| *value <= limits.total_array_elements)
        .ok_or(ReconstructionError::LimitExceeded)?;
    hasher.update(element_type.to_le_bytes());
    hasher.update(count.to_le_bytes());
    for _ in 0..count {
        let _ = parse_scalar(input, element_type, limits, hasher)?;
    }
    Ok(())
}

fn parse_scalar<R, C>(
    input: &mut ExactReader<'_, R, C>,
    value_type: u32,
    limits: &GgufLimits,
    hasher: &mut Sha256,
) -> ReconstructionResult<(Option<u32>, Option<Vec<u8>>)>
where
    R: std::io::Read,
    C: FnMut() -> bool,
{
    match value_type {
        0 | 1 => hasher.update(input.read_array::<1>()?),
        2 | 3 => hasher.update(input.read_array::<2>()?),
        4 => {
            let bytes = input.read_array::<4>()?;
            hasher.update(bytes);
            return Ok((Some(u32::from_le_bytes(bytes)), None));
        }
        5 | 6 => hasher.update(input.read_array::<4>()?),
        7 => {
            let bytes = input.read_array::<1>()?;
            if bytes[0] > 1 {
                return Err(ReconstructionError::InvalidGguf);
            }
            hasher.update(bytes);
        }
        8 => {
            let length = input.read_u64()?;
            if length > limits.string_bytes {
                return Err(ReconstructionError::LimitExceeded);
            }
            let bytes = input.read_vector(length)?;
            std::str::from_utf8(&bytes).map_err(|_| ReconstructionError::InvalidUtf8)?;
            hasher.update(length.to_le_bytes());
            hasher.update(&bytes);
            return Ok((None, Some(bytes)));
        }
        10..=12 => hasher.update(input.read_array::<8>()?),
        _ => return Err(ReconstructionError::UnsupportedGguf),
    }
    Ok((None, None))
}
