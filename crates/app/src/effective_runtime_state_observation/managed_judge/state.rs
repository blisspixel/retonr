use super::super::ManagedOllamaEffectiveRuntimeState;
use super::super::authority::ManagedOllamaManagedJudgeAttemptFacts;
use super::contract::RetainedManagedJudgeEffectiveState;

pub(super) fn retain_effective_state(
    state: ManagedOllamaEffectiveRuntimeState,
    facts: &ManagedOllamaManagedJudgeAttemptFacts<'_>,
) -> Option<RetainedManagedJudgeEffectiveState> {
    state
        .binds_managed_judge_observations(facts)
        .then(|| RetainedManagedJudgeEffectiveState::Managed(Box::new(state)))
}

#[cfg(test)]
pub(super) trait EffectiveStateObservationSubject {
    fn binds_observations(&self, facts: &ManagedOllamaManagedJudgeAttemptFacts<'_>) -> bool;

    fn into_retained(self) -> RetainedManagedJudgeEffectiveState;
}
