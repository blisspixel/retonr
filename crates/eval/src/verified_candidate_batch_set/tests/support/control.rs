use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub(super) type FailurePredicate = Rc<RefCell<Option<Box<dyn Fn() -> bool>>>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OfflineBatchError {
    ForcedRevalidation,
}

pub(crate) struct OfflineBatchFailureControl {
    revalidation_calls: Rc<Cell<usize>>,
    fail_on_call: Rc<Cell<Option<usize>>>,
    predicate: FailurePredicate,
}

impl OfflineBatchFailureControl {
    pub(super) fn new(
        revalidation_calls: Rc<Cell<usize>>,
        fail_on_call: Rc<Cell<Option<usize>>>,
        predicate: FailurePredicate,
    ) -> Self {
        Self {
            revalidation_calls,
            fail_on_call,
            predicate,
        }
    }

    pub(crate) fn fail_when(&self, predicate: impl Fn() -> bool + 'static) {
        *self.predicate.borrow_mut() = Some(Box::new(predicate));
    }

    pub(crate) fn revalidation_calls(&self) -> usize {
        self.revalidation_calls.get()
    }

    pub(crate) fn fail_on_call(&self, call: usize) {
        self.fail_on_call.set(Some(call));
    }
}

impl super::OfflineBatch {
    pub(super) fn revalidate_for_test(&self) -> Result<(), OfflineBatchError> {
        let call = self.revalidation_calls.get() + 1;
        self.revalidation_calls.set(call);
        self.log.borrow_mut().push(self.label);
        if self.fail_on_call.get() == Some(call)
            || self
                .failure_predicate
                .borrow()
                .as_ref()
                .is_some_and(|predicate| predicate())
        {
            Err(OfflineBatchError::ForcedRevalidation)
        } else {
            Ok(())
        }
    }
}
