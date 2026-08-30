use std::io::Read;

use rewrite_model::{ArtifactSetRelativePath, RuntimePackageManifestId};
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use crate::source_build_report::validates_transformed_runtime_binding;
use crate::{
    MemberOpenError, ReconstructedRuntimePackage, RuntimeSourceBuildInputManifest,
    RuntimeSourceBuildInputOpenError, reconstruct_runtime_package_with_limits,
    verify_runtime_source_build_inputs,
};

use super::parse::parse_review;
use super::{
    ReviewEvidence, RuntimePackageReviewDispositionV2, RuntimePackageReviewEvidenceOpenError,
    RuntimePackageReviewV2, RuntimePackageReviewV2Error, RuntimePackageReviewV2Limits,
    VerifiedRuntimePackageReviewV2,
};

const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// Verifies a controlled source-build review and all bytes needed for its claim.
///
/// The evidence opener receives canonical metadata paths. The member opener receives
/// canonical runtime-layout member paths only for an admitted disposition. Neither
/// opener grants filesystem or network authority to this crate.
///
/// # Errors
///
/// Returns [`RuntimePackageReviewV2Error`] for malformed, noncanonical, missing,
/// changed, oversized, inconsistent, or cancelled input.
pub fn verify_runtime_package_review_v2<ER, EF, SR, SF, MR, MF, C>(
    review_bytes: &[u8],
    limits: &RuntimePackageReviewV2Limits,
    mut open_evidence: EF,
    open_source_input: SF,
    open_member: MF,
    mut cancelled: C,
) -> Result<VerifiedRuntimePackageReviewV2, RuntimePackageReviewV2Error>
where
    ER: Read,
    EF: FnMut(&ArtifactSetRelativePath) -> Result<ER, RuntimePackageReviewEvidenceOpenError>,
    SR: Read,
    SF: FnMut(&ArtifactSetRelativePath) -> Result<SR, RuntimeSourceBuildInputOpenError>,
    MR: Read,
    MF: FnMut(&ArtifactSetRelativePath) -> Result<MR, MemberOpenError>,
    C: FnMut() -> bool,
{
    let limits = limits.validate()?;
    let review = parse_review(review_bytes, limits)?;
    let admitted_layout_path = match &review.disposition {
        RuntimePackageReviewDispositionV2::NotAdmitted { .. } => None,
        RuntimePackageReviewDispositionV2::Admitted { runtime_layout, .. } => Some(runtime_layout),
    };
    let mut source_build_inputs_bytes = None;
    let mut runtime_layout_bytes = None;
    for item in &review.evidence {
        if cancelled() {
            return Err(RuntimePackageReviewV2Error::Cancelled);
        }
        let stream = open_evidence(&item.relative_path)
            .map_err(|_| RuntimePackageReviewV2Error::EvidenceUnavailable)?;
        let retain = item.relative_path == review.source_build_inputs_path
            || admitted_layout_path.is_some_and(|path| *path == item.relative_path);
        let bytes = verify_evidence_record(item, stream, retain, &mut cancelled)?;
        if item.relative_path == review.source_build_inputs_path {
            source_build_inputs_bytes = bytes;
        } else if admitted_layout_path.is_some_and(|path| *path == item.relative_path) {
            runtime_layout_bytes = bytes;
        }
    }
    let source_build_inputs_bytes =
        source_build_inputs_bytes.ok_or(RuntimePackageReviewV2Error::InvalidSourceBuildInputs)?;
    let source_build_inputs = verify_runtime_source_build_inputs(
        &source_build_inputs_bytes,
        limits.source_build_inputs,
        open_source_input,
        &mut cancelled,
    )
    .map_err(RuntimePackageReviewV2Error::SourceBuildInputs)?;
    if source_build_inputs
        .manifest()
        .artifact_set()
        .artifact_set_id()
        != review.source_build_inputs_id
    {
        return Err(RuntimePackageReviewV2Error::InvalidSourceBuildInputs);
    }
    let source_build_inputs = source_build_inputs.manifest().clone();

    let RuntimePackageReviewDispositionV2::Admitted {
        layout_digest,
        runtime_package_manifest_id,
        ..
    } = &review.disposition
    else {
        return Ok(VerifiedRuntimePackageReviewV2 {
            review,
            source_build_inputs,
            reconstructed_runtime: None,
        });
    };
    let runtime_layout_bytes =
        runtime_layout_bytes.ok_or(RuntimePackageReviewV2Error::InvalidDisposition)?;
    if &Digest::sha256(&runtime_layout_bytes) != layout_digest {
        return Err(RuntimePackageReviewV2Error::InvalidRuntimeBinding);
    }
    let reconstructed_runtime = reconstruct_runtime_package_with_limits(
        &runtime_layout_bytes,
        &limits.runtime_layout,
        open_member,
        cancelled,
    )
    .map_err(RuntimePackageReviewV2Error::RuntimeReconstruction)?;
    validate_runtime_binding(
        &review,
        &source_build_inputs,
        &reconstructed_runtime,
        runtime_package_manifest_id,
    )?;
    Ok(VerifiedRuntimePackageReviewV2 {
        review,
        source_build_inputs,
        reconstructed_runtime: Some(reconstructed_runtime),
    })
}

fn verify_evidence_record<R, C>(
    item: &ReviewEvidence,
    mut stream: R,
    retain: bool,
    cancelled: &mut C,
) -> Result<Option<Vec<u8>>, RuntimePackageReviewV2Error>
where
    R: Read,
    C: FnMut() -> bool,
{
    let capacity = if retain {
        usize::try_from(item.byte_size).map_err(|_| RuntimePackageReviewV2Error::LimitExceeded)?
    } else {
        0
    };
    let mut retained = retain.then(|| Vec::with_capacity(capacity));
    let mut hasher = Sha256::new();
    let mut remaining = item.byte_size;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        if cancelled() {
            return Err(RuntimePackageReviewV2Error::Cancelled);
        }
        let read = stream
            .read(&mut buffer)
            .map_err(|_| RuntimePackageReviewV2Error::EvidenceRead)?;
        if read == 0 {
            break;
        }
        let read_bytes =
            u64::try_from(read).map_err(|_| RuntimePackageReviewV2Error::EvidenceRead)?;
        if read_bytes > remaining {
            return Err(RuntimePackageReviewV2Error::EvidenceSizeMismatch);
        }
        hasher.update(&buffer[..read]);
        if let Some(bytes) = &mut retained {
            bytes.extend_from_slice(&buffer[..read]);
        }
        remaining -= read_bytes;
    }
    if remaining != 0 {
        return Err(RuntimePackageReviewV2Error::EvidenceSizeMismatch);
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| RuntimePackageReviewV2Error::EvidenceRead)?;
    if digest != item.digest {
        return Err(RuntimePackageReviewV2Error::EvidenceDigestMismatch);
    }
    Ok(retained)
}

fn validate_runtime_binding(
    review: &RuntimePackageReviewV2,
    source_build_inputs: &RuntimeSourceBuildInputManifest,
    reconstructed: &ReconstructedRuntimePackage,
    expected_package_id: &RuntimePackageManifestId,
) -> Result<(), RuntimePackageReviewV2Error> {
    let layout = reconstructed.layout();
    if layout.runtime_family() != review.runtime_family
        || layout.reported_version() != review.reported_version
        || layout.build_revision() != review.build_revision
        || layout.target() != review.target
        || !validates_transformed_runtime_binding(source_build_inputs, reconstructed, None)
        || &reconstructed
            .runtime_package()
            .runtime_package_manifest_id()
            != expected_package_id
    {
        return Err(RuntimePackageReviewV2Error::InvalidRuntimeBinding);
    }
    Ok(())
}
