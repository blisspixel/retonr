//! Exact target resource collection through passed repeatability joins.

use rewrite_model::{
    GenerationRepeatabilityResultRecordV1, GenerationRepeatabilityTerminalStageV1,
    GenerationResourceAttemptResultRecordV1, GenerationSystemId,
    MAX_GENERATION_QUALIFICATION_PHASE_ITEMS,
};
use rewrite_types::CancellationToken;

use super::{
    JoinBindingError, ValidationFailures, VerifiedPassedRepeatabilityJoins,
    VerifiedPassedRepeatabilityJoinsError, join_matches_passed_result, valid_result_closure,
    validate_join_view,
};

impl VerifiedPassedRepeatabilityJoins<'_, '_, '_, '_> {
    /// Collects target results inside every retained join authority bracket.
    ///
    /// The order is repetition result order followed by semantic suite order.
    /// This intermediate collector does not prove a complete passed resource
    /// phase because the surrounding join authority can legally retain any
    /// caller-supplied internally consistent subset of the repetition closure.
    pub(crate) fn collect_target_resource_results(
        &mut self,
        target_generation_system_id: &GenerationSystemId,
        baseline_generation_system_id: &GenerationSystemId,
        ordered_results: &[GenerationRepeatabilityResultRecordV1],
        cancellation: &CancellationToken,
    ) -> Result<Vec<GenerationResourceAttemptResultRecordV1>, VerifiedPassedRepeatabilityJoinsError>
    {
        let passed_results = ordered_results
            .iter()
            .filter(|result| {
                result.terminal_stage() == GenerationRepeatabilityTerminalStageV1::Passed
            })
            .collect::<Vec<_>>();
        let stored_matches = self.target_generation_system_id == *target_generation_system_id
            && self.ordered_result_ids.len() == ordered_results.len()
            && self
                .ordered_result_ids
                .iter()
                .zip(ordered_results)
                .all(|(stored, current)| stored == current.repeatability_result_id());
        let mut failures = ValidationFailures {
            invalid_count: ordered_results.len() > MAX_GENERATION_QUALIFICATION_PHASE_ITEMS
                || self.joins.len() > MAX_GENERATION_QUALIFICATION_PHASE_ITEMS
                || self.joins.len() != passed_results.len(),
            static_relationship: !stored_matches
                || target_generation_system_id == baseline_generation_system_id
                || !valid_result_closure(target_generation_system_id, ordered_results),
            ..ValidationFailures::default()
        };
        let force_callback_failure = failures.invalid_count || failures.static_relationship;
        let mut collected = Vec::new();
        for (index, join) in self.joins.iter_mut().enumerate() {
            let expected = passed_results.get(index).copied();
            let bracket = join.with_revalidated_authorities(cancellation, |view| {
                validate_join_view(&view, cancellation).map_err(|_error| JoinBindingError)?;
                let Some(result) = expected else {
                    return Err(JoinBindingError);
                };
                if force_callback_failure
                    || !join_matches_passed_result(result, target_generation_system_id, view.record)
                {
                    return Err(JoinBindingError);
                }
                let target_results = view
                    .eval
                    .target_resource_results(
                        target_generation_system_id,
                        baseline_generation_system_id,
                        result.repetition_id(),
                        cancellation,
                    )
                    .map_err(|_error| JoinBindingError)?;
                Ok(target_results
                    .records()
                    .iter()
                    .map(|record| (*record).clone())
                    .collect::<Vec<_>>())
            });
            match bracket {
                Ok(results) => collected.extend(results),
                Err(error) => failures.record(Err(error)),
            }
        }
        failures.finish(cancellation.is_cancelled())?;
        Ok(collected)
    }
}

#[cfg(test)]
mod tests {
    use rewrite_model::GenerationSystemId;
    use rewrite_types::{CancellationToken, Digest};

    use super::super::{
        VerifiedPassedRepeatabilityJoinsErrorKind, verify_passed_repeatability_joins,
    };

    fn target_id(label: &str) -> GenerationSystemId {
        serde_json::from_str(&format!("\"{}\"", Digest::sha256(label.as_bytes())))
            .expect("transparent generation-system ID")
    }

    #[test]
    fn empty_collection_is_revalidated_and_cancellation_still_wins() {
        let target = target_id("empty resource collection target");
        let baseline = target_id("empty resource collection baseline");
        let mut verified =
            verify_passed_repeatability_joins(&target, &[], Vec::new(), &CancellationToken::new())
                .expect("empty passed-join closure");
        assert!(
            verified
                .collect_target_resource_results(
                    &target,
                    &baseline,
                    &[],
                    &CancellationToken::new(),
                )
                .expect("empty resource collection")
                .is_empty()
        );

        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let error = verified
            .collect_target_resource_results(&target, &baseline, &[], &cancellation)
            .expect_err("cancelled resource collection");
        assert_eq!(
            error.kind(),
            VerifiedPassedRepeatabilityJoinsErrorKind::Cancelled
        );
    }

    #[test]
    fn duplicate_target_and_target_substitution_fail_closed() {
        let target = target_id("resource collection target");
        let baseline = target_id("resource collection baseline");
        let mut verified =
            verify_passed_repeatability_joins(&target, &[], Vec::new(), &CancellationToken::new())
                .expect("empty passed-join closure");
        let duplicate = verified
            .collect_target_resource_results(&target, &target, &[], &CancellationToken::new())
            .expect_err("duplicate target and baseline");
        assert_eq!(
            duplicate.kind(),
            VerifiedPassedRepeatabilityJoinsErrorKind::Relationship
        );

        let foreign = target_id("foreign resource collection target");
        let substitution = verified
            .collect_target_resource_results(&foreign, &baseline, &[], &CancellationToken::new())
            .expect_err("target substitution");
        assert_eq!(
            substitution.kind(),
            VerifiedPassedRepeatabilityJoinsErrorKind::Relationship
        );
    }
}
