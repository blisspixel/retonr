#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ManagedSnapshotTestReason {
    #[default]
    None,
    ListenerBefore,
    ProcRoot,
    ProcEntry,
    ProcessMetadata,
    FdDirectory,
    FdEntry,
    FdLink,
    HolderEmpty,
    HolderWrong,
    HolderAmbiguous,
    ListenerAfter,
}

std::thread_local! {
    static SNAPSHOT_TEST_REASON: std::cell::Cell<ManagedSnapshotTestReason> =
        const { std::cell::Cell::new(ManagedSnapshotTestReason::None) };
}

pub(crate) fn record_snapshot_test_reason(reason: ManagedSnapshotTestReason) {
    SNAPSHOT_TEST_REASON.set(reason);
}

pub(crate) fn take_snapshot_test_reason() -> ManagedSnapshotTestReason {
    SNAPSHOT_TEST_REASON.replace(ManagedSnapshotTestReason::None)
}
