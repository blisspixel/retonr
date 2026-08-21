use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read};

use sha2::{Digest as _, Sha256};

use rewrite_types::Digest;

use crate::error::{ReconstructionError, ReconstructionResult};

mod value;

use value::{parse_metadata_value, u32_value_digest};

const GGUF_MAGIC: [u8; 4] = *b"GGUF";
const GGUF_VERSION: u32 = 3;
const DEFAULT_ALIGNMENT: u32 = 32;
const MAX_GGUF_BYTES: u64 = 128 * 1024 * 1024 * 1024;

/// Fixed ceilings for GGUF v3 metadata and tensor-table parsing.
///
/// Explicit values may lower the defaults. Values above a default are rejected
/// and cannot relax the crate's absolute hard limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GgufLimits {
    /// Maximum metadata entry count.
    pub metadata_entries: u64,
    /// Maximum tensor entry count.
    pub tensors: u64,
    /// Maximum UTF-8 bytes in one metadata key or tensor name.
    pub name_bytes: u64,
    /// Maximum UTF-8 bytes in one metadata string value.
    pub string_bytes: u64,
    /// Maximum elements in one metadata array.
    pub array_elements: u64,
    /// Maximum scalar values across all metadata arrays.
    pub total_array_elements: u64,
    /// Maximum bytes occupied by the header and structural tables.
    pub table_bytes: u64,
}

impl Default for GgufLimits {
    fn default() -> Self {
        Self {
            metadata_entries: 65_536,
            tensors: 1_000_000,
            name_bytes: 512,
            string_bytes: 16 * 1024 * 1024,
            array_elements: 10_000_000,
            total_array_elements: 50_000_000,
            table_bytes: 1024 * 1024 * 1024,
        }
    }
}

impl GgufLimits {
    pub(crate) fn within_hard_limits(&self) -> bool {
        let hard = Self::default();
        self.metadata_entries != 0
            && self.metadata_entries <= hard.metadata_entries
            && self.tensors != 0
            && self.tensors <= hard.tensors
            && self.name_bytes != 0
            && self.name_bytes <= hard.name_bytes
            && self.string_bytes != 0
            && self.string_bytes <= hard.string_bytes
            && self.array_elements != 0
            && self.array_elements <= hard.array_elements
            && self.total_array_elements != 0
            && self.total_array_elements <= hard.total_array_elements
            && self.table_bytes != 0
            && self.table_bytes <= hard.table_bytes
    }
}

/// Canonical digests of GGUF metadata groups used by the model contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GgufComponentDigests {
    model_configuration: Digest,
    tokenizer: Digest,
    prompt_template: Digest,
}

impl GgufComponentDigests {
    /// Returns the canonical selected model-load configuration digest.
    #[must_use]
    pub const fn model_configuration(&self) -> &Digest {
        &self.model_configuration
    }

    /// Returns the digest of tokenizer metadata excluding the chat template.
    #[must_use]
    pub const fn tokenizer(&self) -> &Digest {
        &self.tokenizer
    }

    /// Returns the SHA-256 digest of the exact embedded chat-template UTF-8 bytes.
    #[must_use]
    pub const fn prompt_template(&self) -> &Digest {
        &self.prompt_template
    }
}

/// Exact byte identity and bounded structural facts observed from one GGUF v3 blob.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GgufObservation {
    byte_digest: Digest,
    byte_size: u64,
    metadata_count: u64,
    tensor_count: u64,
    component_digests: GgufComponentDigests,
}

impl GgufObservation {
    /// Returns the digest of every byte in the declared blob.
    #[must_use]
    pub const fn byte_digest(&self) -> &Digest {
        &self.byte_digest
    }

    /// Returns the exact declared and observed byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the validated metadata entry count.
    #[must_use]
    pub const fn metadata_count(&self) -> u64 {
        self.metadata_count
    }

    /// Returns the validated tensor entry count.
    #[must_use]
    pub const fn tensor_count(&self) -> u64 {
        self.tensor_count
    }

    /// Returns canonical foundational component digests.
    #[must_use]
    pub const fn component_digests(&self) -> &GgufComponentDigests {
        &self.component_digests
    }
}

/// Streams and structurally validates one exact GGUF v3 blob.
///
/// Metadata values and the tensor table are parsed under fixed count and byte
/// budgets. Tensor payload bytes are hashed and length-checked but not decoded.
///
/// # Errors
///
/// Returns [`ReconstructionError`] for cancellation, I/O failure, digest or
/// length drift, invalid UTF-8, duplicates, unsupported types, or exceeded limits.
pub fn inspect_gguf_v3<R, C>(
    reader: &mut R,
    expected_digest: &Digest,
    declared_size: u64,
    limits: &GgufLimits,
    cancelled: &mut C,
) -> ReconstructionResult<GgufObservation>
where
    R: Read,
    C: FnMut() -> bool,
{
    if declared_size == 0 || declared_size > MAX_GGUF_BYTES || !limits.within_hard_limits() {
        return Err(ReconstructionError::LimitExceeded);
    }
    let mut input = ExactReader::new(reader, declared_size, cancelled);
    if input.read_array::<4>()? != GGUF_MAGIC || input.read_u32()? != GGUF_VERSION {
        return Err(ReconstructionError::UnsupportedGguf);
    }
    let tensor_count = input.read_u64()?;
    let metadata_count = input.read_u64()?;
    if tensor_count == 0
        || tensor_count > limits.tensors
        || metadata_count == 0
        || metadata_count > limits.metadata_entries
    {
        return Err(ReconstructionError::LimitExceeded);
    }

    let metadata = parse_metadata(&mut input, metadata_count, limits)?;
    let configuration = metadata.configuration;
    let tokenizer = metadata.tokenizer;
    let prompt = metadata.prompt;
    let alignment = metadata.alignment;
    if configuration.is_empty() || tokenizer.is_empty() {
        return Err(ReconstructionError::UnsupportedGguf);
    }
    if !(1..=4096).contains(&alignment) || !alignment.is_power_of_two() {
        return Err(ReconstructionError::UnsupportedGguf);
    }

    let offsets = parse_tensors(&mut input, tensor_count, limits)?;

    let table_end = input.position();
    let data_start = align_up(table_end, u64::from(alignment))?;
    if data_start > declared_size {
        return Err(ReconstructionError::InvalidGguf);
    }
    let padding = data_start - table_end;
    if input.read_vector(padding)?.iter().any(|byte| *byte != 0) {
        return Err(ReconstructionError::InvalidGguf);
    }
    let data_bytes = declared_size - data_start;
    if data_bytes == 0
        || offsets.windows(2).any(|pair| pair[0] >= pair[1])
        || offsets
            .iter()
            .any(|offset| *offset >= data_bytes || *offset % u64::from(alignment) != 0)
    {
        return Err(ReconstructionError::InvalidGguf);
    }

    let byte_digest = input.finish()?;
    if &byte_digest != expected_digest {
        return Err(ReconstructionError::BlobDigestMismatch);
    }
    Ok(GgufObservation {
        byte_digest,
        byte_size: declared_size,
        metadata_count,
        tensor_count,
        component_digests: GgufComponentDigests {
            model_configuration: component_digest("model-load-configuration", &configuration)?,
            tokenizer: component_digest("tokenizer-without-chat-template", &tokenizer)?,
            prompt_template: prompt,
        },
    })
}

struct ParsedMetadata {
    configuration: BTreeMap<String, Digest>,
    tokenizer: BTreeMap<String, Digest>,
    prompt: Digest,
    alignment: u32,
}

fn parse_metadata<R, C>(
    input: &mut ExactReader<'_, R, C>,
    count: u64,
    limits: &GgufLimits,
) -> ReconstructionResult<ParsedMetadata>
where
    R: Read,
    C: FnMut() -> bool,
{
    let mut values = BTreeMap::new();
    let mut architecture = None;
    let mut prompt = None;
    let mut alignment = DEFAULT_ALIGNMENT;
    let mut array_elements = 0u64;
    for _ in 0..count {
        let key = input.read_utf8(limits.name_bytes)?;
        if !valid_metadata_key(&key) {
            return Err(ReconstructionError::InvalidGguf);
        }
        let value_type = input.read_u32()?;
        if key == "tokenizer.chat_template" && value_type != 8 {
            return Err(ReconstructionError::UnsupportedGguf);
        }
        let parsed = parse_metadata_value(input, value_type, limits, &mut array_elements)?;
        if key == "general.alignment" {
            alignment = parsed
                .u32_value
                .ok_or(ReconstructionError::UnsupportedGguf)?;
        }
        if key == "general.architecture" {
            let bytes = parsed
                .string_value
                .as_deref()
                .ok_or(ReconstructionError::UnsupportedGguf)?;
            let value = std::str::from_utf8(bytes)
                .map_err(|_| ReconstructionError::InvalidUtf8)?
                .to_owned();
            if !valid_architecture(&value) {
                return Err(ReconstructionError::UnsupportedGguf);
            }
            architecture = Some(value);
        }
        if key == "tokenizer.chat_template" {
            let bytes = parsed
                .string_value
                .as_deref()
                .ok_or(ReconstructionError::UnsupportedGguf)?;
            prompt = Some(Digest::sha256(bytes));
        }
        if values.insert(key, parsed.digest).is_some() {
            return Err(ReconstructionError::DuplicateName);
        }
        input.enforce_position(limits.table_bytes)?;
    }
    let architecture = architecture.ok_or(ReconstructionError::UnsupportedGguf)?;
    let prompt = prompt.ok_or(ReconstructionError::UnsupportedGguf)?;
    let mut configuration = BTreeMap::new();
    for key in [
        "general.architecture",
        "general.file_type",
        "general.quantization_version",
        "general.parameter_count",
    ] {
        let digest = values
            .get(key)
            .ok_or(ReconstructionError::UnsupportedGguf)?;
        configuration.insert(key.to_owned(), digest.clone());
    }
    let alignment_digest = values
        .get("general.alignment")
        .cloned()
        .map_or_else(|| u32_value_digest(DEFAULT_ALIGNMENT), Ok)?;
    configuration.insert("general.alignment".to_owned(), alignment_digest);
    let architecture_prefix = format!("{architecture}.");
    for (key, digest) in &values {
        if key.starts_with(&architecture_prefix) {
            configuration.insert(key.clone(), digest.clone());
        }
    }
    let tokenizer = values
        .into_iter()
        .filter(|(key, _digest)| key.starts_with("tokenizer.") && key != "tokenizer.chat_template")
        .collect();
    Ok(ParsedMetadata {
        configuration,
        tokenizer,
        prompt,
        alignment,
    })
}

fn parse_tensors<R, C>(
    input: &mut ExactReader<'_, R, C>,
    count: u64,
    limits: &GgufLimits,
) -> ReconstructionResult<Vec<u64>>
where
    R: Read,
    C: FnMut() -> bool,
{
    let mut names = BTreeSet::new();
    let mut offsets =
        Vec::with_capacity(usize::try_from(count).map_err(|_| ReconstructionError::LimitExceeded)?);
    for _ in 0..count {
        let name = input.read_utf8(limits.name_bytes)?;
        if name.is_empty() || !names.insert(name) {
            return Err(ReconstructionError::DuplicateName);
        }
        let dimensions = input.read_u32()?;
        if !(1..=4).contains(&dimensions) {
            return Err(ReconstructionError::UnsupportedGguf);
        }
        let mut element_count = 1u64;
        for _ in 0..dimensions {
            element_count = element_count
                .checked_mul(input.read_u64()?)
                .filter(|value| *value != 0)
                .ok_or(ReconstructionError::InvalidGguf)?;
        }
        if !supported_tensor_type(input.read_u32()?) {
            return Err(ReconstructionError::UnsupportedGguf);
        }
        offsets.push(input.read_u64()?);
        input.enforce_position(limits.table_bytes)?;
        let _ = element_count;
    }
    Ok(offsets)
}

fn component_digest(
    selector: &str,
    values: &BTreeMap<String, Digest>,
) -> ReconstructionResult<Digest> {
    let mut hasher = Sha256::new();
    hasher.update(b"retonr:gguf-metadata-selection:v1\0");
    hasher.update(
        u64::try_from(selector.len())
            .map_err(|_| ReconstructionError::LimitExceeded)?
            .to_be_bytes(),
    );
    hasher.update(selector.as_bytes());
    hasher.update(
        u64::try_from(values.len())
            .map_err(|_| ReconstructionError::LimitExceeded)?
            .to_be_bytes(),
    );
    for (key, digest) in values {
        hasher.update(
            u64::try_from(key.len())
                .map_err(|_| ReconstructionError::LimitExceeded)?
                .to_be_bytes(),
        );
        hasher.update(key.as_bytes());
        hasher.update(digest.as_str().as_bytes());
    }
    digest_from_hasher(hasher)
}

fn valid_architecture(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

pub(super) fn digest_from_hasher(hasher: Sha256) -> ReconstructionResult<Digest> {
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| ReconstructionError::InvalidGguf)
}

fn valid_metadata_key(value: &str) -> bool {
    !value.is_empty()
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

const fn supported_tensor_type(value: u32) -> bool {
    matches!(value, 0..=3 | 6..=39)
}

fn align_up(value: u64, alignment: u64) -> ReconstructionResult<u64> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum & !(alignment - 1))
        .ok_or(ReconstructionError::InvalidGguf)
}

pub(super) struct ExactReader<'a, R, C> {
    inner: &'a mut R,
    remaining: u64,
    position: u64,
    hasher: Sha256,
    cancelled: &'a mut C,
}

impl<'a, R, C> ExactReader<'a, R, C>
where
    R: Read,
    C: FnMut() -> bool,
{
    fn new(inner: &'a mut R, size: u64, cancelled: &'a mut C) -> Self {
        Self {
            inner,
            remaining: size,
            position: 0,
            hasher: Sha256::new(),
            cancelled,
        }
    }

    pub(super) fn read_array<const N: usize>(&mut self) -> ReconstructionResult<[u8; N]> {
        let mut bytes = [0; N];
        self.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    pub(super) fn read_u32(&mut self) -> ReconstructionResult<u32> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }

    pub(super) fn read_u64(&mut self) -> ReconstructionResult<u64> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }

    pub(super) fn read_vector(&mut self, length: u64) -> ReconstructionResult<Vec<u8>> {
        let length = usize::try_from(length).map_err(|_| ReconstructionError::LimitExceeded)?;
        let mut bytes = vec![0; length];
        self.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    pub(super) fn read_utf8(&mut self, maximum: u64) -> ReconstructionResult<String> {
        let length = self.read_u64()?;
        if length > maximum {
            return Err(ReconstructionError::LimitExceeded);
        }
        String::from_utf8(self.read_vector(length)?).map_err(|_| ReconstructionError::InvalidUtf8)
    }

    fn read_exact(&mut self, output: &mut [u8]) -> ReconstructionResult<()> {
        if (self.cancelled)() {
            return Err(ReconstructionError::Cancelled);
        }
        let length = u64::try_from(output.len()).map_err(|_| ReconstructionError::LimitExceeded)?;
        if length > self.remaining {
            return Err(ReconstructionError::BlobSizeMismatch);
        }
        self.inner
            .read_exact(output)
            .map_err(|error| map_read_error(&error))?;
        self.hasher.update(output);
        self.remaining -= length;
        self.position += length;
        Ok(())
    }

    fn enforce_position(&self, maximum: u64) -> ReconstructionResult<()> {
        if self.position > maximum {
            Err(ReconstructionError::LimitExceeded)
        } else {
            Ok(())
        }
    }

    const fn position(&self) -> u64 {
        self.position
    }

    fn finish(mut self) -> ReconstructionResult<Digest> {
        let mut buffer = vec![0; 64 * 1024];
        while self.remaining != 0 {
            let amount = usize::try_from(self.remaining.min(buffer.len() as u64))
                .map_err(|_| ReconstructionError::LimitExceeded)?;
            self.read_exact(&mut buffer[..amount])?;
        }
        if (self.cancelled)() {
            return Err(ReconstructionError::Cancelled);
        }
        let mut trailing = [0; 1];
        match self.inner.read(&mut trailing) {
            Ok(0) => digest_from_hasher(self.hasher),
            Ok(_) => Err(ReconstructionError::BlobSizeMismatch),
            Err(_) => Err(ReconstructionError::InputRead),
        }
    }
}

fn map_read_error(error: &io::Error) -> ReconstructionError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        ReconstructionError::BlobSizeMismatch
    } else {
        ReconstructionError::InputRead
    }
}
