use std::fmt;

use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::StructuredCompletionRequestBindingId;

/// Narrow owned request lifetime across one asynchronous transport call.
///
/// This crate-private wrapper intentionally owns only the already-verified
/// structured request. Source, candidate, case-material, rubric, and retained
/// authority access remain in the handoff. Response normalization must reopen the
/// synchronous callback view rather than retaining those bytes across traffic.
///
/// The public handoff's compile-fail contract proves that the retained authority
/// is noncloneable and nonserializable. This private transport value deliberately
/// implements neither trait and exposes only content-free inspection plus one
/// consuming request accessor; its unit coverage exercises that exact API shape.
pub(crate) struct CandidateJudgeTrafficRequest {
    schedule_cursor: usize,
    request: StructuredCompletionRequest,
}

impl CandidateJudgeTrafficRequest {
    pub(super) const fn new(schedule_cursor: usize, request: StructuredCompletionRequest) -> Self {
        Self {
            schedule_cursor,
            request,
        }
    }

    pub(crate) const fn schedule_cursor(&self) -> usize {
        self.schedule_cursor
    }

    pub(crate) fn request_binding_id(&self) -> StructuredCompletionRequestBindingId {
        self.request.structured_request_binding_id()
    }

    pub(crate) const fn request(&self) -> &StructuredCompletionRequest {
        &self.request
    }

    pub(crate) fn into_structured_request(self) -> StructuredCompletionRequest {
        self.request
    }
}

impl fmt::Debug for CandidateJudgeTrafficRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgeTrafficRequest")
            .field("schedule_cursor", &self.schedule_cursor)
            .field("request_binding_id", &self.request_binding_id())
            .finish_non_exhaustive()
    }
}
