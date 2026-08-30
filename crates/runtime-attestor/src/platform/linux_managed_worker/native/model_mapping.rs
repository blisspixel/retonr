use std::{fs::File, time::Instant};

use rewrite_model::ArtifactId;
use rewrite_types::{CancellationToken, Digest};
use rustix::fd::OwnedFd;

use super::{
    file_hash::hash_model_file,
    maps::{open_map_file, read_mappings},
};
use crate::{
    MANAGED_OLLAMA_MODEL_ROOT, ManagedGenerationWorkerError, ManagedGenerationWorkerLimits,
    ManagedGenerationWorkerModelMappingEvidence,
};

use super::super::process::{ObjectIdentity, ensure_alive, object_identity};

pub(in crate::platform::linux_managed_worker) struct ModelMappingBaseline {
    mapped_file: File,
    mapped_identity: ObjectIdentity,
    ranges: Vec<(u64, u64)>,
}

struct MappingSnapshot {
    mapped_file: File,
    mapped_identity: ObjectIdentity,
    ranges: Vec<(u64, u64)>,
}

#[expect(
    clippy::too_many_arguments,
    reason = "mapping establishment binds one retained source, target, process, and policy"
)]
pub(in crate::platform::linux_managed_worker) fn establish_model_mapping(
    pid: u32,
    pidfd: &OwnedFd,
    expected_path: &str,
    retained_source: &File,
    retained_source_identity: ObjectIdentity,
    model_artifact_id: &ArtifactId,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<
    (
        ModelMappingBaseline,
        ManagedGenerationWorkerModelMappingEvidence,
    ),
    ManagedGenerationWorkerError,
> {
    validate_source(
        retained_source,
        retained_source_identity,
        model_artifact_id,
        limits,
        cancellation,
        started,
    )?;
    let first = mapping_snapshot(
        pid,
        pidfd,
        expected_path,
        retained_source_identity.bytes,
        model_artifact_id,
        None,
        limits,
        cancellation,
        started,
    )?;
    let second = mapping_snapshot(
        pid,
        pidfd,
        expected_path,
        retained_source_identity.bytes,
        model_artifact_id,
        Some(first.mapped_identity),
        limits,
        cancellation,
        started,
    )?;
    if first.mapped_identity != second.mapped_identity
        || first.ranges != second.ranges
        || object_identity(&first.mapped_file)? != first.mapped_identity
    {
        return Err(ManagedGenerationWorkerError::ObservationChanged);
    }
    validate_source(
        retained_source,
        retained_source_identity,
        model_artifact_id,
        limits,
        cancellation,
        started,
    )?;
    let evidence = mapping_evidence(
        expected_path,
        model_artifact_id,
        second.mapped_identity,
        &second.ranges,
    )?;
    Ok((
        ModelMappingBaseline {
            mapped_file: second.mapped_file,
            mapped_identity: second.mapped_identity,
            ranges: second.ranges,
        },
        evidence,
    ))
}

#[expect(
    clippy::too_many_arguments,
    reason = "mapping reobservation binds one retained source, target, process, and baseline"
)]
pub(in crate::platform::linux_managed_worker) fn reobserve_model_mapping(
    pid: u32,
    pidfd: &OwnedFd,
    expected_path: &str,
    retained_source: &File,
    retained_source_identity: ObjectIdentity,
    model_artifact_id: &ArtifactId,
    baseline: &ModelMappingBaseline,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerError> {
    validate_source(
        retained_source,
        retained_source_identity,
        model_artifact_id,
        limits,
        cancellation,
        started,
    )?;
    validate_mapped_file(
        &baseline.mapped_file,
        baseline.mapped_identity,
        model_artifact_id,
        limits,
        cancellation,
        started,
    )?;
    let snapshot = mapping_snapshot(
        pid,
        pidfd,
        expected_path,
        retained_source_identity.bytes,
        model_artifact_id,
        Some(baseline.mapped_identity),
        limits,
        cancellation,
        started,
    )?;
    if snapshot.ranges != baseline.ranges {
        return Err(ManagedGenerationWorkerError::ObservationChanged);
    }
    validate_source(
        retained_source,
        retained_source_identity,
        model_artifact_id,
        limits,
        cancellation,
        started,
    )?;
    validate_mapped_file(
        &baseline.mapped_file,
        baseline.mapped_identity,
        model_artifact_id,
        limits,
        cancellation,
        started,
    )?;
    mapping_evidence(
        expected_path,
        model_artifact_id,
        snapshot.mapped_identity,
        &snapshot.ranges,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "one snapshot keeps every exact process, path, content, and ceiling input explicit"
)]
fn mapping_snapshot(
    pid: u32,
    pidfd: &OwnedFd,
    expected_path: &str,
    expected_bytes: u64,
    model_artifact_id: &ArtifactId,
    expected_mapped_identity: Option<ObjectIdentity>,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<MappingSnapshot, ManagedGenerationWorkerError> {
    ensure_alive(pidfd)?;
    let mappings = read_mappings(pid, limits, cancellation, started)?;
    let mut ranges = Vec::new();
    let mut retained = None;
    let mut mapped_identity = None;
    for mapping in mappings {
        let model_like = mapping.path == expected_path
            || mapping
                .path
                .starts_with(&format!("{MANAGED_OLLAMA_MODEL_ROOT}/"))
            || mapping.path.to_ascii_lowercase().ends_with(".gguf");
        if !model_like {
            continue;
        }
        if mapping.path != expected_path {
            return Err(ManagedGenerationWorkerError::ModelMappingMismatch);
        }
        let file = open_map_file(pid, &mapping)?;
        let identity = object_identity(&file)?;
        if identity.key != mapping.key
            || identity.bytes != expected_bytes
            || mapped_identity.is_some_and(|observed| observed != identity)
            || expected_mapped_identity.is_some_and(|expected| expected != identity)
        {
            return Err(ManagedGenerationWorkerError::ModelMappingMismatch);
        }
        mapped_identity = Some(identity);
        retained.get_or_insert(file);
        ranges.push((mapping.start, mapping.end));
    }
    ranges.sort_unstable();
    let mapped_file = retained.ok_or(ManagedGenerationWorkerError::ModelMappingMismatch)?;
    let mapped_identity =
        mapped_identity.ok_or(ManagedGenerationWorkerError::ModelMappingMismatch)?;
    let mapped_artifact = hash_model_file(
        &mapped_file,
        mapped_identity.bytes,
        limits,
        cancellation,
        started,
    )?;
    if mapped_artifact != *model_artifact_id {
        return Err(ManagedGenerationWorkerError::ModelMappingMismatch);
    }
    ensure_alive(pidfd)?;
    Ok(MappingSnapshot {
        mapped_file,
        mapped_identity,
        ranges,
    })
}

fn validate_source(
    source: &File,
    expected_identity: ObjectIdentity,
    expected_artifact: &ArtifactId,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<(), ManagedGenerationWorkerError> {
    if object_identity(source)? != expected_identity
        || hash_model_file(
            source,
            expected_identity.bytes,
            limits,
            cancellation,
            started,
        )? != *expected_artifact
    {
        return Err(ManagedGenerationWorkerError::ObservationChanged);
    }
    Ok(())
}

fn validate_mapped_file(
    mapped: &File,
    expected_identity: ObjectIdentity,
    expected_artifact: &ArtifactId,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<(), ManagedGenerationWorkerError> {
    if object_identity(mapped)? != expected_identity
        || hash_model_file(
            mapped,
            expected_identity.bytes,
            limits,
            cancellation,
            started,
        )? != *expected_artifact
    {
        return Err(ManagedGenerationWorkerError::ObservationChanged);
    }
    Ok(())
}

fn mapping_evidence(
    expected_path: &str,
    model_artifact_id: &ArtifactId,
    mapped_identity: ObjectIdentity,
    ranges: &[(u64, u64)],
) -> Result<ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerError> {
    let count = u32::try_from(ranges.len())
        .map_err(|_error| ManagedGenerationWorkerError::ResourceLimit)?;
    let mut material = format!(
        "managed-worker-private-model-mapping-v1\0{}\0{}\0{}\0{}\0{count}",
        model_artifact_id.digest().as_str(),
        mapped_identity.key.device,
        mapped_identity.key.inode,
        mapped_identity.bytes,
    )
    .into_bytes();
    material.extend_from_slice(Digest::sha256(expected_path.as_bytes()).as_str().as_bytes());
    for (start, end) in ranges {
        material.extend_from_slice(format!("\0{start}\0{end}").as_bytes());
    }
    Ok(ManagedGenerationWorkerModelMappingEvidence::new(
        model_artifact_id.clone(),
        count,
        Digest::sha256(&material),
    ))
}

#[cfg(test)]
mod tests;
