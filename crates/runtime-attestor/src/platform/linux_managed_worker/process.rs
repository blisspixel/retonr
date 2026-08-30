use std::{collections::BTreeSet, fs, fs::File, time::Instant};

use rewrite_model::ArtifactId;
use rewrite_types::{CancellationToken, Digest};

use crate::managed_worker::ensure_worker_active;
use crate::{
    MANAGED_OLLAMA_MODEL_ROOT, ManagedGenerationWorkerError, ManagedGenerationWorkerEvidence,
    ManagedGenerationWorkerLimits, ManagedGenerationWorkerObservationRequest,
    ManagedGenerationWorkerProfile,
};

mod command;
mod procfs;
use command::command_digest;
pub(super) use procfs::{
    NamespaceSet, ObjectIdentity, ObjectKey, RetainedProcess, ensure_alive, namespaces,
    object_identity, open_process_member, process_stat, read_bounded, retain_process, status,
    validate_retained_process, worker_high_water_resident_bytes,
};

pub(super) struct WorkerSnapshot {
    pub(super) evidence: ManagedGenerationWorkerEvidence,
    pub(super) process: RetainedProcess,
    pub(super) executable: File,
    pub(super) parent_chain: Vec<RetainedProcess>,
}

#[derive(Clone, Copy)]
pub(super) struct EvidenceSubject<'a> {
    pub(super) profile: ManagedGenerationWorkerProfile,
    pub(super) package_id: &'a rewrite_model::RuntimePackageManifestId,
    pub(super) worker_artifact_id: &'a ArtifactId,
    pub(super) model_artifact_id: &'a ArtifactId,
}

pub(super) fn model_path(artifact_id: &ArtifactId) -> String {
    format!(
        "{MANAGED_OLLAMA_MODEL_ROOT}/blobs/sha256-{}",
        artifact_id.digest().as_str()
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "discovery binds one exact worker to all server and request facts"
)]
pub(super) fn discover_exact_worker(
    server: &RetainedProcess,
    server_namespaces: NamespaceSet,
    server_effective_uid: u32,
    expected_worker: &File,
    expected_model_path: &str,
    profile: ManagedGenerationWorkerProfile,
    request: &ManagedGenerationWorkerObservationRequest<'_>,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
    process_evidence_digest: &Digest,
) -> Result<WorkerSnapshot, ManagedGenerationWorkerError> {
    let matches = matching_descendant_pids(server, expected_worker, limits, cancellation, started)?;
    let [pid] = matches.as_slice() else {
        return Err(ManagedGenerationWorkerError::WorkerCountMismatch);
    };
    let process = retain_process(*pid)?;
    let executable = open_process_member(*pid, "exe")?;
    let parent_chain = retain_parent_chain(&process, server, limits, cancellation, started)?;
    let evidence = observe_retained_worker(
        server,
        server_namespaces,
        server_effective_uid,
        &process,
        &executable,
        &parent_chain,
        expected_worker,
        expected_model_path,
        EvidenceSubject {
            profile,
            package_id: request.expected_package_id,
            worker_artifact_id: request.retained_worker.artifact_id(),
            model_artifact_id: request.retained_model_weight.artifact_id(),
        },
        limits,
        cancellation,
        started,
        process_evidence_digest,
    )?;
    Ok(WorkerSnapshot {
        evidence,
        process,
        executable,
        parent_chain,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "reobservation binds all retained worker and server facts"
)]
pub(super) fn reobserve_worker(
    server: &RetainedProcess,
    server_namespaces: NamespaceSet,
    server_effective_uid: u32,
    process: &RetainedProcess,
    executable: &File,
    parent_chain: &[RetainedProcess],
    expected_worker: &File,
    expected_model_path: &str,
    profile: ManagedGenerationWorkerProfile,
    request: &ManagedGenerationWorkerObservationRequest<'_>,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
    process_evidence_digest: &Digest,
) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError> {
    let matches = matching_descendant_pids(server, expected_worker, limits, cancellation, started)?;
    if matches.as_slice() != [process.pid] {
        return Err(ManagedGenerationWorkerError::WorkerCountMismatch);
    }
    observe_retained_worker(
        server,
        server_namespaces,
        server_effective_uid,
        process,
        executable,
        parent_chain,
        expected_worker,
        expected_model_path,
        EvidenceSubject {
            profile,
            package_id: request.expected_package_id,
            worker_artifact_id: request.retained_worker.artifact_id(),
            model_artifact_id: request.retained_model_weight.artifact_id(),
        },
        limits,
        cancellation,
        started,
        process_evidence_digest,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the distinct worker record binds every retained process fact"
)]
pub(super) fn observe_retained_worker(
    server: &RetainedProcess,
    server_namespaces: NamespaceSet,
    server_effective_uid: u32,
    worker: &RetainedProcess,
    worker_executable: &File,
    parent_chain: &[RetainedProcess],
    expected_worker: &File,
    expected_model_path: &str,
    subject: EvidenceSubject<'_>,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
    process_evidence_digest: &Digest,
) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError> {
    validate_retained_process(server, ManagedGenerationWorkerError::ServerChanged)?;
    validate_retained_process(worker, ManagedGenerationWorkerError::WorkerChanged)?;
    if object_identity(worker_executable)? != object_identity(expected_worker)?
        || object_identity(&open_process_member(worker.pid, "exe")?)?
            != object_identity(expected_worker)?
    {
        return Err(ManagedGenerationWorkerError::ExecutableMismatch);
    }
    if namespaces(worker.pid, limits, cancellation, started)? != server_namespaces {
        return Err(ManagedGenerationWorkerError::NamespaceMismatch);
    }
    if status(worker.pid, limits, cancellation, started, true)? != server_effective_uid {
        return Err(ManagedGenerationWorkerError::PrivilegeMismatch);
    }
    validate_parent_chain(worker, parent_chain, server, limits, cancellation, started)?;
    let command = command_digest(
        worker.pid,
        expected_model_path,
        subject.profile,
        limits,
        cancellation,
        started,
    )?;
    let process = Digest::sha256(
        format!(
            "managed-worker-process-v1\0{}\0{}",
            worker.pid, worker.start_token
        )
        .as_bytes(),
    );
    let parent = parent_digest(worker, parent_chain, server, process_evidence_digest);
    let namespace = namespace_digest(server_namespaces, server_effective_uid);
    ensure_alive(&worker.pidfd)?;
    Ok(ManagedGenerationWorkerEvidence::new(
        subject.profile,
        subject.package_id.clone(),
        subject.worker_artifact_id.clone(),
        subject.model_artifact_id.clone(),
        process,
        parent,
        namespace,
        command.portable_configuration_digest,
        command.observation_digest,
    ))
}

pub(super) fn matching_descendant_pids(
    server: &RetainedProcess,
    expected_worker: &File,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<Vec<u32>, ManagedGenerationWorkerError> {
    let expected = object_identity(expected_worker)?;
    let entries = fs::read_dir("/proc")
        .map_err(|_error| ManagedGenerationWorkerError::ProcessVisibilityInsufficient)?;
    let mut inspected = 0_usize;
    let mut matches = Vec::new();
    for entry in entries {
        ensure_worker_active(cancellation, started, limits)?;
        let entry =
            entry.map_err(|_error| ManagedGenerationWorkerError::ProcessVisibilityInsufficient)?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        inspected = inspected
            .checked_add(1)
            .ok_or(ManagedGenerationWorkerError::ResourceLimit)?;
        if inspected > limits.maximum_processes {
            return Err(ManagedGenerationWorkerError::ResourceLimit);
        }
        if pid == server.pid {
            continue;
        }
        let candidate = match retain_process(pid) {
            Ok(candidate) => candidate,
            Err(ManagedGenerationWorkerError::WorkerChanged) => continue,
            Err(error) => return Err(error),
        };
        let executable = open_process_member(pid, "exe")?;
        if object_identity(&executable)? != expected {
            continue;
        }
        retain_parent_chain(&candidate, server, limits, cancellation, started)?;
        matches.push(pid);
        if matches.len() > 1 {
            return Err(ManagedGenerationWorkerError::WorkerCountMismatch);
        }
    }
    matches.sort_unstable();
    Ok(matches)
}

fn retain_parent_chain(
    worker: &RetainedProcess,
    server: &RetainedProcess,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<Vec<RetainedProcess>, ManagedGenerationWorkerError> {
    let mut chain = Vec::new();
    let mut seen = BTreeSet::from([worker.pid]);
    let mut current = process_stat(
        worker.pid,
        limits.maximum_metadata_bytes,
        Some(cancellation),
        Some((started, limits)),
    )?
    .parent_pid;
    while current != server.pid {
        ensure_worker_active(cancellation, started, limits)?;
        if current == 0 || !seen.insert(current) || chain.len() >= limits.maximum_parent_depth {
            return Err(ManagedGenerationWorkerError::ParentChainMismatch);
        }
        let parent = retain_process(current)
            .map_err(|_error| ManagedGenerationWorkerError::ParentChainMismatch)?;
        current = process_stat(
            current,
            limits.maximum_metadata_bytes,
            Some(cancellation),
            Some((started, limits)),
        )?
        .parent_pid;
        chain.push(parent);
    }
    if chain.len().saturating_add(1) > limits.maximum_parent_depth {
        return Err(ManagedGenerationWorkerError::ParentChainMismatch);
    }
    validate_retained_process(server, ManagedGenerationWorkerError::ServerChanged)?;
    Ok(chain)
}

fn validate_parent_chain(
    worker: &RetainedProcess,
    chain: &[RetainedProcess],
    server: &RetainedProcess,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<(), ManagedGenerationWorkerError> {
    let mut expected_parent = chain.first().map_or(server.pid, |process| process.pid);
    if process_stat(
        worker.pid,
        limits.maximum_metadata_bytes,
        Some(cancellation),
        Some((started, limits)),
    )?
    .parent_pid
        != expected_parent
    {
        return Err(ManagedGenerationWorkerError::ParentChainMismatch);
    }
    for (index, process) in chain.iter().enumerate() {
        validate_retained_process(process, ManagedGenerationWorkerError::ParentChainMismatch)?;
        expected_parent = chain.get(index + 1).map_or(server.pid, |parent| parent.pid);
        if process_stat(
            process.pid,
            limits.maximum_metadata_bytes,
            Some(cancellation),
            Some((started, limits)),
        )?
        .parent_pid
            != expected_parent
        {
            return Err(ManagedGenerationWorkerError::ParentChainMismatch);
        }
    }
    validate_retained_process(server, ManagedGenerationWorkerError::ServerChanged)
}

fn namespace_digest(namespaces: NamespaceSet, uid: u32) -> Digest {
    Digest::sha256(
        format!(
            "managed-worker-namespaces-v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{uid}",
            namespaces.pid.device,
            namespaces.pid.inode,
            namespaces.user.device,
            namespaces.user.inode,
            namespaces.network.device,
            namespaces.network.inode,
            namespaces.mount.device,
            namespaces.mount.inode
        )
        .as_bytes(),
    )
}

fn parent_digest(
    worker: &RetainedProcess,
    chain: &[RetainedProcess],
    server: &RetainedProcess,
    server_evidence: &Digest,
) -> Digest {
    let mut material = format!(
        "managed-worker-parent-chain-v1\0{}\0{}",
        worker.pid, worker.start_token
    )
    .into_bytes();
    for process in chain {
        material
            .extend_from_slice(format!("\0{}\0{}", process.pid, process.start_token).as_bytes());
    }
    material.extend_from_slice(format!("\0{}\0{}\0", server.pid, server.start_token).as_bytes());
    material.extend_from_slice(server_evidence.as_str().as_bytes());
    Digest::sha256(&material)
}

#[cfg(test)]
mod tests;
