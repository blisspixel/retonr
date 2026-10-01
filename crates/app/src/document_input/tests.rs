use super::*;
use tempfile::tempdir;

fn read_direct_bounded(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    RetainedInput::open(path, LinkPolicy::Direct, false)?.read(limit)
}

fn original() -> Vec<u8> {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../../fixtures/cli/retained-input.json"))
            .expect("input fixture");
    fixture["original"]
        .as_str()
        .expect("original text")
        .as_bytes()
        .to_vec()
}

#[test]
fn retained_reads_preserve_bytes_and_enforce_exact_and_zero_limits() {
    let root = tempdir().expect("root");
    let path = root.path().join("draft.txt");
    let bytes = original();
    fs::write(&path, &bytes).expect("write source");
    assert_eq!(
        read_regular_bounded(&path, bytes.len()).expect("exact limit"),
        bytes
    );
    assert_eq!(
        read_direct_bounded(&path, bytes.len()).expect("direct read"),
        bytes
    );
    assert_eq!(
        read_regular_bounded(&path, bytes.len() - 1)
            .expect_err("over limit")
            .kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(
        read_direct_bounded(root.path(), 100)
            .expect_err("directory")
            .kind(),
        io::ErrorKind::InvalidInput
    );
    fs::write(&path, []).expect("empty source");
    assert!(
        read_regular_bounded(&path, 0)
            .expect("zero limit empty")
            .is_empty()
    );
    assert_eq!(
        read_regular_bounded(root.path(), 100)
            .expect_err("directory")
            .kind(),
        io::ErrorKind::InvalidInput
    );
}

#[test]
fn changes_after_handle_selection_are_retryable_content_free_failures() {
    let root = tempdir().expect("root");
    let path = root.path().join("private-draft.txt");
    fs::write(&path, original()).expect("source");
    let retained = RetainedInput::open(&path, LinkPolicy::Direct, false).expect("retain source");
    fs::write(&path, "Changed draft\n").expect("mutate selected object");
    let error = retained.read(100).expect_err("changed object");
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    assert!(!error.to_string().contains("private-draft"));
    assert!(is_changed(&error));
}

#[test]
fn unaliased_reads_refuse_hard_links_using_the_opened_file() {
    let root = tempdir().expect("root");
    let path = root.path().join("draft.txt");
    fs::write(&path, original()).expect("source");
    fs::hard_link(&path, root.path().join("alias.txt")).expect("hard link");
    assert_eq!(
        read_direct_unaliased_bounded(&path, 100)
            .expect_err("alias")
            .kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(
        read_direct_bounded(&path, 100).expect("read-only aliases remain permitted"),
        original()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn replaced_fifo_after_regular_observation_never_waits_for_a_writer() {
    use rustix::fs::{CWD, Mode, mkfifoat};
    let root = tempdir().expect("root");
    let path = root.path().join("draft.txt");
    fs::write(&path, original()).expect("original regular file");
    let observed = fs::metadata(&path).expect("regular-file observation");
    fs::rename(&path, root.path().join("original.txt")).expect("replace after observation");
    mkfifoat(CWD, &path, Mode::RUSR | Mode::WUSR).expect("replacement FIFO");
    let (sender, receiver) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        sender
            .send(open_observed_regular(&path, &observed))
            .expect("send open result");
    });
    let error = receiver
        .recv_timeout(std::time::Duration::from_secs(3))
        .expect("FIFO open must not await a writer")
        .expect_err("opened FIFO must fail validation");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    worker.join().expect("reader worker");
}

#[cfg(unix)]
#[test]
fn direct_reads_refuse_final_and_ancestor_links_but_single_files_follow() {
    use std::os::unix::fs::symlink;
    let root = tempdir().expect("root");
    fs::create_dir(root.path().join("real")).expect("real directory");
    let real = root.path().join("real/draft.txt");
    fs::write(&real, original()).expect("real file");
    let link = root.path().join("link.txt");
    symlink(&real, &link).expect("file link");
    symlink(root.path().join("real"), root.path().join("alias")).expect("directory link");
    assert_eq!(
        read_regular_bounded(&link, 100).expect("ordinary symlink"),
        original()
    );
    assert!(read_direct_bounded(&link, 100).is_err());
    assert!(read_directory_bounded(root.path(), Path::new("alias/draft.txt"), 100).is_err());
}

#[cfg(unix)]
#[test]
fn same_byte_file_and_ancestor_substitution_after_selection_are_detected() {
    use std::os::unix::fs::symlink;
    let root = tempdir().expect("root");
    let parent = root.path().join("parent");
    fs::create_dir(&parent).expect("parent");
    let path = parent.join("draft.txt");
    fs::write(&path, original()).expect("original");
    let retained = RetainedInput::open(&path, LinkPolicy::Direct, false).expect("retain object");
    fs::rename(&path, parent.join("old.txt")).expect("move selected file");
    fs::write(&path, original()).expect("same-byte new object");
    assert_eq!(
        retained
            .read(100)
            .expect_err("same-byte substitution")
            .kind(),
        io::ErrorKind::Interrupted
    );
    let retained =
        RetainedInput::open_in_directory(root.path(), Path::new("parent/draft.txt"), false)
            .expect("retain ancestry");
    let moved = root.path().join("moved");
    fs::rename(&parent, &moved).expect("move parent");
    symlink(&moved, &parent).expect("ancestor alias to same object");
    assert_eq!(
        retained
            .read(100)
            .expect_err("ancestor link substituted")
            .kind(),
        io::ErrorKind::Interrupted
    );
}

#[cfg(windows)]
#[test]
fn retained_direct_handles_prevent_path_and_parent_substitution_during_read() {
    let root = tempdir().expect("root");
    let parent = root.path().join("parent");
    fs::create_dir(&parent).expect("parent");
    let path = parent.join("draft.txt");
    fs::write(&path, original()).expect("source");
    let retained =
        RetainedInput::open(&path, LinkPolicy::Direct, false).expect("retain file and ancestors");
    assert!(fs::rename(&path, parent.join("renamed.txt")).is_err());
    assert!(fs::rename(&parent, root.path().join("moved")).is_err());
    assert_eq!(retained.read(100).expect("read stable object"), original());
    fs::rename(&path, parent.join("renamed.txt")).expect("rename after reader closes");
}

#[cfg(unix)]
#[test]
fn declared_directory_root_can_have_explicit_ancestry_aliases() {
    use std::os::unix::fs::symlink;
    let root = tempdir().expect("root");
    let actual = root.path().join("actual");
    fs::create_dir_all(actual.join("selected")).expect("selected root");
    fs::write(actual.join("selected/draft.txt"), original()).expect("draft");
    let alias = root.path().join("alias");
    symlink(&actual, &alias).expect("explicit ancestor alias");
    assert_eq!(
        read_directory_bounded(&alias.join("selected"), Path::new("draft.txt"), 100)
            .expect("canonical caller-selected root"),
        original()
    );
}

#[test]
fn relative_directory_reads_refuse_escape_components_and_linked_roots() {
    let root = tempdir().expect("root");
    for relative in ["../outside.txt", "/absolute.txt", ""] {
        assert!(read_directory_bounded(root.path(), Path::new(relative), 100).is_err());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let parent = tempdir().expect("parent");
        symlink(root.path(), parent.path().join("linked")).expect("linked root");
        assert!(
            read_directory_bounded(&parent.path().join("linked"), Path::new("draft.txt"), 100)
                .is_err()
        );
    }
}

#[test]
fn unrelated_interruption_is_not_a_retained_input_change() {
    assert!(!is_changed(&io::Error::from(io::ErrorKind::Interrupted)));
}
