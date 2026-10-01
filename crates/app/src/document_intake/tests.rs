use super::*;
use crate::document_selection::RelativeDocumentPath;
use std::{fs, path::PathBuf};
use tempfile::tempdir;

fn original() -> Vec<u8> {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../../fixtures/cli/retained-input.json"))
            .expect("read fixture");
    fixture["original"]
        .as_str()
        .expect("original")
        .as_bytes()
        .to_vec()
}

#[test]
fn explicit_and_catalog_reads_keep_exact_bytes_and_full_inventory() {
    let root = tempdir().expect("root");
    let path = root.path().join("draft.txt");
    let bytes = original();
    fs::write(&path, &bytes).expect("source");
    let token = CancellationToken::new();
    let explicit = DocumentSelection::explicit(&path);
    let result = DocumentIntakeService::read(&explicit, bytes.len(), &token).expect("explicit");
    assert_eq!(result.bytes, bytes);
    assert_eq!(result.observation.inventory.digest, Digest::sha256(&bytes));
    assert_eq!(
        result.observation.derivative,
        DerivativeDisposition::NotRequired
    );
    let catalog = DocumentSelection::catalog(
        root.path(),
        RelativeDocumentPath::new("draft.txt").expect("relative"),
        Digest::sha256(&bytes),
    );
    assert_eq!(
        DocumentIntakeService::read(&catalog, 100, &token)
            .expect("catalog")
            .bytes,
        bytes
    );
    fs::write(&path, b"new bytes").expect("changed catalog file");
    assert!(matches!(
        DocumentIntakeService::read(&catalog, 100, &token),
        Err(DocumentIntakeError::Changed)
    ));
}

#[test]
fn source_budget_is_enforced_before_inventory_and_original_cancel_wins_over_errors() {
    let root = tempdir().expect("root");
    let path = root.path().join("draft.txt");
    fs::write(&path, b"draft").expect("source");
    let selection = DocumentSelection::explicit(&path);
    let token = CancellationToken::new();
    assert!(
        matches!(DocumentIntakeService::read(&selection, 4, &token), Err(DocumentIntakeError::Input(error)) if error.kind() == io::ErrorKind::InvalidData)
    );
    assert!(matches!(
        read_with_post_read(&selection, 4, &token, || token.cancel()),
        Err(DocumentIntakeError::Cancelled)
    ));
    let token = CancellationToken::new();
    assert!(matches!(
        read_with_post_read(&selection, 5, &token, || token.cancel()),
        Err(DocumentIntakeError::Cancelled)
    ));
    assert!(matches!(
        DocumentIntakeService::read(
            &DocumentSelection::explicit("missing-private.txt"),
            100,
            &token
        ),
        Err(DocumentIntakeError::Cancelled)
    ));
    fs::write(&path, []).expect("empty");
    assert!(
        DocumentIntakeService::read(&selection, 0, &CancellationToken::new())
            .expect("zero empty")
            .bytes
            .is_empty()
    );
}

#[test]
fn requested_limit_cannot_expand_supported_plain_text_ceiling() {
    let root = tempdir().expect("root");
    let path = root.path().join("oversized.txt");
    let file = fs::File::create(&path).expect("source");
    file.set_len(u64::try_from(MAX_CANDIDATE_CHECK_BYTES + 1).expect("ceiling"))
        .expect("sparse over limit");
    assert!(
        matches!(DocumentIntakeService::read(&DocumentSelection::explicit(path), usize::MAX, &CancellationToken::new()), Err(DocumentIntakeError::Input(error)) if error.kind() == io::ErrorKind::InvalidData)
    );
}

#[test]
fn carrier_encoding_and_sidecars_are_independent_typed_observations() {
    let token = CancellationToken::new();
    let plain = DocumentIntakeService::inspect_bytes(None, b"Hello\n", &token).expect("stream");
    assert_eq!(
        plain.sidecars.completeness,
        SidecarScanCompleteness::NotApplicable
    );
    assert_eq!(plain.derivative, DerivativeDisposition::NotRequired);
    for bytes in [b"\xff".as_slice(), b"\xff\xfeH\0".as_slice()] {
        assert_eq!(
            DocumentIntakeService::inspect_bytes(None, bytes, &token)
                .expect("unsupported encoding inventory")
                .derivative,
            DerivativeDisposition::NotChecked
        );
    }
    let carrier =
        DocumentIntakeService::inspect_bytes(None, "\u{feff}\u{fe0f}Hello\n".as_bytes(), &token)
            .expect("carrier");
    assert_eq!(
        carrier.inventory.c2pa_unstructured_text,
        CarrierPresence::Possible
    );
    assert_eq!(
        carrier.derivative,
        DerivativeDisposition::ExplicitDecisionRequired
    );
    let root = tempdir().expect("root");
    let source = root.path().join("draft.txt");
    fs::write(&source, b"Hello\n").expect("source");
    for kind in [SidecarKind::C2pa, SidecarKind::Xmp] {
        let mut adjacent = source.as_os_str().to_os_string();
        adjacent.push(kind.suffix());
        let path = PathBuf::from(adjacent);
        fs::write(&path, b"metadata").expect("sidecar");
        let observation = DocumentIntakeService::inspect_bytes(Some(&source), b"Hello\n", &token)
            .expect("sidecar observation");
        assert_eq!(observation.sidecars.present, vec![kind]);
        assert_eq!(
            observation.sidecars.completeness,
            SidecarScanCompleteness::Complete
        );
        assert_eq!(
            observation.derivative,
            DerivativeDisposition::ExplicitDecisionRequired
        );
        fs::remove_file(path).expect("remove sidecar");
    }
}

#[test]
fn existing_sidecar_directory_is_excluded_without_following_or_decoding_it() {
    let root = tempdir().expect("root");
    let source = root.path().join("draft.txt");
    fs::create_dir(root.path().join("draft.txt.c2pa")).expect("metadata directory");
    let observation =
        DocumentIntakeService::inspect_bytes(Some(&source), b"Hello", &CancellationToken::new())
            .expect("inventory");
    assert!(observation.sidecars.present.is_empty());
    assert_eq!(
        observation.sidecars.completeness,
        SidecarScanCompleteness::Complete
    );
    assert_eq!(observation.derivative, DerivativeDisposition::NotRequired);
}

#[test]
fn real_overlong_sidecar_lookup_cannot_look_like_complete_absence() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../fixtures/cli/sidecar-scan-incomplete.json"
    ))
    .expect("fixture");
    let root = tempdir().expect("root");
    let name = "a".repeat(
        usize::try_from(fixture["filename_bytes"].as_u64().expect("length")).expect("length"),
    );
    let source = root.path().join(name);
    let bytes = fixture["document"].as_str().expect("document").as_bytes();
    fs::write(&source, bytes).expect("maximum legal component source");
    let result = DocumentIntakeService::read(
        &DocumentSelection::explicit(&source),
        100,
        &CancellationToken::new(),
    )
    .expect("retained source with incomplete metadata");
    assert_eq!(result.bytes, bytes);
    assert_eq!(
        result.observation.sidecars.completeness,
        SidecarScanCompleteness::Incomplete
    );
    assert!(result.observation.sidecars.present.is_empty());
    assert_eq!(
        result.observation.derivative,
        DerivativeDisposition::ExplicitDecisionRequired
    );
    assert_eq!(fs::read(source).expect("unchanged"), bytes);
}

#[test]
fn failures_and_complete_observations_do_not_debug_document_content() {
    for error in [
        DocumentIntakeError::Input(io::Error::other("private source text")),
        DocumentIntakeError::Changed,
        DocumentIntakeError::Cancelled,
    ] {
        assert!(!format!("{error:?} {error}").contains("private source"));
    }
    let observation = DocumentIntakeService::inspect_bytes(
        None,
        b"private source text",
        &CancellationToken::new(),
    )
    .expect("observation");
    let selected = SelectedDocument {
        bytes: b"private source text".to_vec(),
        observation,
    };
    assert!(!format!("{selected:?}").contains("private source"));
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        DocumentIntakeService::inspect_bytes(None, &[], &token),
        Err(DocumentIntakeError::Cancelled)
    ));
}

#[cfg(unix)]
#[test]
fn explicit_link_policy_and_catalog_alias_refusals_remain_distinct() {
    use std::os::unix::fs::symlink;
    let root = tempdir().expect("root");
    let source = root.path().join("draft.txt");
    fs::write(&source, b"Hello").expect("source");
    symlink(&source, root.path().join("link.txt")).expect("link");
    let token = CancellationToken::new();
    assert_eq!(
        DocumentIntakeService::read(
            &DocumentSelection::explicit(root.path().join("link.txt")),
            100,
            &token
        )
        .expect("explicit link")
        .bytes,
        b"Hello"
    );
    let relative = RelativeDocumentPath::new("link.txt").expect("relative");
    assert!(
        DocumentIntakeService::read(
            &DocumentSelection::catalog(root.path(), relative, Digest::sha256(b"Hello")),
            100,
            &token
        )
        .is_err()
    );
    fs::hard_link(&source, root.path().join("hard.txt")).expect("hard link");
    assert!(
        DocumentIntakeService::read(
            &DocumentSelection::catalog(
                root.path(),
                RelativeDocumentPath::new("draft.txt").expect("relative"),
                Digest::sha256(b"Hello")
            ),
            100,
            &token
        )
        .is_err()
    );
    assert_eq!(fs::read(source).expect("unchanged source"), b"Hello");
}

#[test]
fn already_retained_bytes_cannot_bypass_inventory_ceiling_or_missing_path_disclosure() {
    let bytes = vec![b'a'; MAX_CANDIDATE_CHECK_BYTES + 1];
    let error = DocumentIntakeService::inspect_bytes(None, &bytes, &CancellationToken::new())
        .expect_err("byte inventory ceiling");
    assert!(matches!(error, DocumentIntakeError::Inspection(_)));
    assert_eq!(format!("{error:?}"), "DocumentIntakeError::Inspection");
    let observation = DocumentIntakeService::inspect_bytes(
        Some(Path::new("/")),
        b"Hello",
        &CancellationToken::new(),
    )
    .expect("unusable metadata selection");
    assert_eq!(
        observation.sidecars.completeness,
        SidecarScanCompleteness::Incomplete
    );
    assert_eq!(
        observation.derivative,
        DerivativeDisposition::ExplicitDecisionRequired
    );
}
