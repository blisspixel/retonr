use std::{fs::File, time::Instant};

use rewrite_model::ArtifactId;
use rewrite_types::{CancellationToken, Digest};
use rustix::fd::OwnedFd;

use super::linux::ensure_pidfd_alive;
use crate::managed_worker::ensure_worker_active;
use crate::{
    ManagedGenerationWorkerError, ManagedGenerationWorkerEvidence, ManagedGenerationWorkerLimits,
    ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerNativeLoadEvidence,
    ManagedGenerationWorkerNativeLoadRequest, ManagedGenerationWorkerObservationRequest,
    ManagedGenerationWorkerProfile, ManagedGenerationWorkerResourceObservation,
};

mod native;
mod process;

use native::{
    ModelMappingBaseline, NativePolicy, build_native_policy, establish_model_mapping,
    native_snapshot, reobserve_model_mapping,
};
use process::{
    EvidenceSubject, NamespaceSet, ObjectIdentity, RetainedProcess, discover_exact_worker,
    matching_descendant_pids, model_path, namespaces, object_identity, observe_retained_worker,
    reobserve_worker, retain_process, status, worker_high_water_resident_bytes,
};

impl super::linux_managed::Lease {
    pub(crate) fn observe_generation_worker(
        &mut self,
        request: &ManagedGenerationWorkerObservationRequest<'_>,
        limits: ManagedGenerationWorkerLimits,
        cancellation: &CancellationToken,
        started: Instant,
        process_evidence_digest: &Digest,
    ) -> Result<Lease, ManagedGenerationWorkerError> {
        Lease::observe(
            self.expected.outer_pid(),
            self.expected.process_start_token(),
            &self.pidfd,
            request,
            limits,
            cancellation,
            started,
            process_evidence_digest,
        )
    }
}

pub(crate) struct Lease {
    server: RetainedProcess,
    server_namespaces: NamespaceSet,
    server_effective_uid: u32,
    worker: RetainedProcess,
    worker_executable: File,
    parent_chain: Vec<RetainedProcess>,
    expected_worker: File,
    expected_weight: File,
    expected_weight_identity: ObjectIdentity,
    expected_model_path: String,
    profile: ManagedGenerationWorkerProfile,
    package_id: rewrite_model::RuntimePackageManifestId,
    worker_artifact_id: ArtifactId,
    model_artifact_id: ArtifactId,
    process_evidence_digest: Digest,
    evidence: ManagedGenerationWorkerEvidence,
    native_policy: Option<NativePolicy>,
    native_evidence: Option<ManagedGenerationWorkerNativeLoadEvidence>,
    model_baseline: Option<ModelMappingBaseline>,
    model_evidence: Option<ManagedGenerationWorkerModelMappingEvidence>,
}

impl Lease {
    #[expect(
        clippy::too_many_arguments,
        reason = "the observer binds every retained server and worker input explicitly"
    )]
    pub(in crate::platform) fn observe(
        server_pid: u32,
        server_start_token: u64,
        server_pidfd: &OwnedFd,
        request: &ManagedGenerationWorkerObservationRequest<'_>,
        limits: ManagedGenerationWorkerLimits,
        cancellation: &CancellationToken,
        started: Instant,
        process_evidence_digest: &Digest,
    ) -> Result<Self, ManagedGenerationWorkerError> {
        ensure_worker_active(cancellation, started, limits)?;
        let server = retain_process(server_pid)?;
        if server.start_token != server_start_token {
            return Err(ManagedGenerationWorkerError::ServerChanged);
        }
        ensure_pidfd_alive(server_pidfd)
            .map_err(|_error| ManagedGenerationWorkerError::ServerChanged)?;
        let server_namespaces = namespaces(server_pid, limits, cancellation, started)?;
        let server_effective_uid = status(server_pid, limits, cancellation, started, false)?;
        let expected_worker = request
            .retained_worker
            .file()
            .try_clone()
            .map_err(|_error| ManagedGenerationWorkerError::InvalidRequest)?;
        let expected_weight = request
            .retained_model_weight
            .file()
            .try_clone()
            .map_err(|_error| ManagedGenerationWorkerError::InvalidRequest)?;
        let expected_weight_identity = object_identity(&expected_weight)?;
        let expected_model_path = model_path(request.retained_model_weight.artifact_id());
        let first = discover_exact_worker(
            &server,
            server_namespaces,
            server_effective_uid,
            &expected_worker,
            &expected_model_path,
            request.profile,
            request,
            limits,
            cancellation,
            started,
            process_evidence_digest,
        )?;
        let second = reobserve_worker(
            &server,
            server_namespaces,
            server_effective_uid,
            &first.process,
            &first.executable,
            &first.parent_chain,
            &expected_worker,
            &expected_model_path,
            request.profile,
            request,
            limits,
            cancellation,
            started,
            process_evidence_digest,
        )?;
        if first.evidence != second {
            return Err(ManagedGenerationWorkerError::ObservationChanged);
        }
        Ok(Self {
            server,
            server_namespaces,
            server_effective_uid,
            worker: first.process,
            worker_executable: first.executable,
            parent_chain: first.parent_chain,
            expected_worker,
            expected_weight,
            expected_weight_identity,
            expected_model_path,
            profile: request.profile,
            package_id: request.expected_package_id.clone(),
            worker_artifact_id: request.retained_worker.artifact_id().clone(),
            model_artifact_id: request.retained_model_weight.artifact_id().clone(),
            process_evidence_digest: process_evidence_digest.clone(),
            evidence: second,
            native_policy: None,
            native_evidence: None,
            model_baseline: None,
            model_evidence: None,
        })
    }

    pub(crate) const fn evidence(&self) -> &ManagedGenerationWorkerEvidence {
        &self.evidence
    }

    pub(crate) fn observe_native_load(
        &mut self,
        request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        limits: ManagedGenerationWorkerLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<ManagedGenerationWorkerNativeLoadEvidence, ManagedGenerationWorkerError> {
        self.reobserve_process(limits, cancellation, started)?;
        let policy = build_native_policy(
            request,
            self.profile,
            &self.expected_worker,
            &self.worker_artifact_id,
            limits,
            cancellation,
            started,
        )?;
        let first = native_snapshot(
            self.worker.pid,
            &self.worker.pidfd,
            self.profile,
            &policy,
            limits,
            cancellation,
            started,
        )?;
        let second = native_snapshot(
            self.worker.pid,
            &self.worker.pidfd,
            self.profile,
            &policy,
            limits,
            cancellation,
            started,
        )?;
        if first != second {
            return Err(ManagedGenerationWorkerError::ObservationChanged);
        }
        self.reobserve_process(limits, cancellation, started)?;
        let component_count = u32::try_from(first.component_count)
            .map_err(|_error| ManagedGenerationWorkerError::ResourceLimit)?;
        let evidence = ManagedGenerationWorkerNativeLoadEvidence::new(
            component_count,
            first.portable_closure_digest,
            first.digest,
        );
        self.native_policy = Some(policy);
        self.native_evidence = Some(evidence.clone());
        Ok(evidence)
    }

    pub(crate) fn observe_model_mapping(
        &mut self,
        limits: ManagedGenerationWorkerLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerError> {
        self.reobserve_process(limits, cancellation, started)?;
        let (baseline, evidence) = establish_model_mapping(
            self.worker.pid,
            &self.worker.pidfd,
            &self.expected_model_path,
            &self.expected_weight,
            self.expected_weight_identity,
            &self.model_artifact_id,
            limits,
            cancellation,
            started,
        )?;
        self.reobserve_process(limits, cancellation, started)?;
        self.model_baseline = Some(baseline);
        self.model_evidence = Some(evidence.clone());
        Ok(evidence)
    }

    pub(crate) fn observe_resource(
        &mut self,
        limits: ManagedGenerationWorkerLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<ManagedGenerationWorkerResourceObservation, ManagedGenerationWorkerError> {
        let high_water_resident_bytes =
            worker_high_water_resident_bytes(&self.worker, limits, cancellation, started)?;
        Ok(ManagedGenerationWorkerResourceObservation::new(
            self.evidence.evidence_digest().clone(),
            high_water_resident_bytes,
        ))
    }

    pub(crate) fn reobserve(
        &mut self,
        limits: ManagedGenerationWorkerLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError> {
        let (Some(policy), Some(native_before), Some(model_baseline), Some(model_before)) = (
            self.native_policy.as_ref(),
            self.native_evidence.as_ref(),
            self.model_baseline.as_ref(),
            self.model_evidence.as_ref(),
        ) else {
            return Err(ManagedGenerationWorkerError::IncompleteObservation);
        };
        let observed = self.reobserve_process(limits, cancellation, started)?;
        let native = native_snapshot(
            self.worker.pid,
            &self.worker.pidfd,
            self.profile,
            policy,
            limits,
            cancellation,
            started,
        )?;
        let native = ManagedGenerationWorkerNativeLoadEvidence::new(
            u32::try_from(native.component_count)
                .map_err(|_error| ManagedGenerationWorkerError::ResourceLimit)?,
            native.portable_closure_digest,
            native.digest,
        );
        let model = reobserve_model_mapping(
            self.worker.pid,
            &self.worker.pidfd,
            &self.expected_model_path,
            &self.expected_weight,
            self.expected_weight_identity,
            &self.model_artifact_id,
            model_baseline,
            limits,
            cancellation,
            started,
        )?;
        let confirmed = self.reobserve_process(limits, cancellation, started)?;
        if &native != native_before || &model != model_before || observed != confirmed {
            return Err(ManagedGenerationWorkerError::ObservationChanged);
        }
        Ok(confirmed)
    }

    fn reobserve_process(
        &self,
        limits: ManagedGenerationWorkerLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError> {
        let request = EvidenceSubject {
            profile: self.profile,
            package_id: &self.package_id,
            worker_artifact_id: &self.worker_artifact_id,
            model_artifact_id: &self.model_artifact_id,
        };
        let observed = observe_retained_worker(
            &self.server,
            self.server_namespaces,
            self.server_effective_uid,
            &self.worker,
            &self.worker_executable,
            &self.parent_chain,
            &self.expected_worker,
            &self.expected_model_path,
            request,
            limits,
            cancellation,
            started,
            &self.process_evidence_digest,
        )?;
        let discovered = matching_descendant_pids(
            &self.server,
            &self.expected_worker,
            limits,
            cancellation,
            started,
        )?;
        if discovered.as_slice() != [self.worker.pid] {
            return Err(ManagedGenerationWorkerError::WorkerCountMismatch);
        }
        if observed != self.evidence {
            return Err(ManagedGenerationWorkerError::ObservationChanged);
        }
        Ok(observed)
    }
}
