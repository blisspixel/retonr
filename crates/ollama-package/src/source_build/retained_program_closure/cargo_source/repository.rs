use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{CargoSourceClosureError, CargoSourceClosureLimits};
use crate::source_build::retained_program_closure::cargo_source::{
    archive::TreeSnapshot,
    lockfile::{LockReview, PackageKey},
};

pub(super) fn verify_repository(
    snapshot: &TreeSnapshot,
    lock: &LockReview,
    supplied_lock: &[u8],
    limits: CargoSourceClosureLimits,
) -> Result<usize, CargoSourceClosureError> {
    let archived_lock = snapshot
        .captured
        .get("Cargo.lock")
        .ok_or(CargoSourceClosureError::RepositoryLockMismatch)?;
    if archived_lock != supplied_lock {
        return Err(CargoSourceClosureError::RepositoryLockMismatch);
    }
    let manifests = parse_manifests(snapshot, limits)?;
    let root = manifests
        .get("Cargo.toml")
        .ok_or(CargoSourceClosureError::RepositoryManifestMismatch)?;
    let workspace_version = workspace_version(root)?;
    let mut pending = workspace_members(root, limits)?;
    for target in dependency_paths("Cargo.toml", root, limits)? {
        pending.push_back(target);
    }
    let mut visited_paths = BTreeSet::new();
    let mut visited_packages = BTreeSet::new();
    while let Some(path) = pending.pop_front() {
        if !visited_paths.insert(path.clone()) {
            continue;
        }
        let manifest = manifests
            .get(&path)
            .ok_or(CargoSourceClosureError::RepositoryManifestMismatch)?;
        let package = package_key(manifest, workspace_version)?;
        if !lock.path_packages.contains(&package) || !visited_packages.insert(package) {
            return Err(CargoSourceClosureError::RepositoryManifestMismatch);
        }
        for target in dependency_paths(&path, manifest, limits)? {
            pending.push_back(target);
        }
    }
    if visited_packages != lock.path_packages {
        return Err(CargoSourceClosureError::RepositoryManifestMismatch);
    }
    Ok(visited_packages.len())
}

fn parse_manifests(
    snapshot: &TreeSnapshot,
    limits: CargoSourceClosureLimits,
) -> Result<BTreeMap<String, toml::Value>, CargoSourceClosureError> {
    let mut manifests = BTreeMap::new();
    for (path, bytes) in &snapshot.captured {
        if path == "Cargo.toml" || path.ends_with("/Cargo.toml") {
            if bytes.len() > limits.maximum_manifest_bytes {
                return Err(CargoSourceClosureError::RepositoryManifestMismatch);
            }
            let text = std::str::from_utf8(bytes)
                .map_err(|_| CargoSourceClosureError::RepositoryManifestMismatch)?;
            let value = toml::from_str::<toml::Value>(text)
                .map_err(|_| CargoSourceClosureError::RepositoryManifestMismatch)?;
            manifests.insert(path.clone(), value);
        }
    }
    Ok(manifests)
}

fn workspace_version(root: &toml::Value) -> Result<&str, CargoSourceClosureError> {
    root.get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .ok_or(CargoSourceClosureError::RepositoryManifestMismatch)
}

fn workspace_members(
    root: &toml::Value,
    limits: CargoSourceClosureLimits,
) -> Result<VecDeque<String>, CargoSourceClosureError> {
    let members = root
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .ok_or(CargoSourceClosureError::RepositoryManifestMismatch)?;
    if members.is_empty() || members.len() > limits.maximum_path_packages {
        return Err(CargoSourceClosureError::RepositoryManifestMismatch);
    }
    members
        .iter()
        .map(|member| {
            let path = member
                .as_str()
                .ok_or(CargoSourceClosureError::RepositoryManifestMismatch)?;
            if path.contains(['*', '?', '[', ']']) {
                return Err(CargoSourceClosureError::RepositoryManifestMismatch);
            }
            normalize_manifest_path("Cargo.toml", path, limits.maximum_path_bytes)
        })
        .collect()
}

fn package_key(
    manifest: &toml::Value,
    workspace_version: &str,
) -> Result<PackageKey, CargoSourceClosureError> {
    let package = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or(CargoSourceClosureError::RepositoryManifestMismatch)?;
    let name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or(CargoSourceClosureError::RepositoryManifestMismatch)?;
    let version = match package.get("version") {
        Some(toml::Value::String(version)) => version.as_str(),
        Some(value) if value.get("workspace").and_then(toml::Value::as_bool) == Some(true) => {
            workspace_version
        }
        _ => return Err(CargoSourceClosureError::RepositoryManifestMismatch),
    };
    Ok(PackageKey {
        name: name.to_owned(),
        source: None,
        version: version.to_owned(),
    })
}

fn dependency_paths(
    manifest_path: &str,
    manifest: &toml::Value,
    limits: CargoSourceClosureLimits,
) -> Result<Vec<String>, CargoSourceClosureError> {
    let mut paths = Vec::new();
    collect_dependency_paths(manifest, false, &mut |path| {
        if paths.len() >= limits.maximum_dependency_edges {
            return Err(CargoSourceClosureError::LockfileQuotaExceeded);
        }
        paths.push(normalize_manifest_path(
            manifest_path,
            path,
            limits.maximum_path_bytes,
        )?);
        Ok(())
    })?;
    Ok(paths)
}

fn collect_dependency_paths<F>(
    value: &toml::Value,
    dependency_table: bool,
    visit: &mut F,
) -> Result<(), CargoSourceClosureError>
where
    F: FnMut(&str) -> Result<(), CargoSourceClosureError>,
{
    let Some(table) = value.as_table() else {
        return Ok(());
    };
    if dependency_table {
        for dependency in table.values() {
            if let Some(path) = dependency.get("path").and_then(toml::Value::as_str) {
                visit(path)?;
            }
        }
        return Ok(());
    }
    for (name, child) in table {
        let is_dependencies = matches!(
            name.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        );
        collect_dependency_paths(child, is_dependencies, visit)?;
    }
    Ok(())
}

fn normalize_manifest_path(
    manifest_path: &str,
    dependency_path: &str,
    maximum: usize,
) -> Result<String, CargoSourceClosureError> {
    if dependency_path.is_empty()
        || dependency_path.len() > maximum
        || !dependency_path.is_ascii()
        || dependency_path.starts_with('/')
        || dependency_path.contains(['\\', ':'])
        || dependency_path.chars().any(char::is_control)
    {
        return Err(CargoSourceClosureError::PathPackageEscape);
    }
    let mut components = manifest_path.split('/').collect::<Vec<_>>();
    components.pop();
    for component in dependency_path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components
                    .pop()
                    .ok_or(CargoSourceClosureError::PathPackageEscape)?;
            }
            component => components.push(component),
        }
    }
    components.push("Cargo.toml");
    let normalized = components.join("/");
    super::archive::validate_portable_relative_path(&normalized, maximum)?;
    Ok(normalized)
}
