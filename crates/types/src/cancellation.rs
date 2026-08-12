use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Waker},
};

/// Cloneable cooperative cancellation signal shared across application ports.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    state: Arc<CancellationState>,
}

#[derive(Debug, Default)]
struct CancellationState {
    cancelled: AtomicBool,
    waiters: Mutex<Vec<Waker>>,
}

impl CancellationToken {
    /// Creates a token in the active state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation for every clone of this token.
    pub fn cancel(&self) {
        if self.state.cancelled.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Ok(mut waiters) = self.state.waiters.lock() {
            for waiter in waiters.drain(..) {
                waiter.wake();
            }
        }
    }

    /// Returns whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    /// Returns a future that completes when cancellation is requested.
    #[must_use]
    pub const fn cancelled(&self) -> Cancelled<'_> {
        Cancelled { token: self }
    }
}

/// Future that completes after its cancellation token is cancelled.
#[derive(Debug)]
pub struct Cancelled<'a> {
    token: &'a CancellationToken,
}

impl Future for Cancelled<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.token.is_cancelled() {
            return Poll::Ready(());
        }
        let Ok(mut waiters) = self.token.state.waiters.lock() else {
            return Poll::Ready(());
        };
        if self.token.is_cancelled() {
            return Poll::Ready(());
        }
        if !waiters
            .iter()
            .any(|waiter| waiter.will_wake(context.waker()))
        {
            waiters.push(context.waker().clone());
        }
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use std::task::{Context, Poll, Waker};

    use super::CancellationToken;

    #[test]
    fn cancellation_is_shared_across_clones() {
        let first = CancellationToken::new();
        let second = first.clone();
        assert!(!second.is_cancelled());
        first.cancel();
        assert!(second.is_cancelled());
    }

    #[test]
    fn cancelled_future_transitions_from_pending_to_ready() {
        let token = CancellationToken::new();
        let mut future = Box::pin(token.cancelled());
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        token.cancel();
        assert!(matches!(
            future.as_mut().poll(&mut context),
            Poll::Ready(())
        ));
    }
}
