use rewrite_inference::{
    OperationContext, StructuredCompletionRequest, StructuredCompletionResponse,
};

use super::{
    OllamaObservedSessionError, OllamaRetainedStreamSession, OllamaSessionExecutionReceipt,
    completion,
};
use crate::{
    OllamaGenerateResourceObservation, OllamaResidentResourceObservedCompletion,
    OllamaResidentSessionExecutionReceipt, OllamaResponseObservation, response::malformed_error,
};

const RESIDENT_COMPLETION_RESPONSE_COUNT: usize = 9;
const FIRST_RESIDENCY_RESPONSE_OFFSET: usize = 4;

struct ResidentCompletionOutcome {
    response: StructuredCompletionResponse,
    receipt: OllamaResidentSessionExecutionReceipt,
    resource_observation: Option<OllamaGenerateResourceObservation>,
}

impl<F> OllamaRetainedStreamSession<F> {
    /// Runs one bounded structured completion with exact post-generation
    /// residency observations on the preflighted transport.
    ///
    /// This opt-in profile is admitted only for the reviewed Ollama version and
    /// an idle preflight. It sends an explicit model keep-alive, then requires
    /// two equal singleton `/api/ps` reports around the final runtime, inventory,
    /// and model-detail checks. The result proves only runtime-reported residency.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed session or observation error for every ordinary
    /// completion failure and for an unreviewed runtime, non-idle preflight,
    /// absent, ambiguous, mismatched, or drifting residency. Every error
    /// permanently poisons the session without retry or reconnect.
    pub async fn complete_structured_with_residency<E>(
        &mut self,
        request: StructuredCompletionRequest,
        context: OperationContext<'_>,
    ) -> Result<
        (
            StructuredCompletionResponse,
            OllamaResidentSessionExecutionReceipt,
        ),
        OllamaObservedSessionError<E>,
    >
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        self.complete_structured_with_residency_inner(&request, context)
            .await
    }

    /// Runs one bounded resident structured completion while borrowing the request.
    ///
    /// This has the exact wire, validation, observation, identity, ordinal, and
    /// receipt behavior of [`Self::complete_structured_with_residency`]. It does
    /// not clone or retain the request, so the caller can consume the same request
    /// into a later authority join after this asynchronous exchange returns.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed session or observation error under the same
    /// conditions as [`Self::complete_structured_with_residency`]. Every error
    /// permanently poisons the session without retry or reconnect.
    pub async fn complete_structured_with_residency_borrowed<E>(
        &mut self,
        request: &StructuredCompletionRequest,
        context: OperationContext<'_>,
    ) -> Result<
        (
            StructuredCompletionResponse,
            OllamaResidentSessionExecutionReceipt,
        ),
        OllamaObservedSessionError<E>,
    >
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        self.complete_structured_with_residency_inner(request, context)
            .await
    }

    /// Runs one resident completion with exact provider resource evidence.
    ///
    /// This explicit opt-in path has the same request sequence and callback
    /// ordering as [`Self::complete_structured_with_residency_borrowed`]. The
    /// additional observation records the private response-head checkpoint and
    /// requires all six reviewed Ollama generate metrics. Compatibility paths
    /// that do not request this evidence continue to accept missing metrics.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed session or observation error under the same
    /// conditions as the resident completion path. It also fails closed when a
    /// provider metric is absent or invalid, or when the generate checkpoint is
    /// not the exact expected response ordinal.
    pub async fn complete_structured_with_residency_and_resource_observation_borrowed<E>(
        &mut self,
        request: &StructuredCompletionRequest,
        context: OperationContext<'_>,
    ) -> Result<OllamaResidentResourceObservedCompletion, OllamaObservedSessionError<E>>
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        let outcome = self
            .complete_structured_with_residency_evidence_inner(request, context, true)
            .await?;
        let observation = outcome.resource_observation.ok_or_else(|| {
            OllamaObservedSessionError::Session(malformed_error("resource_observation_unavailable"))
        })?;
        match OllamaResidentResourceObservedCompletion::new(
            outcome.response,
            outcome.receipt,
            observation,
        ) {
            Ok(completion) => Ok(completion),
            Err(_error) => self.fail(malformed_error("resource_observation_binding_mismatch")),
        }
    }

    async fn complete_structured_with_residency_inner<E>(
        &mut self,
        request: &StructuredCompletionRequest,
        context: OperationContext<'_>,
    ) -> Result<
        (
            StructuredCompletionResponse,
            OllamaResidentSessionExecutionReceipt,
        ),
        OllamaObservedSessionError<E>,
    >
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        let outcome = self
            .complete_structured_with_residency_evidence_inner(request, context, false)
            .await?;
        Ok((outcome.response, outcome.receipt))
    }

    async fn complete_structured_with_residency_evidence_inner<E>(
        &mut self,
        request: &StructuredCompletionRequest,
        context: OperationContext<'_>,
        require_resource_observation: bool,
    ) -> Result<ResidentCompletionOutcome, OllamaObservedSessionError<E>>
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        if self.transport.is_none() {
            return self.fail(super::session_closed());
        }
        let Some(preflight) = self.preflight.clone() else {
            return self.fail(crate::response::policy_error("session_preflight_required"));
        };
        let (binding, profile) = match completion::validate_completion_request(
            request,
            &self.bindings,
            self.completion_input_bytes,
        ) {
            Ok(validated) => validated,
            Err(error) => return self.fail(error),
        };
        let binding = binding.clone();
        let completed_before = self
            .completed_responses()
            .ok_or_else(|| OllamaObservedSessionError::Session(super::session_closed()))?;
        let first_ordinal = completed_before.checked_add(1).ok_or_else(|| {
            OllamaObservedSessionError::Session(malformed_error("response_ordinal_overflow"))
        })?;
        let result = if require_resource_observation {
            let session_subject = &self.resource_subject;
            completion::run_completion_with_residency_and_resource_observation(
                self.transport
                    .as_mut()
                    .ok_or_else(|| OllamaObservedSessionError::Session(super::session_closed()))?,
                &mut self.observer,
                &preflight,
                &binding,
                profile,
                request,
                context,
                session_subject,
            )
            .await
            .map(|(response, residency, observation)| (response, residency, Some(observation)))
        } else {
            completion::run_completion_with_residency(
                self.transport
                    .as_mut()
                    .ok_or_else(|| OllamaObservedSessionError::Session(super::session_closed()))?,
                &mut self.observer,
                &preflight,
                &binding,
                profile,
                request,
                context,
            )
            .await
            .map(|(response, residency)| (response, residency, None))
        };
        let (response, residency, resource_observation) = match result {
            Ok(outcome) => outcome,
            Err(error) => return Err(self.poison_observed(error, context)),
        };
        let completed_after = self
            .completed_responses()
            .ok_or_else(|| OllamaObservedSessionError::Session(super::session_closed()))?;
        if completed_before.checked_add(RESIDENT_COMPLETION_RESPONSE_COUNT) != Some(completed_after)
        {
            return self.fail(malformed_error(
                "resident_completion_response_count_mismatch",
            ));
        }
        let execution = match OllamaSessionExecutionReceipt::new(
            &preflight,
            &response,
            first_ordinal,
            completed_after,
        ) {
            Ok(receipt) => receipt,
            Err(error) => return self.fail(error),
        };
        let first_residency_ordinal = first_ordinal
            .checked_add(FIRST_RESIDENCY_RESPONSE_OFFSET)
            .ok_or_else(|| {
                OllamaObservedSessionError::Session(malformed_error("response_ordinal_overflow"))
            })?;
        let receipt = OllamaResidentSessionExecutionReceipt::new(
            execution,
            &residency,
            first_residency_ordinal,
            completed_after,
        );
        Ok(ResidentCompletionOutcome {
            response,
            receipt,
            resource_observation,
        })
    }
}
