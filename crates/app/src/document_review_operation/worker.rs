use super::{DocumentReviewOperationCompletion, DocumentReviewOperationError, Shared};
use crate::{DocumentReviewError, DocumentReviewRequest, DocumentReviewResult};
use rewrite_types::CancellationToken;
use std::{
    io,
    sync::{Arc, atomic::Ordering},
    thread,
    time::Duration,
};

const IDLE_WAKE: Duration = Duration::from_millis(50);

pub(super) fn start(
    shared: Arc<Shared>,
    review: impl Fn(
        DocumentReviewRequest,
        &CancellationToken,
    ) -> Result<DocumentReviewResult, DocumentReviewError>
    + Send
    + 'static,
) -> io::Result<()> {
    thread::Builder::new()
        .name("document-review-supervisor".to_owned())
        .spawn(move || {
            let worker_shared = Arc::clone(&shared);
            let worker = thread::Builder::new()
                .name("document-review-worker".to_owned())
                .spawn(move || run(&worker_shared, &review));
            if let Ok(worker) = worker {
                let _ = worker.join();
            }
            // Joining and shutdown mailbox cleanup happen off the event thread,
            // including panic and worker-spawn failure paths.
            if let Ok(mut mailbox) = shared.mailbox.lock() {
                mailbox.pending = None;
                if shared.shutdown.load(Ordering::Acquire) {
                    mailbox.completed = None;
                }
            }
            shared.stopped.store(true, Ordering::Release);
        })?;
    Ok(())
}

fn run(
    shared: &Shared,
    review: &impl Fn(
        DocumentReviewRequest,
        &CancellationToken,
    ) -> Result<DocumentReviewResult, DocumentReviewError>,
) {
    loop {
        let Ok(mut mailbox) = shared.mailbox.lock() else {
            return;
        };
        while mailbox.pending.is_none() && !shared.shutdown.load(Ordering::Acquire) {
            let Ok((next, _)) = shared.wake.wait_timeout(mailbox, IDLE_WAKE) else {
                return;
            };
            mailbox = next;
        }
        if shared.shutdown.load(Ordering::Acquire) {
            return;
        }
        let Some(job) = mailbox.pending.take() else {
            continue;
        };
        drop(mailbox);
        if job.cancellation.is_cancelled() {
            continue;
        }
        let result =
            review(job.request.0, &job.cancellation).map_err(DocumentReviewOperationError::Review);
        let Ok(mut mailbox) = shared.mailbox.lock() else {
            return;
        };
        if !shared.shutdown.load(Ordering::Acquire)
            && !job.cancellation.is_cancelled()
            && mailbox.latest == Some(job.id)
        {
            mailbox.completed = Some(DocumentReviewOperationCompletion {
                operation_id: job.id,
                result,
            });
        }
    }
}
