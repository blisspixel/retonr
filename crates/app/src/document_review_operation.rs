//! Bounded background document review for event-driven presentation layers.

use crate::{DocumentReviewRequest, DocumentReviewService};
use rewrite_types::CancellationToken;
use std::sync::{
    Arc, Condvar, Mutex, TryLockError,
    atomic::{AtomicBool, Ordering},
};

mod contract;
mod worker;
pub use contract::{
    DocumentReviewOperationCompletion, DocumentReviewOperationError, DocumentReviewOperationId,
    DocumentReviewOperationRequest, DocumentReviewOperationSubmissionFailure,
};

struct Job {
    id: DocumentReviewOperationId,
    request: DocumentReviewOperationRequest,
    cancellation: CancellationToken,
}

#[derive(Default)]
struct Mailbox {
    latest: Option<DocumentReviewOperationId>,
    pending: Option<Job>,
    completed: Option<DocumentReviewOperationCompletion>,
}

#[derive(Default)]
struct Shared {
    mailbox: Mutex<Mailbox>,
    wake: Condvar,
    shutdown: AtomicBool,
    stopped: AtomicBool,
}

/// One bounded read-only review worker and its asynchronous cleanup supervisor.
///
/// At most one request runs, one latest request waits, and one completion is
/// retained. Submitting replaces the pending request and cancels the original
/// token of the preceding operation. Results are returned only for the latest
/// accepted operation. Inputs and result previews remain untrusted text.
pub struct DocumentReviewOperation {
    shared: Arc<Shared>,
    sequence: u64,
    latest: Option<(DocumentReviewOperationId, CancellationToken)>,
}

impl DocumentReviewOperation {
    /// Starts a shared-service worker with joining confined to its supervisor.
    ///
    /// # Errors
    /// Returns `WorkerUnavailable` on thread creation failure. Later failures appear in poll.
    pub fn new() -> Result<Self, DocumentReviewOperationError> {
        Self::start(DocumentReviewService::review)
    }

    fn start(
        review: impl Fn(
            DocumentReviewRequest,
            &CancellationToken,
        ) -> Result<crate::DocumentReviewResult, crate::DocumentReviewError>
        + Send
        + 'static,
    ) -> Result<Self, DocumentReviewOperationError> {
        let shared = Arc::new(Shared::default());
        worker::start(Arc::clone(&shared), review)
            .map_err(|_| DocumentReviewOperationError::WorkerUnavailable)?;
        Ok(Self {
            shared,
            sequence: 0,
            latest: None,
        })
    }

    /// Accepts the latest request and cancels the previous operation's token.
    ///
    /// This never waits for worker work or mailbox ownership. Retrying after a
    /// Busy response returns the original owned request; no request was accepted.
    ///
    /// # Errors
    /// Returns Busy, Stopped, or identifier exhaustion without accepting work.
    pub fn submit(
        &mut self,
        request: DocumentReviewOperationRequest,
    ) -> Result<DocumentReviewOperationId, DocumentReviewOperationSubmissionFailure> {
        let Some(sequence) = self.sequence.checked_add(1) else {
            return Err(DocumentReviewOperationSubmissionFailure {
                request,
                error: DocumentReviewOperationError::IdentifierExhausted,
            });
        };
        let id = DocumentReviewOperationId(sequence);
        let mut mailbox = match self.mailbox() {
            Ok(mailbox) => mailbox,
            Err(error) => return Err(DocumentReviewOperationSubmissionFailure { request, error }),
        };
        if self.shared.shutdown.load(Ordering::Acquire)
            || self.shared.stopped.load(Ordering::Acquire)
        {
            return Err(DocumentReviewOperationSubmissionFailure {
                request,
                error: DocumentReviewOperationError::Stopped,
            });
        }
        if let Some((_, cancellation)) = &self.latest {
            cancellation.cancel();
        }
        let cancellation = CancellationToken::new();
        mailbox.latest = Some(id);
        mailbox.pending = Some(Job {
            id,
            request,
            cancellation: cancellation.clone(),
        });
        mailbox.completed = None;
        drop(mailbox);
        self.sequence = id.0;
        self.latest = Some((id, cancellation));
        self.shared.wake.notify_one();
        Ok(id)
    }

    /// Cancels the original latest-operation token without waiting for the worker.
    /// A subsequent poll reports cancellation and discards any late result.
    pub fn cancel(&self) {
        if let Some((_, cancellation)) = &self.latest {
            cancellation.cancel();
        }
        self.shared.wake.notify_one();
    }

    /// Returns a latest tagged completion without waiting or accepting stale work.
    ///
    /// # Errors
    /// Returns Busy on mailbox contention or Stopped after mailbox poisoning.
    pub fn poll(
        &mut self,
    ) -> Result<Option<DocumentReviewOperationCompletion>, DocumentReviewOperationError> {
        let Some((id, cancellation)) = &self.latest else {
            return Ok(None);
        };
        let id = *id;
        let cancelled = cancellation.is_cancelled();
        let mut mailbox = self.mailbox()?;
        let completion = if cancelled {
            mailbox.pending = None;
            mailbox.completed = None;
            Some(DocumentReviewOperationCompletion {
                operation_id: id,
                result: Err(DocumentReviewOperationError::Cancelled),
            })
        } else {
            let completed = mailbox
                .completed
                .take()
                .filter(|completed| completed.operation_id == id);
            completed.or_else(|| {
                self.shared.stopped.load(Ordering::Acquire).then_some(
                    DocumentReviewOperationCompletion {
                        operation_id: id,
                        result: Err(DocumentReviewOperationError::Stopped),
                    },
                )
            })
        };
        if completion.is_some() {
            mailbox.latest = None;
        }
        drop(mailbox);
        if completion.is_some() {
            self.latest = None;
        }
        Ok(completion)
    }

    /// Requests shutdown without joining on the calling thread.
    /// Current work keeps its original token; the supervisor joins the worker.
    pub fn shutdown(&self) {
        self.cancel();
        self.shared.shutdown.store(true, Ordering::Release);
        self.shared.wake.notify_one();
    }

    /// Reports whether the supervisor has completed worker joining and cleanup.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.shared.stopped.load(Ordering::Acquire)
    }

    fn mailbox(&self) -> Result<std::sync::MutexGuard<'_, Mailbox>, DocumentReviewOperationError> {
        self.shared.mailbox.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => DocumentReviewOperationError::Busy,
            TryLockError::Poisoned(_) => DocumentReviewOperationError::Stopped,
        })
    }
}

impl Drop for DocumentReviewOperation {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl std::fmt::Debug for DocumentReviewOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentReviewOperation")
            .field("operation_id", &self.latest.as_ref().map(|(id, _)| id))
            .field("stopped", &self.is_stopped())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
