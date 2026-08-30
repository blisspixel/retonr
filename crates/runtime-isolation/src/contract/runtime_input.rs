use std::{fmt, fs::File};

use rewrite_types::CancellationToken;
use rewrite_types::Digest;

use super::RedactedDigestBuilder;
use crate::{IsolationError, IsolationResult};

mod evidence;
pub use evidence::ManagedRuntimeInputEvidence;

/// Fixed target-visible root for the first managed runtime-input capability.
pub const MANAGED_RUNTIME_INPUT_ROOT_V1: &str = "/tmp/retonr-managed-runtime-input-v1";
/// Maximum regular-file objects in one managed runtime-input tree.
pub const MAXIMUM_MANAGED_RUNTIME_INPUT_FILES: usize = 64;
/// Maximum aggregate logical bytes in one managed runtime-input tree.
pub const MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES: u64 = 129 * 1024 * 1024 * 1024;

const MAXIMUM_MEMBER_BYTES: u64 = 128 * 1024 * 1024 * 1024;
const MAXIMUM_ALIAS_BYTES: usize = 4_096;
const MAXIMUM_ALIAS_COMPONENTS: usize = 32;
const MAXIMUM_AGGREGATE_ALIAS_BYTES: usize =
    MAXIMUM_MANAGED_RUNTIME_INPUT_FILES * MAXIMUM_ALIAS_BYTES;

/// Source which transfers owned, content-bound members into an isolation-owned sink.
///
/// Implementations can remain opaque and lease-bound. The trait exposes no
/// member accessor and the resulting tree has no raw public constructor.
pub trait RetainedRuntimeInputSource {
    /// Transfers members in strictly increasing canonical alias order.
    ///
    /// # Errors
    ///
    /// Returns a bounded error when the source cannot detach or validate one
    /// of its retained handles.
    fn transfer(self, sink: &mut RetainedRuntimeInputSink<'_>) -> IsolationResult<()>;
}

/// Narrow receiver used only while constructing an opaque retained input tree.
pub struct RetainedRuntimeInputSink<'tree> {
    members: &'tree mut Vec<RetainedRuntimeInputMember>,
    total_bytes: &'tree mut u64,
    alias_bytes: &'tree mut usize,
    cancellation: &'tree CancellationToken,
}

impl RetainedRuntimeInputSink<'_> {
    /// Retains one exact regular file at one canonical target-relative alias.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::RuntimeInputObjectMismatch`] for an invalid,
    /// duplicate, reordered, oversized, indirect, writable, or multiply-linked
    /// object. Errors and debug output never include the alias or contents.
    pub fn retain(
        &mut self,
        relative_alias: impl Into<String>,
        expected_digest: Digest,
        expected_bytes: u64,
        file: File,
    ) -> IsolationResult<()> {
        let relative_alias = relative_alias.into();
        if !valid_alias(&relative_alias)
            || self
                .members
                .last()
                .is_some_and(|member| member.declaration.relative_alias >= relative_alias)
            || self.members.len() == MAXIMUM_MANAGED_RUNTIME_INPUT_FILES
            || expected_bytes == 0
            || expected_bytes > MAXIMUM_MEMBER_BYTES
        {
            return Err(IsolationError::RuntimeInputObjectMismatch);
        }
        let next_total = self
            .total_bytes
            .checked_add(expected_bytes)
            .filter(|total| *total <= MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES)
            .ok_or(IsolationError::RuntimeInputObjectMismatch)?;
        let next_alias_bytes = self
            .alias_bytes
            .checked_add(relative_alias.len())
            .filter(|total| *total <= MAXIMUM_AGGREGATE_ALIAS_BYTES)
            .ok_or(IsolationError::RuntimeInputObjectMismatch)?;
        let identity = validate_file(&file, &expected_digest, expected_bytes, self.cancellation)?;
        self.members.push(RetainedRuntimeInputMember {
            declaration: RetainedRuntimeInputDeclaration {
                relative_alias,
                expected_digest,
                expected_bytes,
                identity,
            },
            file,
        });
        *self.total_bytes = next_total;
        *self.alias_bytes = next_alias_bytes;
        Ok(())
    }
}

/// Opaque, owned retained regular-file map for one managed launch.
pub struct RetainedRuntimeInputTree {
    members: Vec<RetainedRuntimeInputMember>,
    total_bytes: u64,
    layout_digest: Digest,
}

impl RetainedRuntimeInputTree {
    /// Consumes an opaque source and constructs a nonempty canonical tree.
    ///
    /// # Errors
    ///
    /// Returns a bounded error when transfer fails or the source supplies no members.
    pub fn from_source(
        source: impl RetainedRuntimeInputSource,
        cancellation: &CancellationToken,
    ) -> IsolationResult<Self> {
        if cancellation.is_cancelled() {
            return Err(IsolationError::Cancelled);
        }
        let mut members = Vec::new();
        let mut total_bytes = 0_u64;
        let mut alias_bytes = 0_usize;
        source.transfer(&mut RetainedRuntimeInputSink {
            members: &mut members,
            total_bytes: &mut total_bytes,
            alias_bytes: &mut alias_bytes,
            cancellation,
        })?;
        if members.is_empty() {
            return Err(IsolationError::RuntimeInputObjectMismatch);
        }
        let layout_digest =
            runtime_input_layout_digest(members.iter().map(|member| &member.declaration));
        Ok(Self {
            members,
            total_bytes,
            layout_digest,
        })
    }

    /// Returns the exact retained member count without exposing aliases or handles.
    #[must_use]
    pub const fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Returns the aggregate retained logical bytes.
    #[must_use]
    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Returns a redacted digest of ordered aliases, expected content, and objects.
    #[must_use]
    pub const fn redacted_digest(&self) -> &Digest {
        &self.layout_digest
    }

    pub(crate) fn require_nonempty(&self) -> IsolationResult<()> {
        if self.members.is_empty() {
            Err(IsolationError::RuntimeInputObjectMismatch)
        } else {
            Ok(())
        }
    }

    pub(crate) fn launch_digest(&self, launch: &Digest) -> Digest {
        managed_input_launch_digest(launch, &self.layout_digest)
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn into_members(self) -> Vec<RetainedRuntimeInputMember> {
        self.members
    }
}

pub(super) fn managed_input_launch_digest(launch: &Digest, layout: &Digest) -> Digest {
    let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/managed-input-launch/v1");
    digest.push_bytes(launch.as_str().as_bytes());
    digest.push_bytes(layout.as_str().as_bytes());
    digest.finish()
}

#[cfg(test)]
mod digest_tests {
    use rewrite_types::Digest;

    use super::managed_input_launch_digest;

    #[test]
    fn input_bound_launch_digest_is_distinct_and_binds_both_inputs() {
        let plain = Digest::sha256(b"plain closed launch");
        let layout = Digest::sha256(b"private input layout");
        let composite = managed_input_launch_digest(&plain, &layout);
        assert_ne!(composite, plain);
        assert_ne!(
            composite,
            managed_input_launch_digest(&plain, &Digest::sha256(b"other layout"))
        );
        assert_ne!(
            composite,
            managed_input_launch_digest(&Digest::sha256(b"other launch"), &layout)
        );
    }
}

impl RetainedRuntimeInputDeclaration {
    #[cfg(target_os = "linux")]
    pub(crate) fn from_wire(
        relative_alias: String,
        expected_digest: Digest,
        expected_bytes: u64,
        identity: RuntimeInputObjectIdentity,
    ) -> IsolationResult<Self> {
        if !valid_alias(&relative_alias)
            || expected_bytes == 0
            || expected_bytes > MAXIMUM_MEMBER_BYTES
            || identity.device == 0
            || identity.inode == 0
        {
            return Err(IsolationError::RuntimeInputObjectMismatch);
        }
        Ok(Self {
            relative_alias,
            expected_digest,
            expected_bytes,
            identity,
        })
    }
}

impl fmt::Debug for RetainedRuntimeInputTree {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedRuntimeInputTree")
            .field("member_count", &self.members.len())
            .field("total_bytes", &self.total_bytes)
            .field("layout_digest", &self.layout_digest)
            .finish()
    }
}

#[cfg_attr(
    not(target_os = "linux"),
    expect(dead_code, reason = "Linux helper contract")
)]
pub(crate) struct RetainedRuntimeInputMember {
    pub(crate) declaration: RetainedRuntimeInputDeclaration,
    pub(crate) file: File,
}

impl fmt::Debug for RetainedRuntimeInputMember {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedRuntimeInputMember")
            .field("expected_digest", &self.declaration.expected_digest)
            .field("expected_bytes", &self.declaration.expected_bytes)
            .field("identity", &self.declaration.identity)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct RetainedRuntimeInputDeclaration {
    pub(crate) relative_alias: String,
    pub(crate) expected_digest: Digest,
    pub(crate) expected_bytes: u64,
    pub(crate) identity: RuntimeInputObjectIdentity,
}

impl fmt::Debug for RetainedRuntimeInputDeclaration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedRuntimeInputDeclaration")
            .field("expected_digest", &self.expected_digest)
            .field("expected_bytes", &self.expected_bytes)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeInputObjectIdentity {
    pub(crate) device: u64,
    pub(crate) inode: u64,
}

pub(crate) fn runtime_input_layout_digest<'a>(
    declarations: impl IntoIterator<Item = &'a RetainedRuntimeInputDeclaration>,
) -> Digest {
    let declarations = declarations.into_iter().collect::<Vec<_>>();
    let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/managed-input-layout/v1");
    digest.push_bytes(MANAGED_RUNTIME_INPUT_ROOT_V1.as_bytes());
    digest.push_usize(MAXIMUM_MANAGED_RUNTIME_INPUT_FILES);
    digest.push_u64(MAXIMUM_MANAGED_RUNTIME_INPUT_BYTES);
    digest.push_usize(declarations.len());
    for declaration in declarations {
        digest.push_bytes(declaration.relative_alias.as_bytes());
        digest.push_bytes(declaration.expected_digest.as_str().as_bytes());
        digest.push_u64(declaration.expected_bytes);
        digest.push_u64(declaration.identity.device);
        digest.push_u64(declaration.identity.inode);
    }
    digest.finish()
}

fn valid_alias(alias: &str) -> bool {
    if alias.is_empty()
        || alias.len() > MAXIMUM_ALIAS_BYTES
        || alias.starts_with('/')
        || alias.ends_with('/')
        || alias.contains('\\')
        || alias.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return false;
    }
    let components = alias.split('/').collect::<Vec<_>>();
    !components.is_empty()
        && components.len() <= MAXIMUM_ALIAS_COMPONENTS
        && components.iter().all(|component| {
            !component.is_empty()
                && *component != "."
                && *component != ".."
                && component.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                })
        })
}

#[cfg(target_os = "linux")]
fn validate_file(
    file: &File,
    expected_digest: &Digest,
    expected_bytes: u64,
    cancellation: &CancellationToken,
) -> IsolationResult<RuntimeInputObjectIdentity> {
    use std::os::unix::fs::MetadataExt as _;

    use rustix::fs::{OFlags, fcntl_getfl};

    let metadata = file
        .metadata()
        .map_err(|_| IsolationError::RuntimeInputObjectMismatch)?;
    let flags = fcntl_getfl(file).map_err(|_| IsolationError::RuntimeInputObjectMismatch)?;
    if !metadata.is_file()
        || metadata.nlink() > 1
        || metadata.len() != expected_bytes
        || flags.intersects(OFlags::WRONLY | OFlags::RDWR | OFlags::PATH)
        || &runtime_input_file_digest(file, expected_bytes, Some(cancellation))? != expected_digest
    {
        return Err(IsolationError::RuntimeInputObjectMismatch);
    }
    Ok(RuntimeInputObjectIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(not(target_os = "linux"))]
fn validate_file(
    file: &File,
    expected_digest: &Digest,
    expected_bytes: u64,
    cancellation: &CancellationToken,
) -> IsolationResult<RuntimeInputObjectIdentity> {
    let metadata = file
        .metadata()
        .map_err(|_| IsolationError::RuntimeInputObjectMismatch)?;
    if !metadata.is_file()
        || metadata.len() != expected_bytes
        || !digest_matches_on_supported_host(file, expected_digest, expected_bytes, cancellation)?
    {
        return Err(IsolationError::RuntimeInputObjectMismatch);
    }
    Ok(RuntimeInputObjectIdentity {
        device: 0,
        inode: 0,
    })
}

#[cfg(any(all(unix, not(target_os = "linux")), target_os = "windows"))]
fn digest_matches_on_supported_host(
    file: &File,
    expected_digest: &Digest,
    expected_bytes: u64,
    cancellation: &CancellationToken,
) -> IsolationResult<bool> {
    runtime_input_file_digest(file, expected_bytes, Some(cancellation))
        .map(|digest| &digest == expected_digest)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn digest_matches_on_supported_host(
    _file: &File,
    _expected_digest: &Digest,
    _expected_bytes: u64,
    cancellation: &CancellationToken,
) -> IsolationResult<bool> {
    if cancellation.is_cancelled() {
        Err(IsolationError::Cancelled)
    } else {
        Ok(true)
    }
}

#[cfg(any(unix, target_os = "windows"))]
pub(crate) fn runtime_input_file_digest(
    file: &File,
    expected_bytes: u64,
    cancellation: Option<&CancellationToken>,
) -> IsolationResult<Digest> {
    runtime_input_file_digest_inner(file, expected_bytes, cancellation, |_| {})
}

#[cfg(any(unix, target_os = "windows"))]
fn runtime_input_file_digest_inner(
    file: &File,
    expected_bytes: u64,
    cancellation: Option<&CancellationToken>,
    mut after_chunk: impl FnMut(u64),
) -> IsolationResult<Digest> {
    use sha2::{Digest as _, Sha256};

    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    let mut offset = 0_u64;
    while offset < expected_bytes {
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            return Err(IsolationError::Cancelled);
        }
        let remaining = expected_bytes.saturating_sub(offset);
        let requested = buffer
            .len()
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let read = positioned_read(file, &mut buffer[..requested], offset)
            .map_err(|_| IsolationError::RuntimeInputObjectMismatch)?;
        if read == 0 {
            return Err(IsolationError::RuntimeInputObjectMismatch);
        }
        hasher.update(&buffer[..read]);
        offset = offset
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or(IsolationError::RuntimeInputObjectMismatch)?;
        after_chunk(offset);
    }
    if cancellation.is_some_and(CancellationToken::is_cancelled) {
        return Err(IsolationError::Cancelled);
    }
    if positioned_read(file, &mut buffer[..1], expected_bytes)
        .map_err(|_| IsolationError::RuntimeInputObjectMismatch)?
        != 0
    {
        return Err(IsolationError::RuntimeInputObjectMismatch);
    }
    let current_bytes = file
        .metadata()
        .map_err(|_| IsolationError::RuntimeInputObjectMismatch)?
        .len();
    if current_bytes != expected_bytes {
        return Err(IsolationError::RuntimeInputObjectMismatch);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| IsolationError::RuntimeInputObjectMismatch)
}

#[cfg(unix)]
fn positioned_read(file: &File, buffer: &mut [u8], offset: u64) -> std::io::Result<usize> {
    use std::os::unix::fs::FileExt as _;

    file.read_at(buffer, offset)
}

#[cfg(target_os = "windows")]
fn positioned_read(file: &File, buffer: &mut [u8], offset: u64) -> std::io::Result<usize> {
    use std::os::windows::fs::FileExt as _;

    file.seek_read(buffer, offset)
}

#[cfg(test)]
#[path = "runtime_input/tests.rs"]
mod tests;
