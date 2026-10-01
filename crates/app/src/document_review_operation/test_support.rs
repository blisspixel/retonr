//! Private orchestration fault fixtures, absent from shipping builds.

use super::DocumentReviewOperation;
use std::{
    sync::{Arc, mpsc},
    thread,
};

impl DocumentReviewOperation {
    pub(crate) fn hold_mailbox_for_test(&self) -> (mpsc::Sender<()>, thread::JoinHandle<()>) {
        let shared = Arc::clone(&self.shared);
        let (ready_tx, ready_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::channel();
        let holder = thread::spawn(move || {
            let _guard = shared.mailbox.lock().expect("fixture mailbox");
            ready_tx.send(()).expect("fixture ready");
            let _ = release_rx.recv();
        });
        ready_rx.recv().expect("mailbox held");
        (release_tx, holder)
    }

    pub(crate) fn poison_mailbox_for_test(&self) {
        let shared = Arc::clone(&self.shared);
        let outcome = std::panic::catch_unwind(move || {
            let _guard = shared.mailbox.lock().expect("fixture mailbox");
            panic!("synthetic mailbox poison");
        });
        assert!(outcome.is_err());
    }
}
