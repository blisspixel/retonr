use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OfflineBatchError {
    ForcedRevalidation,
}

pub(crate) struct OfflineBatchFailureControl {
    revalidation_calls: Rc<Cell<usize>>,
    fail_on_call: Rc<Cell<Option<usize>>>,
}

impl OfflineBatchFailureControl {
    pub(super) fn new(
        revalidation_calls: Rc<Cell<usize>>,
        fail_on_call: Rc<Cell<Option<usize>>>,
    ) -> Self {
        Self {
            revalidation_calls,
            fail_on_call,
        }
    }

    pub(crate) fn revalidation_calls(&self) -> usize {
        self.revalidation_calls.get()
    }

    pub(crate) fn fail_on_call(&self, call: usize) {
        self.fail_on_call.set(Some(call));
    }
}
