use super::*;

impl ManagedOllamaEffectiveRuntimeState {
    pub(in crate::effective_runtime_state_observation) fn managed_judge_batch_test_fixture(
        state: EffectiveRuntimeState,
        binding_digest: Digest,
        facts: &ManagedOllamaManagedJudgeAttemptFacts<'_>,
        admitted_runtime: &VerifiedAdmittedRuntime,
        generation_path: &VerifiedManagedGenerationPath,
        runtime_package: &RuntimePackageLease,
        managed_ollama: &ManagedOllamaIsolationLease<'_>,
    ) -> Self {
        let input = managed_ollama.input_evidence();
        Self {
            state,
            binding_digest,
            effective_package_subject: ManagedOllamaEffectivePackageSubject {
                admitted_runtime_id: admitted_runtime.admitted_runtime_id().digest().clone(),
                generation_path_id: generation_path.generation_path_id().clone(),
                runtime_installation_generation: runtime_package
                    .installation_key()
                    .installation_generation(),
                model_artifact_set_id: input.artifact_set_id().clone(),
                model_package_manifest_id: input.model_package_manifest_id().clone(),
                model_installation_generation: input.installation_generation(),
                input_bound_launch_spec_digest: managed_ollama
                    .input_bound_launch_spec_digest()
                    .clone(),
                isolation_policy_digest: managed_ollama.isolation_policy_digest().clone(),
                live_subject: Some(managed_ollama.live_subject_token()),
                runtime_package_identity: Some(managed_ollama.runtime_package_identity_token()),
                model_package_identity: Some(managed_ollama.model_package_identity_token()),
                retained_session_subject: None,
            },
            managed_judge_subject: ManagedOllamaManagedJudgeSubject {
                server_process: facts.server_process.evidence_digest().clone(),
                server_native_load: facts
                    .server_native_load
                    .native_load_observation_id()
                    .digest()
                    .clone(),
                initial_worker: facts.initial_worker.evidence_digest().clone(),
                final_worker: facts.final_worker.evidence_digest().clone(),
                worker_native_load: facts.worker_native_load.observation_digest().clone(),
                model_mapping: facts.model_mapping.observation_digest().clone(),
                request: facts.request.clone(),
                response: facts.response.clone(),
                receipt: facts.receipt.clone(),
                preflight: facts.preflight.clone(),
                first_response_ordinal: facts.first_response_ordinal,
                last_response_ordinal: facts.last_response_ordinal,
                first_residency_ordinal: facts.first_residency_ordinal,
                last_residency_ordinal: facts.last_residency_ordinal,
            },
        }
    }
}
