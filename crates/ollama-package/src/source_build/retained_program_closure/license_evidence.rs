//! Inert, fail-closed license-material evidence for retained programs.

use std::io::Read;

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::Digest;

use super::{VerifiedCargoSourceClosure, VerifiedRetainedProgramUpstreamClosure};
use crate::source_build::{
    RuntimeSourceBuildInputComponent, RuntimeSourceBuildInputManifest,
    RuntimeSourceBuildInputOpenError, RuntimeSourceBuildInputRole,
    VerifiedRuntimeSourceBuildInputs,
};

mod error;
mod validation;
mod wire;

pub use error::RetainedProgramLicenseEvidenceError;
use validation::{ClosureBindings, verify_inventory};

/// Domain identifier for retained-program license-material evidence review.
pub const RETAINED_PROGRAM_LICENSE_EVIDENCE_PROCEDURE_ID: &str =
    "retonr:runtime-source-build:retained-program-license-evidence";
/// Current retained-program license-material evidence procedure version.
pub const RETAINED_PROGRAM_LICENSE_EVIDENCE_PROCEDURE_VERSION: u32 = 1;

const INVENTORY_PATH: &str = "legal/licenses.json";
const DEFAULT_INVENTORY_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_LOCK_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_SUBJECTS: usize = 16_384;
const DEFAULT_MATERIALS: usize = 65_536;
const DEFAULT_MATERIAL_BYTES: usize = 2 * 1024 * 1024;
const DEFAULT_TOTAL_MATERIAL_BYTES: usize = 12 * 1024 * 1024;
const DEFAULT_STRING_BYTES: usize = 64 * 1024 * 1024;
const READ_BUFFER_BYTES: usize = 64 * 1024;

/// Fixed resource ceilings for one retained-program license-evidence review.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetainedProgramLicenseEvidenceLimits {
    /// Maximum canonical inventory bytes.
    pub maximum_inventory_bytes: usize,
    /// Maximum separately retained `Cargo.lock` bytes.
    pub maximum_lock_bytes: usize,
    /// Maximum exact component and Cargo package subjects.
    pub maximum_subjects: usize,
    /// Maximum retained license, notice, attribution, and exception records.
    pub maximum_materials: usize,
    /// Maximum bytes in one retained legal material.
    pub maximum_material_bytes: usize,
    /// Maximum aggregate retained legal-material bytes.
    pub maximum_total_material_bytes: usize,
    /// Maximum aggregate reviewer-facing identity and expression bytes.
    pub maximum_string_bytes: usize,
}

impl Default for RetainedProgramLicenseEvidenceLimits {
    fn default() -> Self {
        Self {
            maximum_inventory_bytes: DEFAULT_INVENTORY_BYTES,
            maximum_lock_bytes: DEFAULT_LOCK_BYTES,
            maximum_subjects: DEFAULT_SUBJECTS,
            maximum_materials: DEFAULT_MATERIALS,
            maximum_material_bytes: DEFAULT_MATERIAL_BYTES,
            maximum_total_material_bytes: DEFAULT_TOTAL_MATERIAL_BYTES,
            maximum_string_bytes: DEFAULT_STRING_BYTES,
        }
    }
}

impl RetainedProgramLicenseEvidenceLimits {
    /// Rejects zero or relaxed ceilings.
    ///
    /// # Errors
    ///
    /// Returns [`RetainedProgramLicenseEvidenceError::InvalidLimits`] for an
    /// invalid ceiling.
    pub fn validate(self) -> Result<Self, RetainedProgramLicenseEvidenceError> {
        let hard = Self::default();
        if self.maximum_inventory_bytes == 0
            || self.maximum_inventory_bytes > hard.maximum_inventory_bytes
            || self.maximum_lock_bytes == 0
            || self.maximum_lock_bytes > hard.maximum_lock_bytes
            || self.maximum_subjects == 0
            || self.maximum_subjects > hard.maximum_subjects
            || self.maximum_materials == 0
            || self.maximum_materials > hard.maximum_materials
            || self.maximum_material_bytes == 0
            || self.maximum_material_bytes > hard.maximum_material_bytes
            || self.maximum_total_material_bytes == 0
            || self.maximum_total_material_bytes > hard.maximum_total_material_bytes
            || self.maximum_material_bytes > self.maximum_total_material_bytes
            || self.maximum_string_bytes == 0
            || self.maximum_string_bytes > hard.maximum_string_bytes
        {
            Err(RetainedProgramLicenseEvidenceError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Reviewer-visible mechanical facts, without a legal disposition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedProgramLicenseEvidenceReviewerFacts {
    cargo_packages: usize,
    component_subjects: usize,
    material_bytes: usize,
    materials: usize,
}

impl RetainedProgramLicenseEvidenceReviewerFacts {
    /// Returns the exact retained input-component subject count.
    #[must_use]
    pub const fn component_subject_count(&self) -> usize {
        self.component_subjects
    }

    /// Returns the exact Cargo package subject count.
    #[must_use]
    pub const fn cargo_package_subject_count(&self) -> usize {
        self.cargo_packages
    }

    /// Returns the number of retained legal-material records.
    #[must_use]
    pub const fn material_count(&self) -> usize {
        self.materials
    }

    /// Returns aggregate retained legal-material bytes.
    #[must_use]
    pub const fn material_byte_count(&self) -> usize {
        self.material_bytes
    }

    /// Makes the mandatory human adjudication boundary explicit.
    #[must_use]
    pub const fn human_adjudication_required(&self) -> bool {
        true
    }
}

/// Inert identity of exact, bounded retained-program license evidence.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RetainedProgramLicenseEvidenceClosureId(Digest);

impl RetainedProgramLicenseEvidenceClosureId {
    /// Returns the digest defining the closure identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Complete mechanical license-material evidence without legal authority.
///
/// This value does not approve a license, distribution, build, admission, or
/// execution. The explicit runtime-admission license review remains the only
/// license-control pass authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRetainedProgramLicenseEvidenceClosure {
    cargo_source_closure_id: Digest,
    closure_id: RetainedProgramLicenseEvidenceClosureId,
    evidence_digest: Digest,
    facts: RetainedProgramLicenseEvidenceReviewerFacts,
    source_input_set_id: Digest,
    upstream_closure_id: Digest,
}

impl VerifiedRetainedProgramLicenseEvidenceClosure {
    /// Returns the domain-separated evidence-closure identity.
    #[must_use]
    pub const fn closure_id(&self) -> &RetainedProgramLicenseEvidenceClosureId {
        &self.closure_id
    }

    /// Returns the exact source-input artifact-set digest.
    #[must_use]
    pub const fn source_input_set_id(&self) -> &Digest {
        &self.source_input_set_id
    }

    /// Returns the exact strong Cargo source-closure identity.
    #[must_use]
    pub const fn cargo_source_closure_id(&self) -> &Digest {
        &self.cargo_source_closure_id
    }

    /// Returns the exact authenticated upstream-closure identity.
    #[must_use]
    pub const fn upstream_closure_id(&self) -> &Digest {
        &self.upstream_closure_id
    }

    /// Returns the digest of canonical `legal/licenses.json` bytes.
    #[must_use]
    pub const fn evidence_digest(&self) -> &Digest {
        &self.evidence_digest
    }

    /// Returns bounded mechanical facts for later human review.
    #[must_use]
    pub const fn reviewer_facts(&self) -> &RetainedProgramLicenseEvidenceReviewerFacts {
        &self.facts
    }
}

/// Reopens and verifies exact retained license materials against strong source closures.
///
/// Schema 1 is deliberately rejected because it contains only eight unbound
/// declarations. Schema 2 must retain exact legal text bytes for every frozen
/// component and Cargo package, while remaining `pending_review`.
///
/// # Errors
///
/// Returns [`RetainedProgramLicenseEvidenceError`] for incomplete legacy
/// evidence, binding drift, malformed or ambiguous JSON, missing subjects or
/// materials, resource excess, cancellation, or changed component bytes.
pub fn verify_retained_program_license_evidence_closure<R, F, C>(
    inputs: &VerifiedRuntimeSourceBuildInputs,
    cargo: &VerifiedCargoSourceClosure,
    upstream: &VerifiedRetainedProgramUpstreamClosure,
    limits: RetainedProgramLicenseEvidenceLimits,
    mut open_component: F,
    mut cancelled: C,
) -> Result<VerifiedRetainedProgramLicenseEvidenceClosure, RetainedProgramLicenseEvidenceError>
where
    R: Read,
    F: FnMut(&ArtifactSetRelativePath) -> Result<R, RuntimeSourceBuildInputOpenError>,
    C: FnMut() -> bool,
{
    let limits = limits.validate()?;
    let source_set = inputs
        .manifest()
        .artifact_set()
        .artifact_set_id()
        .digest()
        .clone();
    if cargo.source_input_set_id() != &source_set
        || upstream.source_input_set_id().digest() != &source_set
    {
        return Err(RetainedProgramLicenseEvidenceError::ClosureBindingMismatch);
    }
    let evidence = find_role(
        inputs.manifest(),
        RuntimeSourceBuildInputRole::LicenseEvidence,
    )?;
    let lock = find_role(
        inputs.manifest(),
        RuntimeSourceBuildInputRole::CargoLockfile,
    )?;
    if evidence.relative_path().as_str() != INVENTORY_PATH
        || evidence.roles() != [RuntimeSourceBuildInputRole::LicenseEvidence]
        || lock.roles() != [RuntimeSourceBuildInputRole::CargoLockfile]
        || lock.digest() != cargo.cargo_lock_digest()
    {
        return Err(RetainedProgramLicenseEvidenceError::InvalidComponentRoles);
    }
    let evidence_bytes = read_component(
        evidence,
        limits.maximum_inventory_bytes,
        &mut open_component,
        &mut cancelled,
    )?;
    let lock_bytes = read_component(
        lock,
        limits.maximum_lock_bytes,
        &mut open_component,
        &mut cancelled,
    )?;
    let bindings = ClosureBindings {
        cargo: cargo.closure_id().clone(),
        source_set,
        upstream: upstream.closure_id().digest().clone(),
    };
    verify_inventory(
        inputs.manifest(),
        &evidence_bytes,
        &lock_bytes,
        bindings,
        limits,
    )
}

fn find_role(
    manifest: &RuntimeSourceBuildInputManifest,
    role: RuntimeSourceBuildInputRole,
) -> Result<&RuntimeSourceBuildInputComponent, RetainedProgramLicenseEvidenceError> {
    let mut matches = manifest
        .components()
        .iter()
        .filter(|component| component.roles().contains(&role));
    let component = matches
        .next()
        .ok_or(RetainedProgramLicenseEvidenceError::InvalidComponentRoles)?;
    if matches.next().is_some() {
        return Err(RetainedProgramLicenseEvidenceError::InvalidComponentRoles);
    }
    Ok(component)
}

fn read_component<R, F, C>(
    component: &RuntimeSourceBuildInputComponent,
    maximum: usize,
    open_component: &mut F,
    cancelled: &mut C,
) -> Result<Vec<u8>, RetainedProgramLicenseEvidenceError>
where
    R: Read,
    F: FnMut(&ArtifactSetRelativePath) -> Result<R, RuntimeSourceBuildInputOpenError>,
    C: FnMut() -> bool,
{
    let expected = usize::try_from(component.byte_size())
        .ok()
        .filter(|value| *value <= maximum)
        .ok_or(RetainedProgramLicenseEvidenceError::QuotaExceeded)?;
    let mut stream = open_component(component.relative_path())
        .map_err(|_| RetainedProgramLicenseEvidenceError::ComponentUnavailable)?;
    let mut bytes = Vec::with_capacity(expected);
    let mut buffer = vec![0_u8; READ_BUFFER_BYTES];
    loop {
        if cancelled() {
            return Err(RetainedProgramLicenseEvidenceError::Cancelled);
        }
        let read = stream
            .read(&mut buffer)
            .map_err(|_| RetainedProgramLicenseEvidenceError::ComponentRead)?;
        if read == 0 {
            break;
        }
        if bytes.len().saturating_add(read) > expected {
            return Err(RetainedProgramLicenseEvidenceError::ComponentMeasurementMismatch);
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    if bytes.len() != expected || &Digest::sha256(&bytes) != component.digest() {
        return Err(RetainedProgramLicenseEvidenceError::ComponentMeasurementMismatch);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
