use super::*;
use tempfile::tempdir;

const fn flags(requested: bool, backup: bool) -> InPlaceFlags {
    InPlaceFlags { requested, backup }
}

#[test]
fn in_place_implies_backup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    fs::write(&source, b"original\n").expect("write source");
    let destination = resolve_destination(&source, None, flags(true, false), CommandName::Check)
        .expect("in-place implies backup");
    assert!(matches!(destination, Destination::InPlace { .. }));
    let with_flag = resolve_destination(&source, None, flags(true, true), CommandName::Check)
        .expect("redundant --backup remains valid");
    assert_eq!(destination, with_flag);
}

#[test]
fn in_place_reserves_its_source_backup_and_staging_paths() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    fs::write(&source, b"original\n").expect("write source");
    let destination = resolve_destination(&source, None, flags(true, false), CommandName::Check)
        .expect("resolve destination");

    for path in [
        source.clone(),
        directory.path().join("draft.txt.retonr-backup"),
        directory.path().join("draft.txt.retonr-staging"),
    ] {
        assert!(
            destination
                .reserves_path(&path, CommandName::Check)
                .expect("reserved path check"),
            "{} must be reserved",
            path.display()
        );
    }
    assert!(
        !destination
            .reserves_path(&directory.path().join("trace.json"), CommandName::Check)
            .expect("unreserved path check")
    );
}

#[test]
fn output_reservation_normalizes_existing_parent_components() {
    let directory = tempdir().expect("temporary directory");
    let nested = directory.path().join("nested");
    fs::create_dir(&nested).expect("create nested directory");
    let direct = directory.path().join("accepted.txt");
    let aliased = nested.join("..").join("accepted.txt");
    let destination = Destination::Sink(OutputSink::File(direct));

    assert!(
        destination
            .reserves_path(&aliased, CommandName::Check)
            .expect("reserved path check")
    );
}

#[test]
fn backup_without_in_place_is_usage() {
    let failure = resolve_destination(
        Path::new("draft.txt"),
        None,
        flags(false, true),
        CommandName::Rewrite,
    )
    .expect_err("backup requires in-place");
    assert!(failure.message.contains("requires --in-place"));
}

#[test]
fn in_place_with_output_is_usage() {
    let failure = resolve_destination(
        Path::new("draft.txt"),
        Some(Path::new("out.txt")),
        flags(true, true),
        CommandName::Check,
    )
    .expect_err("in-place rejects --output");
    assert!(failure.message.contains("incompatible with --output"));
}

#[test]
fn in_place_on_standard_input_is_usage() {
    let failure = resolve_destination(Path::new("-"), None, flags(true, true), CommandName::Check)
        .expect_err("stdin is refused");
    assert!(failure.message.contains("standard input"));
}

#[test]
fn commit_leaves_identical_bytes_untouched() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    fs::write(&source, b"same\n").expect("write source");
    let outcome = commit(&source, b"same\n", b"same\n", CommandName::Check).expect("commit");
    assert!(matches!(outcome, InPlaceOutcome::Unchanged));
    assert_eq!(fs::read(&source).expect("read source"), b"same\n");
    assert!(!directory.path().join("draft.txt.retonr-backup").exists());
    assert!(!directory.path().join("draft.txt.retonr-staging").exists());
}

#[test]
fn unchanged_commit_rejects_a_source_changed_after_validation() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    fs::write(&source, b"changed\n").expect("write changed source");

    let failure = commit(&source, b"original\n", b"original\n", CommandName::Check)
        .expect_err("changed source must not be reported unchanged");

    assert_eq!(
        failure.body,
        ErrorBody::new(
            ErrorCategory::Operational,
            ErrorCode::ConcurrentModification,
            true,
        )
    );
    assert_eq!(
        failure.exit_code,
        ExitCode::from(crate::contract::EXIT_OPERATIONAL)
    );
    assert_eq!(fs::read(&source).expect("read source"), b"changed\n");
    assert!(!directory.path().join("draft.txt.retonr-backup").exists());
    assert!(!directory.path().join("draft.txt.retonr-staging").exists());
}

#[test]
fn commit_retains_a_backup_and_replaces_the_source() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    fs::write(&source, b"original\n").expect("write source");
    let outcome =
        commit(&source, b"original\n", b"accepted\n", CommandName::Check).expect("commit");
    match outcome {
        InPlaceOutcome::Replaced { backup_name } => {
            assert_eq!(backup_name, "draft.txt.retonr-backup");
        }
        InPlaceOutcome::Unchanged => panic!("changed bytes must replace"),
    }
    assert_eq!(fs::read(&source).expect("read source"), b"accepted\n");
    assert_eq!(
        fs::read(directory.path().join("draft.txt.retonr-backup")).expect("read backup"),
        b"original\n"
    );
    assert!(!directory.path().join("draft.txt.retonr-staging").exists());
}

#[test]
fn existing_backup_is_refused_without_mutation() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let backup = directory.path().join("draft.txt.retonr-backup");
    fs::write(&source, b"original\n").expect("write source");
    fs::write(&backup, b"keep\n").expect("write backup");
    let failure = commit(&source, b"original\n", b"accepted\n", CommandName::Check)
        .expect_err("existing backup");
    assert_eq!(
        failure.exit_code,
        ExitCode::from(crate::contract::EXIT_POLICY)
    );
    assert_eq!(fs::read(&source).expect("read source"), b"original\n");
    assert_eq!(fs::read(&backup).expect("read backup"), b"keep\n");
}

#[test]
fn existing_staging_is_refused_without_mutation() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let staging = directory.path().join("draft.txt.retonr-staging");
    fs::write(&source, b"original\n").expect("write source");
    fs::write(&staging, b"keep\n").expect("write staging");
    let failure = commit(&source, b"original\n", b"accepted\n", CommandName::Rewrite)
        .expect_err("existing staging");
    assert_eq!(
        failure.exit_code,
        ExitCode::from(crate::contract::EXIT_POLICY)
    );
    assert_eq!(fs::read(&source).expect("read source"), b"original\n");
    assert_eq!(fs::read(&staging).expect("read staging"), b"keep\n");
    assert!(!directory.path().join("draft.txt.retonr-backup").exists());
}

#[cfg(unix)]
#[test]
fn dangling_backup_link_is_reserved_before_document_work() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let backup = directory.path().join("draft.txt.retonr-backup");
    fs::write(&source, b"original\n").expect("write source");
    symlink(directory.path().join("missing.txt"), &backup).expect("create dangling backup link");

    let failure = resolve_destination(&source, None, flags(true, false), CommandName::Check)
        .expect_err("dangling backup link reserves the path");
    assert_eq!(
        failure.body,
        ErrorBody::new(ErrorCategory::Policy, ErrorCode::OutputExists, false)
    );
    assert_eq!(fs::read(&source).expect("read source"), b"original\n");
}

#[test]
fn hard_link_alias_is_refused_without_mutation() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let alias = directory.path().join("alias.txt");
    fs::write(&source, b"original\n").expect("write source");
    fs::hard_link(&source, &alias).expect("create hard-link alias");

    let failure = commit(&source, b"original\n", b"accepted\n", CommandName::Check)
        .expect_err("hard-linked source is ambiguous");

    assert_eq!(failure.exit_code, ExitCode::from(EXIT_USAGE));
    assert!(failure.message.contains("hard-link aliases"));
    assert_eq!(fs::read(&source).expect("read source"), b"original\n");
    assert_eq!(fs::read(&alias).expect("read alias"), b"original\n");
    assert!(!directory.path().join("draft.txt.retonr-backup").exists());
    assert!(!directory.path().join("draft.txt.retonr-staging").exists());
}

#[test]
fn install_replaces_one_path_without_rewriting_a_late_hard_link_alias() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let alias = directory.path().join("late-alias.txt");
    let staging = directory.path().join("draft.txt.retonr-staging");
    fs::write(&source, b"original\n").expect("write source");
    fs::hard_link(&source, &alias).expect("create late hard-link alias");
    fs::write(&staging, b"accepted\n").expect("write staging");

    install(&source, &staging, CommandName::Check).expect("replace source path");

    assert_eq!(fs::read(&source).expect("read source"), b"accepted\n");
    assert_eq!(fs::read(&alias).expect("read alias"), b"original\n");
    assert!(!staging.exists());
}

#[test]
fn verified_install_rejects_source_drift_before_replacement() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("draft.txt");
    let staging = directory.path().join("draft.txt.retonr-staging");
    fs::write(&source, b"changed\n").expect("write changed source");
    fs::write(&staging, b"accepted\n").expect("write staging");

    let failure = install_verified(&source, &staging, b"original\n", CommandName::Rewrite)
        .expect_err("source drift must stop replacement");

    assert_eq!(
        failure.body,
        ErrorBody::new(
            ErrorCategory::Operational,
            ErrorCode::ConcurrentModification,
            true,
        )
    );
    assert_eq!(fs::read(&source).expect("read source"), b"changed\n");
    assert_eq!(fs::read(&staging).expect("read staging"), b"accepted\n");
}

#[test]
fn sibling_names_stay_in_the_source_directory() {
    let path = Path::new("nested").join("draft.txt");
    let backup = sibling(&path, BACKUP_SUFFIX, CommandName::Check).expect("backup");
    assert_eq!(backup, Path::new("nested").join("draft.txt.retonr-backup"));
    assert_eq!(
        backup.file_name().and_then(|name| name.to_str()),
        Some("draft.txt.retonr-backup")
    );
}
