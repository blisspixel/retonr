//! Headless latest-operation state for bounded read-only document review.

use crate::{
    DocumentReviewError, DocumentReviewResult,
    document_review_operation::{
        DocumentReviewOperation, DocumentReviewOperationCompletion, DocumentReviewOperationError,
        DocumentReviewOperationId, DocumentReviewOperationRequest,
        DocumentReviewOperationSubmissionFailure,
    },
};

/// Explicit review lifecycle, without invented generation or progress claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentReviewPhase {
    /// No review has been accepted.
    Idle,
    /// An accepted review is pending or running, including cancellation awaiting completion.
    Loading,
    /// Exact shared-service evidence is available for presentation.
    Ready,
    /// The original operation token cancelled the latest review.
    Cancelled,
    /// The latest accepted review failed.
    Failed,
}

/// Factual complete-input disclosure derived from the accepted bounded request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentReviewInputDisclosure {
    /// Complete source byte count, independent of preview clipping.
    pub source_bytes: usize,
    /// Complete candidate byte count; None means no candidate was supplied.
    pub candidate_bytes: Option<usize>,
}

enum Outcome {
    Idle,
    Loading,
    Ready(Box<DocumentReviewResult>),
    Cancelled,
    Failed(DocumentReviewOperationError),
}

/// UI-free reducer tied to one owned operation bridge and its local identities.
///
/// Busy submissions preserve the original request and all prior state. Accepted
/// submissions discard previous evidence immediately. Ready previews and finding
/// evidence remain untrusted, unchanged text that presentation adapters must
/// render safely. This service has no generation or filesystem write authority.
pub struct DocumentReviewState {
    operation: DocumentReviewOperation,
    current: Option<DocumentReviewOperationId>,
    disclosure: Option<DocumentReviewInputDisclosure>,
    cancellation_requested: bool,
    outcome: Outcome,
}

impl DocumentReviewState {
    /// Starts one bounded operation bridge with asynchronous worker cleanup.
    ///
    /// # Errors
    /// Returns the content-redacted bridge constructor failure.
    pub fn new() -> Result<Self, DocumentReviewOperationError> {
        Ok(Self::with_operation(DocumentReviewOperation::new()?))
    }

    fn with_operation(operation: DocumentReviewOperation) -> Self {
        Self {
            operation,
            current: None,
            disclosure: None,
            cancellation_requested: false,
            outcome: Outcome::Idle,
        }
    }

    /// Accepts a review and changes selection disclosure only after acceptance.
    ///
    /// # Errors
    /// Returns the original bounded request on Busy, stopped worker or ID exhaustion.
    pub fn submit(
        &mut self,
        request: DocumentReviewOperationRequest,
    ) -> Result<DocumentReviewOperationId, DocumentReviewOperationSubmissionFailure> {
        let disclosure = DocumentReviewInputDisclosure {
            source_bytes: request.source_bytes(),
            candidate_bytes: request.candidate_bytes(),
        };
        let id = self.operation.submit(request)?;
        self.current = Some(id);
        self.disclosure = Some(disclosure);
        self.cancellation_requested = false;
        self.outcome = Outcome::Loading;
        Ok(id)
    }

    /// Signals the original latest token; Loading remains until its tagged completion.
    /// Calling this on a terminal state leaves its evidence unchanged.
    pub fn cancel(&mut self) {
        if matches!(self.outcome, Outcome::Loading) {
            self.operation.cancel();
            self.cancellation_requested = true;
        }
    }

    /// Polls without waiting and applies only the latest accepted completion.
    /// Returns true only when the state accepts a terminal completion.
    ///
    /// # Errors
    /// Busy leaves the entire state unchanged. A stopped or poisoned mailbox
    /// settles a loading review as Failed, without exposing partial evidence.
    pub fn poll(&mut self) -> Result<bool, DocumentReviewOperationError> {
        match self.operation.poll() {
            Ok(Some(completion)) => Ok(self.apply(completion)),
            Ok(None) => Ok(false),
            Err(DocumentReviewOperationError::Busy) => Err(DocumentReviewOperationError::Busy),
            Err(error) => {
                if matches!(self.outcome, Outcome::Loading) {
                    self.operation.cancel();
                    self.outcome = Outcome::Failed(error);
                    Ok(true)
                } else {
                    Err(error)
                }
            }
        }
    }

    fn apply(&mut self, completion: DocumentReviewOperationCompletion) -> bool {
        if !matches!(self.outcome, Outcome::Loading)
            || self.current != Some(completion.operation_id)
        {
            return false;
        }
        self.outcome = match completion.result {
            Ok(result) => Outcome::Ready(Box::new(result)),
            Err(
                DocumentReviewOperationError::Cancelled
                | DocumentReviewOperationError::Review(DocumentReviewError::Cancelled),
            ) => Outcome::Cancelled,
            Err(error) => Outcome::Failed(error),
        };
        true
    }

    /// Returns the explicit lifecycle phase of the latest accepted review.
    #[must_use]
    pub const fn phase(&self) -> DocumentReviewPhase {
        match &self.outcome {
            Outcome::Idle => DocumentReviewPhase::Idle,
            Outcome::Loading => DocumentReviewPhase::Loading,
            Outcome::Ready(_) => DocumentReviewPhase::Ready,
            Outcome::Cancelled => DocumentReviewPhase::Cancelled,
            Outcome::Failed(_) => DocumentReviewPhase::Failed,
        }
    }

    /// Returns the identity of the latest accepted review, including terminal states.
    #[must_use]
    pub const fn operation_id(&self) -> Option<DocumentReviewOperationId> {
        self.current
    }

    /// Returns factual disclosure for the latest accepted complete inputs.
    #[must_use]
    pub const fn input_disclosure(&self) -> Option<DocumentReviewInputDisclosure> {
        self.disclosure
    }

    /// Reports an explicit cancellation request, without claiming worker termination.
    #[must_use]
    pub const fn cancellation_requested(&self) -> bool {
        self.cancellation_requested
    }

    /// Borrows exact ready evidence with untouched presentation-untrusted text.
    #[must_use]
    pub fn result(&self) -> Option<&DocumentReviewResult> {
        match &self.outcome {
            Outcome::Ready(result) => Some(result),
            _ => None,
        }
    }

    /// Borrows the typed failure of the latest accepted review.
    #[must_use]
    pub const fn error(&self) -> Option<&DocumentReviewOperationError> {
        match &self.outcome {
            Outcome::Failed(error) => Some(error),
            _ => None,
        }
    }
}

impl std::fmt::Debug for DocumentReviewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentReviewState")
            .field("phase", &self.phase())
            .field("operation_id", &self.current)
            .field("input_disclosure", &self.disclosure)
            .field("cancellation_requested", &self.cancellation_requested)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
