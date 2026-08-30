use crate::{
    ManagedOllamaIsolationLease, RuntimePackageLease, VerifiedAdmittedRuntime,
    VerifiedManagedGenerationPath,
};

use super::{ManagedOllamaEffectiveRuntimeState, ManagedOllamaManagedJudgeAttemptFacts};

impl ManagedOllamaEffectiveRuntimeState {
    pub(in crate::effective_runtime_state_observation) fn binds_managed_judge_observations(
        &self,
        facts: &ManagedOllamaManagedJudgeAttemptFacts<'_>,
    ) -> bool {
        self.managed_judge_subject.server_process == *facts.server_process.evidence_digest()
            && self.managed_judge_subject.server_native_load
                == *facts
                    .server_native_load
                    .native_load_observation_id()
                    .digest()
            && facts.server_native_load.process_evidence_digest()
                == facts.server_process.evidence_digest()
            && self.managed_judge_subject.initial_worker == *facts.initial_worker.evidence_digest()
            && self.managed_judge_subject.final_worker == *facts.final_worker.evidence_digest()
            && self.managed_judge_subject.worker_native_load
                == *facts.worker_native_load.observation_digest()
            && self.managed_judge_subject.model_mapping == *facts.model_mapping.observation_digest()
            && self.managed_judge_subject.request == *facts.request
            && self.managed_judge_subject.response == *facts.response
            && self.managed_judge_subject.receipt == *facts.receipt
            && self.managed_judge_subject.preflight == *facts.preflight
            && self.managed_judge_subject.first_response_ordinal == facts.first_response_ordinal
            && self.managed_judge_subject.last_response_ordinal == facts.last_response_ordinal
            && self.managed_judge_subject.first_residency_ordinal == facts.first_residency_ordinal
            && self.managed_judge_subject.last_residency_ordinal == facts.last_residency_ordinal
    }

    /// Tests whether this state retains the exact live subject selected for a
    /// pending effective-package derivation.
    ///
    /// These attempt-local relationship facts never enter the portable runtime
    /// state or effective-package closure digests.
    pub(crate) fn binds_effective_package_inputs(
        &self,
        admitted_runtime: &VerifiedAdmittedRuntime,
        generation_path: &VerifiedManagedGenerationPath,
        runtime_package: &RuntimePackageLease,
        managed_ollama: &ManagedOllamaIsolationLease<'_>,
    ) -> bool {
        let managed_input = managed_ollama.input_evidence();
        let subject = &self.effective_package_subject;
        subject.admitted_runtime_id == *admitted_runtime.admitted_runtime_id().digest()
            && subject.generation_path_id == *generation_path.generation_path_id()
            && subject.runtime_installation_generation
                == runtime_package.installation_key().installation_generation()
            && subject.model_artifact_set_id == *managed_input.artifact_set_id()
            && subject.model_package_manifest_id == *managed_input.model_package_manifest_id()
            && subject.model_installation_generation == managed_input.installation_generation()
            && subject.input_bound_launch_spec_digest
                == *managed_ollama.input_bound_launch_spec_digest()
            && subject.isolation_policy_digest == *managed_ollama.isolation_policy_digest()
            && subject
                .live_subject
                .as_ref()
                .is_some_and(|token| managed_ollama.binds_live_subject(token))
            && subject
                .runtime_package_identity
                .as_ref()
                .is_some_and(|token| runtime_package.binds_identity_token(token))
            && subject
                .model_package_identity
                .as_ref()
                .is_some_and(|token| managed_ollama.binds_model_package_identity_token(token))
            && managed_ollama.binds_exact_runtime_package(runtime_package)
    }

    #[cfg(test)]
    pub(super) fn has_same_effective_package_subject(&self, other: &Self) -> bool {
        let left = &self.effective_package_subject;
        let right = &other.effective_package_subject;
        left.admitted_runtime_id == right.admitted_runtime_id
            && left.generation_path_id == right.generation_path_id
            && left.runtime_installation_generation == right.runtime_installation_generation
            && left.model_artifact_set_id == right.model_artifact_set_id
            && left.model_package_manifest_id == right.model_package_manifest_id
            && left.model_installation_generation == right.model_installation_generation
            && left.input_bound_launch_spec_digest == right.input_bound_launch_spec_digest
            && left.isolation_policy_digest == right.isolation_policy_digest
    }

    pub(super) fn retain_effective_package_live_subject(
        &mut self,
        runtime_package: &RuntimePackageLease,
        managed_ollama: &ManagedOllamaIsolationLease<'_>,
    ) {
        debug_assert!(managed_ollama.binds_exact_runtime_package(runtime_package));
        self.effective_package_subject.live_subject = Some(managed_ollama.live_subject_token());
        self.effective_package_subject.runtime_package_identity =
            Some(managed_ollama.runtime_package_identity_token());
        self.effective_package_subject.model_package_identity =
            Some(managed_ollama.model_package_identity_token());
    }
}
