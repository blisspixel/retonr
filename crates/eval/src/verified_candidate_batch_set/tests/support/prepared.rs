//! Synthetic portable batches matching one already preregistered test plan.

use std::{cell::RefCell, rc::Rc};

use rewrite_model::{
    EffectivePackageEvidenceV2, GenerationQualificationRequestProjectionV1,
    GenerationSystemRecordV1,
};
use rewrite_model_store::GenerationQualificationPlanFoundationV1Input;

use super::{Scenario, offline_batch};

pub(crate) fn prepared_scenario(
    foundation: GenerationQualificationPlanFoundationV1Input<'_>,
    system: &GenerationSystemRecordV1,
    effective_package: &EffectivePackageEvidenceV2,
    projection: &GenerationQualificationRequestProjectionV1,
) -> Scenario {
    prepared_scenario_with_suffix(
        foundation,
        system,
        effective_package,
        projection,
        "qualification-closure",
    )
}

pub(crate) fn prepared_scenario_with_suffix(
    foundation: GenerationQualificationPlanFoundationV1Input<'_>,
    system: &GenerationSystemRecordV1,
    effective_package: &EffectivePackageEvidenceV2,
    projection: &GenerationQualificationRequestProjectionV1,
    suffix: &str,
) -> Scenario {
    let repetition = &foundation.repetitions[0];
    let log = Rc::new(RefCell::new(Vec::new()));
    let batches = foundation
        .cases
        .iter()
        .enumerate()
        .map(|(index, case)| {
            let cluster = foundation
                .clusters
                .iter()
                .find(|cluster| cluster.cluster_id() == case.cluster_id())
                .expect("case cluster");
            let planned = foundation
                .planned_attempts
                .iter()
                .find(|planned| {
                    planned.case_id() == case.case_id()
                        && planned.repetition_id() == repetition.repetition_id()
                        && planned.generation_system_id() == system.generation_system_id()
                })
                .expect("exact planned attempt");
            let request = projection
                .entries()
                .iter()
                .find(|entry| entry.planned_attempt_id() == planned.planned_attempt_id())
                .expect("projected exact request");
            offline_batch(
                foundation.plan,
                foundation.suite,
                case,
                cluster,
                repetition,
                planned,
                system,
                effective_package,
                index,
                Rc::clone(&log),
                suffix,
                None,
                Some(request.structured_completion_request_binding_id()),
            )
        })
        .collect();
    Scenario {
        input: crate::VerifiedCandidateBatchSetInput {
            qualification_plan: foundation.plan.clone(),
            suite: foundation.suite.clone(),
            repetition: repetition.clone(),
            generation_system: system.clone(),
            selection_policy: foundation.candidate_selection_policy.clone(),
            planned_attempts: foundation.planned_attempts.to_vec(),
        },
        batches,
        log,
    }
}
