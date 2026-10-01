use std::{collections::BTreeMap, path::Path};

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_types::{CancellationToken, Digest};
use thiserror::Error;

use super::{
    RuntimeAdmissionEvidenceBundleError, RuntimeAdmissionEvidenceBundleLimits,
    RuntimeAdmissionEvidenceFoundation, RuntimeAdmissionEvidenceMember,
    RuntimeAdmissionEvidenceTreePlan, RuntimeAdmissionFoundationBindingError,
    VerifiedRuntimeAdmissionFoundationBinding,
};
use crate::RuntimeSourceBuildEvidenceBundleLease;

/// One caller-selected opaque member for an inert evidence assembly.
/// Neither the purpose nor supplied bytes assert a passed admission control.
#[derive(Clone, Copy)]
pub struct RuntimeAdmissionEvidenceAssemblyMember<'a> {
    member: RuntimeAdmissionEvidenceMember,
    bytes: &'a [u8],
}

impl<'a> RuntimeAdmissionEvidenceAssemblyMember<'a> {
    /// Selects exact bytes for one fixed purpose, without interpreting them.
    #[must_use]
    pub const fn new(member: RuntimeAdmissionEvidenceMember, bytes: &'a [u8]) -> Self {
        Self { member, bytes }
    }
}

/// Bounded immutable first-root material that confers no semantic authority.
/// The foundation subjects are verified against retained source-build evidence.
/// All eleven other members remain opaque bytes. This is not an all-pass review,
/// runtime admission, or a production policy entry.
pub struct CompiledRuntimeAdmissionEvidenceAssembly {
    pub(super) foundation: RuntimeAdmissionEvidenceFoundation,
    pub(super) manifest: ArtifactSetManifest,
    pub(super) plan: RuntimeAdmissionEvidenceTreePlan,
    pub(super) bytes: BTreeMap<&'static str, Vec<u8>>,
}

impl std::fmt::Debug for CompiledRuntimeAdmissionEvidenceAssembly {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledRuntimeAdmissionEvidenceAssembly")
            .field("manifest_id", &self.manifest.artifact_set_id())
            .field("foundation_id", self.foundation.foundation_id())
            .finish_non_exhaustive()
    }
}

impl CompiledRuntimeAdmissionEvidenceAssembly {
    /// Returns the exact manifest derived from the retained snapshot bytes.
    #[must_use]
    pub const fn manifest(&self) -> &ArtifactSetManifest {
        &self.manifest
    }
    /// Returns the bounded fixed member plan.
    #[must_use]
    pub const fn tree_plan(&self) -> &RuntimeAdmissionEvidenceTreePlan {
        &self.plan
    }
    /// Returns the inert source-build-bound foundation.
    #[must_use]
    pub const fn foundation(&self) -> &RuntimeAdmissionEvidenceFoundation {
        &self.foundation
    }
}

/// Failure during inert first-root compilation or publication.
#[derive(Error)]
pub enum RuntimeAdmissionEvidenceAssemblyError {
    /// The fixed member purposes, order, or count were not exact.
    #[error("runtime admission evidence assembly members are invalid")]
    InvalidMembers,
    /// The exact source-build subject binding could not be verified.
    #[error(transparent)]
    Foundation(#[from] RuntimeAdmissionFoundationBindingError),
    /// Byte closure, storage, cancellation, or bounds verification failed.
    #[error(transparent)]
    Bundle(#[from] RuntimeAdmissionEvidenceBundleError),
    /// An operation failure was followed by independent staging cleanup failure.
    #[error("runtime admission evidence assembly and cleanup both failed")]
    CleanupAfterFailure {
        /// The original compilation or publication failure.
        operation: Box<Self>,
        /// The independent cleanup failure.
        cleanup: Box<RuntimeAdmissionEvidenceBundleError>,
    },
    /// The primary operation and its mandatory fresh source check both failed.
    #[error("runtime admission evidence assembly and final source verification both failed")]
    FinalizationAfterFailure {
        /// The independent primary operation failure.
        operation: Box<Self>,
        /// The failure from fresh uncancelled source verification.
        finalization: Box<Self>,
    },
    /// Publication committed but independent final verification then failed.
    #[error("runtime admission evidence publication committed but final verification failed")]
    CommittedFinalization {
        /// The independent finalization failure. The root remains inert.
        failure: Box<Self>,
    },
}

impl std::fmt::Debug for RuntimeAdmissionEvidenceAssemblyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let class = match self {
            Self::InvalidMembers => "invalid_members",
            Self::Foundation(_) => "foundation",
            Self::Bundle(_) => "bundle",
            Self::CleanupAfterFailure { .. } => "cleanup_after_failure",
            Self::FinalizationAfterFailure { .. } => "finalization_after_failure",
            Self::CommittedFinalization { .. } => "committed_finalization",
        };
        formatter
            .debug_tuple("RuntimeAdmissionEvidenceAssemblyError")
            .field(&class)
            .finish()
    }
}

/// Compiler for a bounded first-root byte closure without admission authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeAdmissionEvidenceAssemblyCompiler;

impl RuntimeAdmissionEvidenceAssemblyCompiler {
    /// Verifies source-build foundation subjects and snapshots exactly eleven
    /// nonfoundation members in [`RuntimeAdmissionEvidenceMember::ALL`] order.
    /// Caller bytes are opaque and cannot grant passed control or policy authority.
    /// Complete sizes are checked before member bytes are copied.
    ///
    /// # Errors
    /// Returns [`RuntimeAdmissionEvidenceAssemblyError`] for incorrect purposes,
    /// bounds, cancellation, source drift, or subject mismatch.
    pub fn compile(
        foundation: &RuntimeAdmissionEvidenceFoundation,
        source: &RuntimeSourceBuildEvidenceBundleLease,
        members: &[RuntimeAdmissionEvidenceAssemblyMember<'_>],
        limits: RuntimeAdmissionEvidenceBundleLimits,
        cancellation: &CancellationToken,
    ) -> Result<CompiledRuntimeAdmissionEvidenceAssembly, RuntimeAdmissionEvidenceAssemblyError>
    {
        compile_view(foundation, source, members, limits, cancellation)
    }
}

pub(super) trait AssemblySourceEvidence {
    fn verify_foundation(
        &self,
        foundation: &RuntimeAdmissionEvidenceFoundation,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionEvidenceAssemblyError>;
    fn overlaps_path(&self, path: &Path) -> bool;
}

impl AssemblySourceEvidence for RuntimeSourceBuildEvidenceBundleLease {
    fn verify_foundation(
        &self,
        foundation: &RuntimeAdmissionEvidenceFoundation,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionEvidenceAssemblyError> {
        VerifiedRuntimeAdmissionFoundationBinding::verify(foundation, self, cancellation)?;
        Ok(())
    }
    fn overlaps_path(&self, path: &Path) -> bool {
        self.overlaps_evidence_path(path)
    }
}

pub(super) fn compile_view(
    source_foundation: &RuntimeAdmissionEvidenceFoundation,
    source: &impl AssemblySourceEvidence,
    members: &[RuntimeAdmissionEvidenceAssemblyMember<'_>],
    limits: RuntimeAdmissionEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<CompiledRuntimeAdmissionEvidenceAssembly, RuntimeAdmissionEvidenceAssemblyError> {
    let limits = limits
        .validate()
        .map_err(RuntimeAdmissionEvidenceBundleError::from)?;
    super::verify::active(cancellation)?;
    if members.len() != super::RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT - 1 {
        return Err(RuntimeAdmissionEvidenceAssemblyError::InvalidMembers);
    }
    let mut sizes = [0; super::RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT];
    sizes[0] = source_foundation.canonical_bytes().len() as u64;
    for (index, (selected, expected)) in members
        .iter()
        .zip(RuntimeAdmissionEvidenceMember::ALL.into_iter().skip(1))
        .enumerate()
    {
        if selected.member != expected {
            return Err(RuntimeAdmissionEvidenceAssemblyError::InvalidMembers);
        }
        sizes[index + 1] = selected.bytes.len() as u64;
    }
    let plan = RuntimeAdmissionEvidenceTreePlan::compile(sizes, limits)
        .map_err(RuntimeAdmissionEvidenceBundleError::from)?;
    source.verify_foundation(source_foundation, cancellation)?;
    let foundation = RuntimeAdmissionEvidenceFoundation::from_canonical_bytes(
        source_foundation.canonical_bytes(),
    )
    .map_err(RuntimeAdmissionEvidenceBundleError::from)?;
    let mut borrowed_bytes = BTreeMap::from([(
        RuntimeAdmissionEvidenceMember::Foundation.relative_path(),
        foundation.canonical_bytes(),
    )]);
    for selected in members {
        super::verify::active(cancellation)?;
        borrowed_bytes.insert(selected.member.relative_path(), selected.bytes);
    }
    let manifest_members = borrowed_bytes
        .iter()
        .map(|(path, bytes)| {
            ArtifactSetMember::new(
                ArtifactId::from_digest(Digest::sha256(bytes)),
                bytes.len() as u64,
                ArtifactSetRelativePath::new(*path).expect("fixed evidence path is portable"),
            )
        })
        .collect();
    let manifest = ArtifactSetManifest::new(manifest_members)
        .map_err(|_| RuntimeAdmissionEvidenceBundleError::InvalidTree)?;
    let total_bytes = manifest
        .total_byte_size()
        .checked_add(manifest.canonical_json().len() as u64)
        .ok_or(RuntimeAdmissionEvidenceBundleError::LimitExceeded)?;
    if total_bytes > limits.maximum_total_bytes {
        return Err(RuntimeAdmissionEvidenceBundleError::LimitExceeded.into());
    }
    let mut bytes = BTreeMap::new();
    for (path, member_bytes) in borrowed_bytes {
        super::verify::active(cancellation)?;
        bytes.insert(path, member_bytes.to_vec());
    }
    source.verify_foundation(&foundation, cancellation)?;
    super::verify::active(cancellation)?;
    Ok(CompiledRuntimeAdmissionEvidenceAssembly {
        foundation,
        manifest,
        plan,
        bytes,
    })
}

#[cfg(test)]
pub(super) mod tests;
