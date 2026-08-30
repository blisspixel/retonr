use super::*;

pub(crate) fn persist_plan_foundation(store: &mut ArtifactStateStore, fixture: &Fixture) {
    store
        .put_artifact_set_manifest(&fixture.system.runtime_set)
        .expect("runtime artifact set");
    store
        .put_runtime_package_manifest(&fixture.system.runtime_package)
        .expect("runtime package");
    store
        .put_runtime_build_identity(&fixture.system.runtime_build)
        .expect("runtime build");
    store
        .put_effective_runtime_state(&fixture.system.runtime_state)
        .expect("runtime state");
    store
        .put_artifact_set_manifest(&fixture.system.model_set)
        .expect("model artifact set");
    store
        .put_model_package_manifest(&fixture.system.model_package)
        .expect("model package");
    for system in &fixture.systems {
        store
            .transact_generation_system_foundation_v1(
                GenerationSystemFoundationV1Input {
                    generation_system: system,
                    relations: fixture.system.relations(),
                },
                |_| Ok::<_, ()>(()),
            )
            .expect("generation-system foundation");
    }
    store
        .transact_generation_qualification_plan_foundation_v1(
            GenerationQualificationPlanFoundationV1Input {
                clusters: &fixture.clusters,
                deterministic_case_contracts: &fixture.deterministic_case_contracts,
                cases: &fixture.cases,
                suite: &fixture.suite,
                repetitions: &fixture.repetitions,
                generation_systems: &fixture.systems,
                planned_attempts: &fixture.attempts,
                candidate_selection_policy: &fixture.selection_policy,
                plan: &fixture.plan,
            },
            |_| Ok::<_, ()>(()),
        )
        .expect("plan foundation");
}
