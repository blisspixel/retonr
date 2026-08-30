use std::{collections::BTreeSet, fs, path::Path};

use rewrite_model::ArtifactSetRelativePath;

use crate::BuildError;

const MAXIMUM_PATCH_BYTES: usize = 1024 * 1024;
const MAXIMUM_PATCH_FILES: usize = 64;
const MAXIMUM_SOURCE_BYTES: u64 = 32 * 1024 * 1024;
const MAXIMUM_HUNK_OFFSET_LINES: usize = 4_096;

pub(super) fn apply_exact_unified_diff(
    patch_path: &Path,
    source_root: &Path,
) -> Result<(), BuildError> {
    let bytes = fs::read(patch_path).map_err(|_| BuildError::InvalidPatch)?;
    if bytes.is_empty() || bytes.len() > MAXIMUM_PATCH_BYTES || bytes.contains(&b'\r') {
        return Err(BuildError::InvalidPatch);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| BuildError::InvalidPatch)?;
    if !text.ends_with('\n') {
        return Err(BuildError::InvalidPatch);
    }
    let lines = text.split_inclusive('\n').collect::<Vec<_>>();
    let mut cursor = 0_usize;
    let mut paths = BTreeSet::new();
    let mut updates = Vec::new();
    while cursor < lines.len() {
        let path = parse_file_header(&lines, &mut cursor)?;
        if !paths.insert(path.clone()) || paths.len() > MAXIMUM_PATCH_FILES {
            return Err(BuildError::InvalidPatch);
        }
        let source_path = source_root.join(path.as_str());
        let metadata = fs::symlink_metadata(&source_path).map_err(|_| BuildError::InvalidPatch)?;
        if !metadata.is_file() || metadata.len() > MAXIMUM_SOURCE_BYTES {
            return Err(BuildError::InvalidPatch);
        }
        let source = fs::read(&source_path).map_err(|_| BuildError::InvalidPatch)?;
        let source = std::str::from_utf8(&source).map_err(|_| BuildError::InvalidPatch)?;
        if !source.ends_with('\n') || source.contains('\r') {
            return Err(BuildError::InvalidPatch);
        }
        let transformed_bytes = apply_file(source, &lines, &mut cursor)?;
        updates.push((source_path, transformed_bytes));
    }
    if updates.is_empty() {
        return Err(BuildError::InvalidPatch);
    }
    for (path, bytes) in updates {
        fs::write(path, bytes).map_err(|_| BuildError::InvalidPatch)?;
    }
    Ok(())
}

fn parse_file_header(
    lines: &[&str],
    cursor: &mut usize,
) -> Result<ArtifactSetRelativePath, BuildError> {
    let line = next(lines, cursor)?;
    let fields = line
        .strip_prefix("diff --git a/")
        .and_then(|line| line.strip_suffix('\n'))
        .and_then(|line| line.split_once(" b/"))
        .ok_or(BuildError::InvalidPatch)?;
    if fields.0 != fields.1 {
        return Err(BuildError::InvalidPatch);
    }
    let path = ArtifactSetRelativePath::new(fields.0).map_err(|_| BuildError::InvalidPatch)?;
    while lines
        .get(*cursor)
        .is_some_and(|line| !line.starts_with("--- "))
    {
        *cursor += 1;
    }
    if next(lines, cursor)? != format!("--- a/{}\n", path.as_str())
        || next(lines, cursor)? != format!("+++ b/{}\n", path.as_str())
    {
        return Err(BuildError::InvalidPatch);
    }
    Ok(path)
}

fn apply_file(
    source: &str,
    patch_lines: &[&str],
    cursor: &mut usize,
) -> Result<Vec<u8>, BuildError> {
    let source_lines = source.split_inclusive('\n').collect::<Vec<_>>();
    let mut source_cursor = 0_usize;
    let mut output = Vec::with_capacity(source.len());
    let mut hunks = 0_usize;
    while patch_lines
        .get(*cursor)
        .is_some_and(|line| !line.starts_with("diff --git "))
    {
        let (old_start, old_count, new_count) = parse_hunk(next(patch_lines, cursor)?)?;
        let declared_index = old_start.checked_sub(1).ok_or(BuildError::InvalidPatch)?;
        let operations_start = *cursor;
        let mut consumed = 0_usize;
        let mut produced = 0_usize;
        while let Some(line) = patch_lines.get(*cursor) {
            if line.starts_with("@@ ") || line.starts_with("diff --git ") {
                break;
            }
            *cursor += 1;
            let (kind, _content) = line.split_at(1);
            match kind {
                " " => {
                    consumed += 1;
                    produced += 1;
                }
                "-" => {
                    consumed += 1;
                }
                "+" => {
                    produced += 1;
                }
                _ => return Err(BuildError::InvalidPatch),
            }
        }
        if consumed != old_count || produced != new_count {
            return Err(BuildError::InvalidPatch);
        }
        let operations = &patch_lines[operations_start..*cursor];
        let old_index = unique_hunk_index(
            &source_lines,
            source_cursor,
            declared_index,
            consumed,
            operations,
        )?;
        for line in &source_lines[source_cursor..old_index] {
            output.extend_from_slice(line.as_bytes());
        }
        source_cursor = old_index;
        for line in operations {
            let (kind, content) = line.split_at(1);
            match kind {
                " " => {
                    require_source_line(&source_lines, &mut source_cursor, content)?;
                    output.extend_from_slice(content.as_bytes());
                }
                "-" => require_source_line(&source_lines, &mut source_cursor, content)?,
                "+" => output.extend_from_slice(content.as_bytes()),
                _ => return Err(BuildError::InvalidPatch),
            }
        }
        hunks += 1;
    }
    if hunks == 0 {
        return Err(BuildError::InvalidPatch);
    }
    for line in &source_lines[source_cursor..] {
        output.extend_from_slice(line.as_bytes());
    }
    Ok(output)
}

fn unique_hunk_index(
    source: &[&str],
    source_cursor: usize,
    declared_index: usize,
    consumed: usize,
    operations: &[&str],
) -> Result<usize, BuildError> {
    if declared_index > source.len() {
        return Err(BuildError::InvalidPatch);
    }
    if consumed == 0 {
        return (declared_index >= source_cursor)
            .then_some(declared_index)
            .ok_or(BuildError::InvalidPatch);
    }
    let last = source
        .len()
        .checked_sub(consumed)
        .ok_or(BuildError::InvalidPatch)?;
    let first = source_cursor.max(declared_index.saturating_sub(MAXIMUM_HUNK_OFFSET_LINES));
    let last = last.min(declared_index.saturating_add(MAXIMUM_HUNK_OFFSET_LINES));
    if first > last {
        return Err(BuildError::InvalidPatch);
    }
    let mut matched = None;
    for candidate in first..=last {
        if old_hunk_matches(source, candidate, operations) && matched.replace(candidate).is_some() {
            return Err(BuildError::InvalidPatch);
        }
    }
    matched.ok_or(BuildError::InvalidPatch)
}

fn old_hunk_matches(source: &[&str], candidate: usize, operations: &[&str]) -> bool {
    let mut cursor = candidate;
    for line in operations {
        let (kind, content) = line.split_at(1);
        if kind != "+" {
            if source.get(cursor).copied() != Some(content) {
                return false;
            }
            cursor += 1;
        }
    }
    true
}

fn parse_hunk(line: &str) -> Result<(usize, usize, usize), BuildError> {
    let ranges = line
        .strip_prefix("@@ -")
        .and_then(|line| line.split_once(" @@"))
        .map(|(ranges, _context)| ranges)
        .ok_or(BuildError::InvalidPatch)?;
    let (old, new) = ranges.split_once(" +").ok_or(BuildError::InvalidPatch)?;
    let (old_start, old_count) = parse_range(old)?;
    let (_new_start, new_count) = parse_range(new)?;
    Ok((old_start, old_count, new_count))
}

fn parse_range(value: &str) -> Result<(usize, usize), BuildError> {
    let (start, count) = value.split_once(',').unwrap_or((value, "1"));
    let start = start.parse().map_err(|_| BuildError::InvalidPatch)?;
    let count = count.parse().map_err(|_| BuildError::InvalidPatch)?;
    Ok((start, count))
}

fn require_source_line(
    source: &[&str],
    cursor: &mut usize,
    expected: &str,
) -> Result<(), BuildError> {
    if source.get(*cursor).copied() != Some(expected) {
        return Err(BuildError::InvalidPatch);
    }
    *cursor += 1;
    Ok(())
}

fn next<'a>(lines: &'a [&str], cursor: &mut usize) -> Result<&'a str, BuildError> {
    let line = lines
        .get(*cursor)
        .copied()
        .ok_or(BuildError::InvalidPatch)?;
    *cursor += 1;
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn exact_multi_file_patch_applies_once_and_fails_on_drift() {
        let root = tempdir().expect("source root");
        fs::create_dir(root.path().join("src")).expect("source directory");
        fs::write(root.path().join("src/a.txt"), b"one\ntwo\nthree\n").expect("first source");
        fs::write(root.path().join("src/b.txt"), b"alpha\nbeta\n").expect("second source");
        let patch = root.path().join("change.patch");
        fs::write(
            &patch,
            b"diff --git a/src/a.txt b/src/a.txt\n--- a/src/a.txt\n+++ b/src/a.txt\n@@ -1,3 +1,3 @@\n one\n-two\n+changed\n three\ndiff --git a/src/b.txt b/src/b.txt\n--- a/src/b.txt\n+++ b/src/b.txt\n@@ -1,2 +1,3 @@\n alpha\n+middle\n beta\n",
        )
        .expect("patch");
        apply_exact_unified_diff(&patch, root.path()).expect("exact patch applies");
        assert_eq!(
            fs::read(root.path().join("src/a.txt")).expect("patched first source"),
            b"one\nchanged\nthree\n"
        );
        assert_eq!(
            fs::read(root.path().join("src/b.txt")).expect("patched second source"),
            b"alpha\nmiddle\nbeta\n"
        );
        assert_eq!(
            apply_exact_unified_diff(&patch, root.path()),
            Err(BuildError::InvalidPatch)
        );
    }

    #[test]
    fn traversal_malformed_counts_and_carriage_returns_fail_closed() {
        let root = tempdir().expect("source root");
        for patch in [
            b"diff --git a/../outside b/../outside\n--- a/../outside\n+++ b/../outside\n@@ -1 +1 @@\n-old\n+new\n".as_slice(),
            b"diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1,2 +1,1 @@\n-old\n+new\n".as_slice(),
            b"diff --git a/a b/a\r\n".as_slice(),
        ] {
            let path = root.path().join("invalid.patch");
            fs::write(&path, patch).expect("invalid patch fixture");
            assert_eq!(
                apply_exact_unified_diff(&path, root.path()),
                Err(BuildError::InvalidPatch)
            );
        }
    }

    #[test]
    fn exact_context_relocates_but_ambiguous_context_fails_closed() {
        let root = tempdir().expect("source root");
        fs::write(
            root.path().join("source.txt"),
            b"preface\none\ntwo\nthree\n",
        )
        .expect("offset source");
        let patch = root.path().join("offset.patch");
        fs::write(
            &patch,
            b"diff --git a/source.txt b/source.txt\n--- a/source.txt\n+++ b/source.txt\n@@ -1,3 +1,3 @@\n one\n-two\n+changed\n three\n",
        )
        .expect("offset patch");
        apply_exact_unified_diff(&patch, root.path()).expect("unique exact context");
        assert_eq!(
            fs::read(root.path().join("source.txt")).expect("relocated output"),
            b"preface\none\nchanged\nthree\n"
        );

        fs::write(
            root.path().join("source.txt"),
            b"one\ntwo\nthree\none\ntwo\nthree\n",
        )
        .expect("ambiguous source");
        assert_eq!(
            apply_exact_unified_diff(&patch, root.path()),
            Err(BuildError::InvalidPatch)
        );
    }
}
