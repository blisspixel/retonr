use super::*;

#[test]
fn verifies_imported_bytes_and_emits_content_free_progress() {
    let (directory, mut store) = initialized();
    let value = import_bytes(&directory, &mut store, b"verified artifact");
    let service = ArtifactInventoryService::open(storage(&directory), &store, limits())
        .expect("open inventory");
    let mut progress = Vec::new();

    let report = service
        .inventory(&CancellationToken::new(), |item| progress.push(item))
        .expect("verify imported artifact");

    assert_eq!(report.registered.len(), 1);
    assert_eq!(report.registered[0].manifest, value);
    assert_eq!(
        report.registered[0].bytes,
        RegisteredArtifactBytes::Verified
    );
    assert_eq!(report.verified_bytes, 17);
    assert_eq!(
        progress.last().map(|item| item.stage),
        Some(ArtifactInventoryStage::RecheckingStorageAndState)
    );
}

#[test]
fn progress_counts_state_and_uninstalled_entries_exactly_once() {
    let (directory, mut store) = initialized();
    let installed = import_bytes(&directory, &mut store, b"installed bytes");
    let manifest_only = manifest(b"manifest orphan", "manifest-orphan");
    store
        .put_manifest(&manifest_only)
        .expect("store manifest-only state");
    write_artifact(
        &directory,
        &manifest_only.artifact_digest,
        b"manifest orphan",
    );
    let unrelated = manifest(b"unrelated orphan", "unrelated-orphan");
    write_artifact(&directory, &unrelated.artifact_digest, b"unrelated orphan");
    let service = ArtifactInventoryService::open(storage(&directory), &store, limits())
        .expect("open progress inventory");
    let mut progress = Vec::new();

    service
        .inventory(&CancellationToken::new(), |item| progress.push(item))
        .expect("inspect progress fixtures");

    let stages: Vec<_> = progress.iter().map(|item| item.stage).collect();
    assert_eq!(
        stages,
        vec![
            ArtifactInventoryStage::OpeningStorage,
            ArtifactInventoryStage::LoadingState,
            ArtifactInventoryStage::FreezingStorage,
            ArtifactInventoryStage::InspectingState,
            ArtifactInventoryStage::InspectingState,
            ArtifactInventoryStage::VerifyingUninstalled,
            ArtifactInventoryStage::VerifyingUninstalled,
            ArtifactInventoryStage::RecheckingStorageAndState,
        ]
    );
    let counts: Vec<_> = progress.iter().map(|item| item.completed_entries).collect();
    assert_eq!(counts, vec![0, 0, 0, 1, 2, 3, 4, 4]);
    assert!(
        progress
            .windows(2)
            .all(|pair| pair[0].verified_bytes <= pair[1].verified_bytes)
    );
    let expected_bytes = installed.byte_size + manifest_only.byte_size + unrelated.byte_size;
    assert_eq!(
        progress.last().map(|item| item.verified_bytes),
        Some(expected_bytes)
    );
}

#[test]
fn associates_manifest_only_state_with_verified_orphans() {
    let (directory, store) = initialized();
    let matching = manifest(b"matching orphan", "matching");
    let mut size_conflict = manifest(b"size conflict orphan", "size-conflict");
    size_conflict.byte_size += 1;
    store
        .put_manifest(&matching)
        .expect("store matching manifest");
    store
        .put_manifest(&size_conflict)
        .expect("store size-conflicting manifest");
    write_artifact(&directory, &matching.artifact_digest, b"matching orphan");
    write_artifact(
        &directory,
        &size_conflict.artifact_digest,
        b"size conflict orphan",
    );
    let unrecorded = manifest(b"unrecorded orphan", "unrecorded");
    write_artifact(
        &directory,
        &unrecorded.artifact_digest,
        b"unrecorded orphan",
    );

    let service = ArtifactInventoryService::open(storage(&directory), &store, limits())
        .expect("open orphan inventory");
    let mut progress = Vec::new();
    let report = service
        .inventory(&CancellationToken::new(), |item| progress.push(item))
        .expect("inspect orphan fixtures");

    assert_eq!(report.manifest_only.len(), 2);
    assert_eq!(report.verified_orphans.len(), 3);
    assert_eq!(progress.last().map(|item| item.completed_entries), Some(5));
    assert!(report.verified_orphans.iter().any(|item| {
        matches!(
            &item.manifest,
            OrphanManifestAssociation::MatchingManifest(value) if value == &matching
        )
    }));
    assert!(report.verified_orphans.iter().any(|item| {
        matches!(
            &item.manifest,
            OrphanManifestAssociation::ManifestSizeConflict { manifest }
                if manifest == &size_conflict
        )
    }));
    assert!(report.verified_orphans.iter().any(|item| {
        item.artifact_id == unrecorded.artifact_id
            && item.manifest == OrphanManifestAssociation::NoManifest
    }));
}
