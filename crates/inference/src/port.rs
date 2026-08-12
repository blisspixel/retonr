use std::{future::Future, pin::Pin, time::Instant};

use rewrite_types::CancellationToken;

use crate::{BackendDiscovery, GenerationRequest, GenerationResponse, InferenceError};

/// Object-safe future returned by an inference port.
pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Borrowed cancellation and deadline state for one backend call.
#[derive(Clone, Copy, Debug)]
pub struct OperationContext<'a> {
    cancellation: &'a CancellationToken,
    deadline: Option<Instant>,
}

impl<'a> OperationContext<'a> {
    /// Creates operation context with cooperative cancellation and an optional
    /// monotonic deadline.
    #[must_use]
    pub const fn new(cancellation: &'a CancellationToken, deadline: Option<Instant>) -> Self {
        Self {
            cancellation,
            deadline,
        }
    }

    /// Returns whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// Returns whether the monotonic deadline has expired.
    #[must_use]
    pub fn is_expired(self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    /// Returns the monotonic deadline when one was configured.
    #[must_use]
    pub const fn deadline(self) -> Option<Instant> {
        self.deadline
    }

    /// Returns the shared cancellation token for async cancellation selection.
    #[must_use]
    pub const fn cancellation(self) -> &'a CancellationToken {
        self.cancellation
    }
}

/// Backend-neutral local inference port.
pub trait InferenceBackend: Send + Sync {
    /// Discovers exact runtime identity, capabilities, and installed artifacts.
    fn discover<'a>(
        &'a self,
        context: OperationContext<'a>,
    ) -> PortFuture<'a, Result<BackendDiscovery, InferenceError>>;

    /// Produces bounded candidate payloads for one explicit artifact and policy.
    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        context: OperationContext<'a>,
    ) -> PortFuture<'a, Result<GenerationResponse, InferenceError>>;
}
