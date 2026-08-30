use rustix::io::Errno;

use super::{
    ProcAccessClass, classify_errno, map_io_error, map_pidfd_error, map_pidfd_liveness_error,
    parse_stat, parse_status,
};
use crate::{AttachedProcessWitnessError, ManagedGenerationWorkerError};

fn stat(state: &str, parent: &str, start: &str) -> Vec<u8> {
    let mut fields = vec![state.to_owned(), parent.to_owned()];
    fields.extend((0..17).map(|_| "0".to_owned()));
    fields.push(start.to_owned());
    format!("42 (worker with ) name) {}\n", fields.join(" ")).into_bytes()
}

fn status(cap_eff: &str, seccomp: &str) -> String {
    format!(
        "Name:\tworker\nUid:\t1000\t1000\t1000\t1000\nNoNewPrivs:\t1\nSeccomp:\t{seccomp}\nCapInh:\t0000000000000000\nCapPrm:\t0000000000000000\nCapEff:\t{cap_eff}\nCapBnd:\t0000000000000000\nCapAmb:\t0000000000000000\n"
    )
}

#[test]
fn stat_parser_handles_parentheses_and_rejects_dead_or_truncated_rows() {
    let observed = parse_stat(&stat("S", "7", "99")).expect("valid stat");
    assert_eq!(observed.parent_pid, 7);
    assert_eq!(observed.start_token, 99);
    assert_eq!(
        parse_stat(&stat("Z", "7", "99")),
        Err(ManagedGenerationWorkerError::WorkerChanged)
    );
    assert_eq!(
        parse_stat(b"42 missing-close"),
        Err(ManagedGenerationWorkerError::PlatformObservationFailed)
    );
}

#[test]
fn status_parser_requires_uid_nnp_seccomp_and_all_zero_capability_sets() {
    assert_eq!(
        parse_status(&status("0000000000000000", "2"), true),
        Ok(1000)
    );
    assert_eq!(
        parse_status(&status("0000000000000001", "2"), true),
        Err(ManagedGenerationWorkerError::PrivilegeMismatch)
    );
    assert_eq!(
        parse_status(&status("0000000000000000", "0"), true),
        Err(ManagedGenerationWorkerError::PrivilegeMismatch)
    );
    assert_eq!(parse_status(&status("1", "0"), false), Ok(1000));
}

#[test]
fn only_pidfd_missing_errors_are_classified_as_confirmed_exit_races() {
    use ProcAccessClass::{AccessDenied, Exited, Incomplete, ResourceLimit};

    for (error, class, pidfd, io) in [
        (
            Errno::NOENT,
            Exited,
            ManagedGenerationWorkerError::WorkerChanged,
            ManagedGenerationWorkerError::PlatformObservationFailed,
        ),
        (
            Errno::SRCH,
            Exited,
            ManagedGenerationWorkerError::WorkerChanged,
            ManagedGenerationWorkerError::PlatformObservationFailed,
        ),
        (
            Errno::ACCESS,
            AccessDenied,
            ManagedGenerationWorkerError::ProcessVisibilityInsufficient,
            ManagedGenerationWorkerError::ProcessVisibilityInsufficient,
        ),
        (
            Errno::MFILE,
            ResourceLimit,
            ManagedGenerationWorkerError::ResourceLimit,
            ManagedGenerationWorkerError::ResourceLimit,
        ),
        (
            Errno::IO,
            Incomplete,
            ManagedGenerationWorkerError::PlatformObservationFailed,
            ManagedGenerationWorkerError::PlatformObservationFailed,
        ),
    ] {
        assert_eq!(classify_errno(error), class);
        assert_eq!(map_pidfd_error(error), pidfd);
        assert_eq!(
            map_io_error(&std::io::Error::from_raw_os_error(error.raw_os_error())),
            io
        );
    }
    assert_eq!(
        map_pidfd_liveness_error(AttachedProcessWitnessError::ProcessExited),
        ManagedGenerationWorkerError::WorkerChanged
    );
    assert_eq!(
        map_pidfd_liveness_error(AttachedProcessWitnessError::ProcessInstanceUnavailable),
        ManagedGenerationWorkerError::PlatformObservationFailed
    );
}
