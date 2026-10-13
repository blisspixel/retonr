use std::fs::File;

pub(super) fn archive(entries: &[(&str, tar::EntryType, &[u8])]) -> File {
    let file = tempfile::tempfile().expect("archive");
    let mut builder = tar::Builder::new(file);
    for (path, kind, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(*kind);
        header.set_mode(if kind.is_dir() { 0o755 } else { 0o644 });
        header.set_size(u64::try_from(bytes.len()).expect("size"));
        let path_bytes = path.as_bytes();
        assert!(path_bytes.len() < 100);
        header.as_mut_bytes()[..100].fill(0);
        header.as_mut_bytes()[..path_bytes.len()].copy_from_slice(path_bytes);
        header.set_cksum();
        builder.append(&header, *bytes).expect("append");
    }
    builder.into_inner().expect("finish")
}

pub(super) fn linked_archive(entries: &[(&str, tar::EntryType, Option<&str>, &[u8])]) -> File {
    let file = tempfile::tempfile().expect("archive");
    let mut builder = tar::Builder::new(file);
    for (path, kind, target, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(*kind);
        header.set_mode(if kind.is_dir() { 0o755 } else { 0o644 });
        header.set_size(u64::try_from(bytes.len()).expect("size"));
        let path_bytes = path.as_bytes();
        header.as_mut_bytes()[..100].fill(0);
        header.as_mut_bytes()[..path_bytes.len()].copy_from_slice(path_bytes);
        if let Some(target) = target {
            header.set_link_name(target).expect("link target");
        }
        header.set_cksum();
        builder.append(&header, *bytes).expect("append");
    }
    builder.into_inner().expect("finish")
}
