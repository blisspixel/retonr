use rewrite_model::{
    ActivationId, ArtifactId, ArtifactRole, InstalledArtifact, QualificationInvalidation,
};
use rewrite_types::Digest;
use tempfile::tempdir;

use super::{ArtifactStateStore, fixture, populate, qualification_id};
use crate::StoreError;

#[test]
fn lists_validated_installations_in_identity_order() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let first = fixture();
    let mut second = fixture();
    let second_digest = Digest::sha256(b"second artifact");
    second.manifest.artifact_id = ArtifactId::from_digest(second_digest.clone());
    second.manifest.artifact_digest = second_digest.clone();
    second.manifest.byte_size = 15;
    second.installed = InstalledArtifact {
        artifact_id: second.manifest.artifact_id.clone(),
        artifact_digest: second_digest,
        byte_size: 15,
        storage_key: "artifacts/second.gguf".to_owned(),
    };
    for item in [&second, &first] {
        store
            .put_installation(&item.manifest, &item.installed)
            .expect("register installation");
    }

    let installed = store.artifact_inventory(2).expect("list installations");
    assert_eq!(installed.len(), 2);
    assert!(
        installed[0].manifest.artifact_id.digest().as_str()
            < installed[1].manifest.artifact_id.digest().as_str()
    );
    assert!(installed.iter().any(|item| {
        item.manifest == first.manifest
            && item.installed.as_ref().map(|value| &value.installed) == Some(&first.installed)
    }));
    assert!(installed.iter().any(|item| {
        item.manifest == second.manifest
            && item.installed.as_ref().map(|value| &value.installed) == Some(&second.installed)
    }));
}

#[test]
fn rejects_installation_record_that_disagrees_with_its_index() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let value = fixture();
    store
        .put_installation(&value.manifest, &value.installed)
        .expect("register installation");
    let mut changed = value.installed;
    let changed_digest = Digest::sha256(b"changed identity");
    changed.artifact_id = ArtifactId::from_digest(changed_digest.clone());
    changed.artifact_digest = changed_digest;
    let encoded = serde_json::to_string(&changed).expect("encode changed installation");
    store
        .connection()
        .execute("UPDATE installed_artifacts SET record_json = ?1", [encoded])
        .expect("corrupt installed record");

    assert!(matches!(
        store.artifact_inventory(1),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn rejects_installation_that_disagrees_with_its_manifest() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let value = fixture();
    store
        .put_installation(&value.manifest, &value.installed)
        .expect("register installation");
    let mut changed = value.installed;
    changed.byte_size += 1;
    let encoded = serde_json::to_string(&changed).expect("encode changed installation");
    store
        .connection()
        .execute("UPDATE installed_artifacts SET record_json = ?1", [encoded])
        .expect("corrupt installed size");

    assert!(matches!(
        store.artifact_inventory(1),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn rejects_invalid_and_exceeded_inventory_limits() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let value = fixture();
    store
        .put_installation(&value.manifest, &value.installed)
        .expect("register installation");

    assert!(matches!(
        store.artifact_inventory(0),
        Err(StoreError::InvalidLimit)
    ));
    let mut second = value;
    let digest = Digest::sha256(b"second bounded artifact");
    second.manifest.artifact_id = ArtifactId::from_digest(digest.clone());
    second.manifest.artifact_digest = digest.clone();
    second.installed.artifact_id = ArtifactId::from_digest(digest.clone());
    second.installed.artifact_digest = digest;
    store
        .put_installation(&second.manifest, &second.installed)
        .expect("register second installation");
    assert!(matches!(
        store.artifact_inventory(1),
        Err(StoreError::InventoryLimitExceeded)
    ));
}

#[test]
fn includes_only_fully_validated_active_bindings() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let value = fixture();
    populate(&mut store, &value);
    let binding = store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"inventory activation")),
            ArtifactRole::Generation,
            &value.installed,
            &qualification_id(&value),
        )
        .expect("activate fixture");

    let inventory = store.artifact_inventory(1).expect("read inventory");
    assert_eq!(inventory[0].active_bindings, vec![binding]);
}

#[test]
fn returns_multiple_bindings_in_domain_role_order() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let mut value = fixture();
    value.manifest.declared_capabilities.roles =
        vec![ArtifactRole::Generation, ArtifactRole::Embedding];
    value.qualification.supported_roles = vec![ArtifactRole::Generation, ArtifactRole::Embedding];
    populate(&mut store, &value);
    for (role, label) in [
        (ArtifactRole::Embedding, b"embedding".as_slice()),
        (ArtifactRole::Generation, b"generation".as_slice()),
    ] {
        store
            .activate(
                ActivationId::from_digest(Digest::sha256(label)),
                role,
                &value.installed,
                &qualification_id(&value),
            )
            .expect("activate role");
    }

    let inventory = store.artifact_inventory(1).expect("read inventory");
    let roles: Vec<_> = inventory[0]
        .active_bindings
        .iter()
        .map(|binding| binding.role)
        .collect();
    assert_eq!(
        roles,
        vec![ArtifactRole::Generation, ArtifactRole::Embedding]
    );
}

#[test]
fn any_invalidation_rejects_inventory_binding_with_bounded_lookup() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let value = fixture();
    populate(&mut store, &value);
    let binding = store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"bounded invalidation activation")),
            ArtifactRole::Generation,
            &value.installed,
            &qualification_id(&value),
        )
        .expect("activate fixture");
    for sequence in 0..128 {
        let invalidation = QualificationInvalidation {
            qualification_id: binding.qualification_id.clone(),
            reason_code: format!("invalid_{sequence}"),
        };
        store
            .connection()
            .execute(
                "INSERT INTO qualification_invalidations
                    (qualification_id, reason_code, record_json)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![
                    invalidation.qualification_id.digest().as_str(),
                    &invalidation.reason_code,
                    serde_json::to_string(&invalidation).expect("encode invalidation")
                ],
            )
            .expect("insert invalidation fixture");
    }

    assert!(matches!(
        store.artifact_inventory(1),
        Err(StoreError::InvalidActiveBinding)
    ));
}

#[test]
fn includes_manifest_only_state_without_claiming_installation() {
    let directory = tempdir().expect("temporary directory");
    let store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let value = fixture();
    store
        .put_manifest(&value.manifest)
        .expect("store manifest only");

    let inventory = store.artifact_inventory(1).expect("read inventory");
    assert_eq!(inventory.len(), 1);
    assert_eq!(inventory[0].manifest, value.manifest);
    assert_eq!(inventory[0].installed, None);
    assert!(inventory[0].active_bindings.is_empty());
}
