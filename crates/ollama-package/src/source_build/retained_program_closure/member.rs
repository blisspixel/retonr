use std::{collections::BTreeMap, io::Read};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use crate::source_build::{
    RuntimeSourceBuildInputComponent, RuntimeSourceBuildInputManifest,
    RuntimeSourceBuildInputOpenError, RuntimeSourceBuildInputRole,
};

use super::RetainedProgramUpstreamClosureError;

const HASH_BUFFER_BYTES: usize = 64 * 1024;

pub(super) const RUST_CHANNEL: &str = "lineage/rust/channel-rust-1.97.1.toml";
pub(super) const RUST_CHANNEL_CHECKSUM: &str = "lineage/rust/channel-rust-1.97.1.toml.sha256";
pub(super) const RUST_CHANNEL_SIGNATURE: &str = "lineage/rust/channel-rust-1.97.1.toml.asc";
pub(super) const RUST_TRUST_ROOT: &str = "lineage/rust/rust-key.gpg.ascii";
pub(super) const ALPINE_MINIROOTFS: &str =
    "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz";
pub(super) const ALPINE_CHECKSUM: &str =
    "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.sha256";
pub(super) const ALPINE_SIGNATURE: &str =
    "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.asc";
pub(super) const ALPINE_TRUST_ROOT: &str = "lineage/host/alpine/ncopa.asc";
pub(super) const ALPINE_LIBGCC: &str = "lineage/host/alpine/libgcc-15.2.0-r2.apk";
pub(super) const ALPINE_BUSYBOX_APK: &str = "lineage/host/alpine/busybox-static-1.37.0-r30.apk";
pub(super) const BUSYBOX_EXECUTABLE: &str = "toolchains/busybox";

pub(super) struct MemberSpec {
    pub(super) path: &'static str,
    pub(super) name: &'static str,
    pub(super) revision: &'static str,
    pub(super) locator: &'static str,
    pub(super) roles: &'static [RuntimeSourceBuildInputRole],
    pub(super) size: u64,
    pub(super) digest: &'static str,
    pub(super) retain_bytes: bool,
}

macro_rules! member {
    ($path:expr, $name:expr, $revision:expr, $locator:expr, $roles:expr, $size:expr, $digest:expr, $retain:expr $(,)?) => {
        MemberSpec {
            path: $path,
            name: $name,
            revision: $revision,
            locator: $locator,
            roles: $roles,
            size: $size,
            digest: $digest,
            retain_bytes: $retain,
        }
    };
}

pub(super) const RUST_MEMBERS: [MemberSpec; 7] = [
    member!(
        "lineage/rust/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        "Rust Cargo musl host distribution",
        "1.97.1",
        "https://static.rust-lang.org/dist/2026-07-16/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        &[RuntimeSourceBuildInputRole::RustCargoDistribution],
        17_472_040,
        "d0aeadfea55964a8866014efbe08bcfd6225d68529d522a6eef300d4f8d5c9d2",
        false,
    ),
    member!(
        RUST_CHANNEL,
        "Rust channel manifest",
        "1.97.1",
        "https://static.rust-lang.org/dist/channel-rust-1.97.1.toml",
        &[RuntimeSourceBuildInputRole::RustChannelManifest],
        845_916,
        "03569b1886ceb5c05276b50c8431ab111de944cd6140fe1fa7d821dd8e0f29cf",
        true,
    ),
    member!(
        RUST_CHANNEL_SIGNATURE,
        "Rust channel manifest signature",
        "1.97.1",
        "https://static.rust-lang.org/dist/channel-rust-1.97.1.toml.asc",
        &[RuntimeSourceBuildInputRole::RustChannelManifestSignature],
        801,
        "14553bf89b963f1d1f0a92413b91510ed43f8d50c68fe665763747d815022017",
        true,
    ),
    member!(
        RUST_CHANNEL_CHECKSUM,
        "Rust channel manifest checksum",
        "1.97.1",
        "https://static.rust-lang.org/dist/channel-rust-1.97.1.toml.sha256",
        &[RuntimeSourceBuildInputRole::RustChannelManifestChecksum],
        91,
        "3cd57815146650e28e1756549952f655c683a6576a689fbbc7a5ee9d1c6ffab2",
        true,
    ),
    member!(
        RUST_TRUST_ROOT,
        "Rust release signing trust root",
        "108F66205EAEB0AAA8DD5E1C85AB96E6FA1BE5FE",
        "https://static.rust-lang.org/rust-key.gpg.ascii",
        &[RuntimeSourceBuildInputRole::RustReleaseTrustRoot],
        5_326,
        "e54b09a439647e006b4831eec9785cbaaf3e07ab371c3a6ee6a68e1bdb9fbc6b",
        true,
    ),
    member!(
        "lineage/rust/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        "Rust musl host and target standard library distribution",
        "1.97.1",
        "https://static.rust-lang.org/dist/2026-07-16/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        &[
            RuntimeSourceBuildInputRole::RustHostStandardLibraryDistribution,
            RuntimeSourceBuildInputRole::RustTargetStandardLibraryDistribution,
        ],
        67_584_421,
        "d160dfc81d21fdc72534859fae249fecf6ab70640375f64fc4e77005a48c18d0",
        false,
    ),
    member!(
        "lineage/rust/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        "Rust compiler musl host distribution",
        "1.97.1",
        "https://static.rust-lang.org/dist/2026-07-16/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        &[RuntimeSourceBuildInputRole::RustCompilerDistribution],
        172_143_184,
        "33a15df85ab0faf63b4c75b1113e47b41fd745a73bdc898c41034e9a9257b154",
        false,
    ),
];

pub(super) const ALPINE_MEMBERS: [MemberSpec; 7] = [
    member!(
        ALPINE_MINIROOTFS,
        "Alpine build-host minirootfs",
        "3.23.3",
        "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-minirootfs-3.23.3-x86_64.tar.gz",
        &[RuntimeSourceBuildInputRole::AlpineMinirootfs],
        3_713_234,
        "42d0e6d8de5521e7bf92e075e032b5690c1d948fa9775efa32a51a38b25460fb",
        true,
    ),
    member!(
        ALPINE_SIGNATURE,
        "Alpine build-host minirootfs signature",
        "3.23.3",
        "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-minirootfs-3.23.3-x86_64.tar.gz.asc",
        &[RuntimeSourceBuildInputRole::AlpineMinirootfsSignature],
        833,
        "7a4b882fa8cfb3b59d5346d14b02f0b11641bf2703f94b6a01bb806fe2b98405",
        true,
    ),
    member!(
        ALPINE_CHECKSUM,
        "Alpine build-host minirootfs checksum",
        "3.23.3",
        "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-minirootfs-3.23.3-x86_64.tar.gz.sha256",
        &[RuntimeSourceBuildInputRole::AlpineMinirootfsChecksum],
        105,
        "2eca4849bc42f82f2616b7d6c1ee38fa73a2d143acf0f6fb2092b068350b931f",
        true,
    ),
    member!(
        ALPINE_BUSYBOX_APK,
        "Alpine authenticated static BusyBox package",
        "1.37.0-r30",
        "https://dl-cdn.alpinelinux.org/alpine/v3.23/main/x86_64/busybox-static-1.37.0-r30.apk",
        &[RuntimeSourceBuildInputRole::AlpineBusyboxStaticPackage],
        640_317,
        "bb30365b954f531938a33de18173342185844828863121d7e821115d2f7f29c9",
        true,
    ),
    member!(
        ALPINE_LIBGCC,
        "Alpine build-host libgcc package",
        "15.2.0-r2",
        "https://dl-cdn.alpinelinux.org/alpine/v3.23/main/x86_64/libgcc-15.2.0-r2.apk",
        &[RuntimeSourceBuildInputRole::AlpineLibgccPackage],
        80_358,
        "4279d14cbf43311312d3a7d43619f4c15ede2e46d5608ba421ed157ded57c700",
        true,
    ),
    member!(
        BUSYBOX_EXECUTABLE,
        "Alpine BusyBox static shell",
        "1.37.0-r30",
        "pkg:apk/alpine/busybox-static@1.37.0-r30?arch=x86_64",
        &[
            RuntimeSourceBuildInputRole::NativePackage,
            RuntimeSourceBuildInputRole::PosixShell,
        ],
        1_034_600,
        "82bbbabec12a985ae58810cfe975c3399264dc888aa592d8e460732bdd30a8dd",
        true,
    ),
    member!(
        ALPINE_TRUST_ROOT,
        "Alpine release signing trust root",
        "0482D84022F52DF1C4E7CD43293ACD0907D9495A",
        "https://alpinelinux.org/keys/ncopa.asc",
        &[RuntimeSourceBuildInputRole::AlpineReleaseTrustRoot],
        3_092,
        "75a9a7e0cc35bfa946ce40c26133b3ed29a204fbd98a3b33331b659d927b3027",
        true,
    ),
];

pub(super) struct VerifiedMemberBytes(BTreeMap<&'static str, Vec<u8>>);

impl VerifiedMemberBytes {
    pub(super) fn required(
        &self,
        path: &'static str,
    ) -> Result<&[u8], RetainedProgramUpstreamClosureError> {
        self.0
            .get(path)
            .map(Vec::as_slice)
            .ok_or(RetainedProgramUpstreamClosureError::ManifestMismatch)
    }
}

pub(super) fn validate_manifest_members(
    manifest: &RuntimeSourceBuildInputManifest,
    specs: &[MemberSpec],
) -> Result<(), RetainedProgramUpstreamClosureError> {
    for spec in specs {
        let component = find_exact_component(manifest, spec)?;
        if component.relative_path().as_str() != spec.path
            || component.name() != spec.name
            || component.revision() != spec.revision
            || component.source_locator() != spec.locator
            || component.roles() != spec.roles
            || component.byte_size() != spec.size
            || component.digest().as_str() != spec.digest
        {
            return Err(RetainedProgramUpstreamClosureError::ManifestMismatch);
        }
    }
    Ok(())
}

fn find_exact_component<'a>(
    manifest: &'a RuntimeSourceBuildInputManifest,
    spec: &MemberSpec,
) -> Result<&'a RuntimeSourceBuildInputComponent, RetainedProgramUpstreamClosureError> {
    let first_role = spec
        .roles
        .first()
        .ok_or(RetainedProgramUpstreamClosureError::ManifestMismatch)?;
    let mut matches = manifest
        .components()
        .iter()
        .filter(|component| component.roles().contains(first_role));
    let component = matches
        .next()
        .ok_or(RetainedProgramUpstreamClosureError::ManifestMismatch)?;
    if matches.next().is_some() {
        return Err(RetainedProgramUpstreamClosureError::ManifestMismatch);
    }
    for role in spec.roles {
        let mut role_matches = manifest
            .components()
            .iter()
            .filter(|candidate| candidate.roles().contains(role));
        if role_matches.next() != Some(component) || role_matches.next().is_some() {
            return Err(RetainedProgramUpstreamClosureError::ManifestMismatch);
        }
    }
    Ok(component)
}

pub(super) fn read_and_verify_members<R, F, C>(
    manifest: &RuntimeSourceBuildInputManifest,
    specs: &[MemberSpec],
    open_component: &mut F,
    cancelled: &mut C,
) -> Result<VerifiedMemberBytes, RetainedProgramUpstreamClosureError>
where
    R: Read,
    F: FnMut(&ArtifactSetRelativePath) -> Result<R, RuntimeSourceBuildInputOpenError>,
    C: FnMut() -> bool,
{
    let mut retained = BTreeMap::new();
    for spec in specs {
        if cancelled() {
            return Err(RetainedProgramUpstreamClosureError::Cancelled);
        }
        let component = find_exact_component(manifest, spec)?;
        let stream = open_component(component.relative_path())
            .map_err(|_| RetainedProgramUpstreamClosureError::ComponentUnavailable)?;
        let bytes = read_and_verify(stream, spec, cancelled)?;
        if let Some(bytes) = bytes
            && retained.insert(spec.path, bytes).is_some()
        {
            return Err(RetainedProgramUpstreamClosureError::ManifestMismatch);
        }
    }
    Ok(VerifiedMemberBytes(retained))
}

fn read_and_verify<R, C>(
    mut stream: R,
    spec: &MemberSpec,
    cancelled: &mut C,
) -> Result<Option<Vec<u8>>, RetainedProgramUpstreamClosureError>
where
    R: Read,
    C: FnMut() -> bool,
{
    let capacity = spec
        .retain_bytes
        .then(|| usize::try_from(spec.size))
        .transpose()
        .map_err(|_| RetainedProgramUpstreamClosureError::ComponentMeasurementMismatch)?;
    let mut retained = capacity.map(Vec::with_capacity);
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES].into_boxed_slice();
    let mut remaining = spec.size;
    let mut hasher = Sha256::new();
    loop {
        if cancelled() {
            return Err(RetainedProgramUpstreamClosureError::Cancelled);
        }
        let read = stream
            .read(&mut buffer)
            .map_err(|_| RetainedProgramUpstreamClosureError::ComponentRead)?;
        if read == 0 {
            break;
        }
        let read_u64 =
            u64::try_from(read).map_err(|_| RetainedProgramUpstreamClosureError::ComponentRead)?;
        if read_u64 > remaining {
            return Err(RetainedProgramUpstreamClosureError::ComponentMeasurementMismatch);
        }
        hasher.update(&buffer[..read]);
        if let Some(bytes) = &mut retained {
            bytes.extend_from_slice(&buffer[..read]);
        }
        remaining -= read_u64;
    }
    let digest = format!("{:x}", hasher.finalize());
    if remaining != 0 || digest != spec.digest {
        return Err(RetainedProgramUpstreamClosureError::ComponentMeasurementMismatch);
    }
    Ok(retained)
}

pub(super) fn closure_id(domain: &[u8], specs: &[MemberSpec]) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for spec in specs {
        for value in [
            spec.path.as_bytes(),
            spec.name.as_bytes(),
            spec.revision.as_bytes(),
            spec.locator.as_bytes(),
            spec.digest.as_bytes(),
        ] {
            hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
            hasher.update(value);
        }
        hasher.update(spec.size.to_be_bytes());
        for role in spec.roles {
            hasher.update([*role as u8]);
        }
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .expect("SHA-256 formatting always produces one valid digest")
}
