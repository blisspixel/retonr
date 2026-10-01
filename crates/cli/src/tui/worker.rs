//! One bounded background reader with tagged responses and cooperative shutdown.

use rewrite_types::CancellationToken;
use std::{
    io,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, JoinHandle},
};

use super::state::Snapshot;

pub(super) struct Response {
    pub operation: u64,
    pub result: Result<Snapshot, &'static str>,
    pub stopped: bool,
}

pub(super) struct Worker<R> {
    requests: Option<SyncSender<(u64, R)>>,
    responses: Receiver<Response>,
    cancellation: CancellationToken,
    handle: Option<JoinHandle<()>>,
    busy: AtomicBool,
    operation: AtomicU64,
}

impl<R: Send + 'static> Worker<R> {
    pub fn new(
        load: impl Fn(R, &CancellationToken) -> Result<Snapshot, &'static str> + Send + 'static,
    ) -> io::Result<Self> {
        let (requests, incoming) = mpsc::sync_channel::<(u64, R)>(1);
        let (outgoing, responses) = mpsc::sync_channel(1);
        let cancellation = CancellationToken::new();
        let thread_cancellation = cancellation.clone();
        let handle = thread::Builder::new()
            .name("read-only-review".into())
            .spawn(move || {
                while let Ok((operation, request)) = incoming.recv() {
                    if thread_cancellation.is_cancelled() {
                        break;
                    }
                    let result = load(request, &thread_cancellation);
                    if thread_cancellation.is_cancelled() {
                        break;
                    }
                    if outgoing
                        .try_send(Response {
                            operation,
                            result,
                            stopped: false,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests: Some(requests),
            responses,
            cancellation,
            handle: Some(handle),
            busy: AtomicBool::new(false),
            operation: AtomicU64::new(0),
        })
    }

    pub fn submit(&self, operation: u64, request: R) -> bool {
        if self
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        self.operation.store(operation, Ordering::Release);
        let submitted = self.requests.as_ref().is_some_and(|requests| {
            match requests.try_send((operation, request)) {
                Ok(()) => true,
                Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => false,
            }
        });
        if !submitted {
            self.busy.store(false, Ordering::Release);
        }
        submitted
    }

    pub fn response(&self) -> Option<Response> {
        match self.responses.try_recv() {
            Ok(response) => {
                self.busy.store(false, Ordering::Release);
                Some(response)
            }
            Err(TryRecvError::Disconnected) if self.busy.swap(false, Ordering::AcqRel) => {
                Some(Response {
                    operation: self.operation.load(Ordering::Acquire),
                    result: Err("read-only worker stopped before completing the snapshot"),
                    stopped: true,
                })
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    pub fn shutdown(&mut self) {
        self.cancellation.cancel();
        self.requests.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.busy.store(false, Ordering::Release);
    }
}

impl<R> Drop for Worker<R> {
    fn drop(&mut self) {
        self.cancellation.cancel();
        self.requests.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests;
