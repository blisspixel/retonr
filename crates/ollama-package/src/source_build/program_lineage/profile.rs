use rewrite_types::Digest;

use crate::source_build::{
    RuntimeSourceBuildInputError, RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputRole,
};

pub(in crate::source_build) struct ExpectedComponent {
    pub(in crate::source_build) path: &'static str,
    pub(in crate::source_build) name: &'static str,
    pub(in crate::source_build) revision: &'static str,
    pub(in crate::source_build) locator: &'static str,
    pub(in crate::source_build) roles: &'static [RuntimeSourceBuildInputRole],
    pub(in crate::source_build) official: Option<(u64, &'static str)>,
}

pub(super) fn validate(
    manifest: &RuntimeSourceBuildInputManifest,
) -> Result<(), RuntimeSourceBuildInputError> {
    if manifest.components().len() != PRODUCTION_COMPONENTS.len() {
        return Err(RuntimeSourceBuildInputError::ProgramLineageMismatch);
    }
    for (component, expected) in manifest.components().iter().zip(PRODUCTION_COMPONENTS) {
        if component.relative_path().as_str() != expected.path
            || component.name() != expected.name
            || component.revision() != expected.revision
            || component.source_locator() != expected.locator
            || component.roles() != expected.roles
        {
            return Err(RuntimeSourceBuildInputError::ProgramLineageMismatch);
        }
        if let Some((byte_size, digest)) = expected.official
            && (component.byte_size() != byte_size
                || component.digest()
                    != &Digest::from_sha256_hex(digest)
                        .map_err(|_| RuntimeSourceBuildInputError::InvalidProgramLineage)?)
        {
            return Err(RuntimeSourceBuildInputError::ProgramLineageMismatch);
        }
    }
    Ok(())
}

const fn component(
    path: &'static str,
    name: &'static str,
    revision: &'static str,
    locator: &'static str,
    roles: &'static [RuntimeSourceBuildInputRole],
) -> ExpectedComponent {
    ExpectedComponent {
        path,
        name,
        revision,
        locator,
        roles,
        official: None,
    }
}

const fn official(
    path: &'static str,
    name: &'static str,
    revision: &'static str,
    locator: &'static str,
    roles: &'static [RuntimeSourceBuildInputRole],
    byte_size: u64,
    digest: &'static str,
) -> ExpectedComponent {
    ExpectedComponent {
        path,
        name,
        revision,
        locator,
        roles,
        official: Some((byte_size, digest)),
    }
}

pub(in crate::source_build) const PRODUCTION_COMPONENTS: [ExpectedComponent; 38] = [
    component(
        "helper/isolation",
        "Retonr isolation helper",
        "workspace-candidate-v3-rust-lld",
        "urn:retonr:runtime-isolation-helper:workspace-candidate-v3-rust-lld",
        &[RuntimeSourceBuildInputRole::IsolationHelper],
    ),
    component(
        "legal/licenses.json",
        "source build license evidence",
        "review-v2",
        "urn:retonr:runtime-source-build:licenses:review-v2",
        &[RuntimeSourceBuildInputRole::LicenseEvidence],
    ),
    component(
        "lineage/build-recipe-v2.json",
        "retained program build recipe",
        "v2",
        "urn:retonr:runtime-source-build:retained-program-build:v2",
        &[RuntimeSourceBuildInputRole::CanonicalBuildRecipe],
    ),
    official(
        "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz",
        "Alpine build-host minirootfs",
        "3.23.3",
        "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-minirootfs-3.23.3-x86_64.tar.gz",
        &[RuntimeSourceBuildInputRole::AlpineMinirootfs],
        3_713_234,
        "42d0e6d8de5521e7bf92e075e032b5690c1d948fa9775efa32a51a38b25460fb",
    ),
    official(
        "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.asc",
        "Alpine build-host minirootfs signature",
        "3.23.3",
        "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-minirootfs-3.23.3-x86_64.tar.gz.asc",
        &[RuntimeSourceBuildInputRole::AlpineMinirootfsSignature],
        833,
        "7a4b882fa8cfb3b59d5346d14b02f0b11641bf2703f94b6a01bb806fe2b98405",
    ),
    official(
        "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.sha256",
        "Alpine build-host minirootfs checksum",
        "3.23.3",
        "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-minirootfs-3.23.3-x86_64.tar.gz.sha256",
        &[RuntimeSourceBuildInputRole::AlpineMinirootfsChecksum],
        105,
        "2eca4849bc42f82f2616b7d6c1ee38fa73a2d143acf0f6fb2092b068350b931f",
    ),
    official(
        "lineage/host/alpine/busybox-static-1.37.0-r30.apk",
        "Alpine authenticated static BusyBox package",
        "1.37.0-r30",
        "https://dl-cdn.alpinelinux.org/alpine/v3.23/main/x86_64/busybox-static-1.37.0-r30.apk",
        &[RuntimeSourceBuildInputRole::AlpineBusyboxStaticPackage],
        640_317,
        "bb30365b954f531938a33de18173342185844828863121d7e821115d2f7f29c9",
    ),
    official(
        "lineage/host/alpine/libgcc-15.2.0-r2.apk",
        "Alpine build-host libgcc package",
        "15.2.0-r2",
        "https://dl-cdn.alpinelinux.org/alpine/v3.23/main/x86_64/libgcc-15.2.0-r2.apk",
        &[RuntimeSourceBuildInputRole::AlpineLibgccPackage],
        80_358,
        "4279d14cbf43311312d3a7d43619f4c15ede2e46d5608ba421ed157ded57c700",
    ),
    official(
        "lineage/host/alpine/ncopa.asc",
        "Alpine release signing trust root",
        "0482D84022F52DF1C4E7CD43293ACD0907D9495A",
        "https://alpinelinux.org/keys/ncopa.asc",
        &[RuntimeSourceBuildInputRole::AlpineReleaseTrustRoot],
        3_092,
        "75a9a7e0cc35bfa946ce40c26133b3ed29a204fbd98a3b33331b659d927b3027",
    ),
    official(
        "lineage/rust/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        "Rust Cargo musl host distribution",
        "1.97.1",
        "https://static.rust-lang.org/dist/2026-07-16/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        &[RuntimeSourceBuildInputRole::RustCargoDistribution],
        17_472_040,
        "d0aeadfea55964a8866014efbe08bcfd6225d68529d522a6eef300d4f8d5c9d2",
    ),
    official(
        "lineage/rust/channel-rust-1.97.1.toml",
        "Rust channel manifest",
        "1.97.1",
        "https://static.rust-lang.org/dist/channel-rust-1.97.1.toml",
        &[RuntimeSourceBuildInputRole::RustChannelManifest],
        845_916,
        "03569b1886ceb5c05276b50c8431ab111de944cd6140fe1fa7d821dd8e0f29cf",
    ),
    official(
        "lineage/rust/channel-rust-1.97.1.toml.asc",
        "Rust channel manifest signature",
        "1.97.1",
        "https://static.rust-lang.org/dist/channel-rust-1.97.1.toml.asc",
        &[RuntimeSourceBuildInputRole::RustChannelManifestSignature],
        801,
        "14553bf89b963f1d1f0a92413b91510ed43f8d50c68fe665763747d815022017",
    ),
    official(
        "lineage/rust/channel-rust-1.97.1.toml.sha256",
        "Rust channel manifest checksum",
        "1.97.1",
        "https://static.rust-lang.org/dist/channel-rust-1.97.1.toml.sha256",
        &[RuntimeSourceBuildInputRole::RustChannelManifestChecksum],
        91,
        "3cd57815146650e28e1756549952f655c683a6576a689fbbc7a5ee9d1c6ffab2",
    ),
    official(
        "lineage/rust/rust-key.gpg.ascii",
        "Rust release signing trust root",
        "108F66205EAEB0AAA8DD5E1C85AB96E6FA1BE5FE",
        "https://static.rust-lang.org/rust-key.gpg.ascii",
        &[RuntimeSourceBuildInputRole::RustReleaseTrustRoot],
        5_326,
        "e54b09a439647e006b4831eec9785cbaaf3e07ab371c3a6ee6a68e1bdb9fbc6b",
    ),
    official(
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
    ),
    official(
        "lineage/rust/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        "Rust compiler musl host distribution",
        "1.97.1",
        "https://static.rust-lang.org/dist/2026-07-16/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz",
        &[RuntimeSourceBuildInputRole::RustCompilerDistribution],
        172_143_184,
        "33a15df85ab0faf63b4c75b1113e47b41fd745a73bdc898c41034e9a9257b154",
    ),
    component(
        "lineage/source/Cargo.lock",
        "Retonr Cargo lockfile",
        "workspace-candidate-v1",
        "urn:retonr:cargo-lock:workspace-candidate-v1",
        &[RuntimeSourceBuildInputRole::CargoLockfile],
    ),
    component(
        "lineage/source/cargo-crates.tar",
        "Retonr raw Cargo crate sources",
        "workspace-candidate-v1",
        "urn:retonr:cargo-crates:workspace-candidate-v1",
        &[RuntimeSourceBuildInputRole::CargoRawCrateSource],
    ),
    component(
        "lineage/source/cargo-vendor.tar",
        "Retonr vendored Cargo sources",
        "workspace-candidate-v1",
        "urn:retonr:cargo-vendor:workspace-candidate-v1",
        &[RuntimeSourceBuildInputRole::CargoVendorSource],
    ),
    component(
        "lineage/source/retonr-source.tar",
        "Retonr repository source",
        "workspace-candidate-v1",
        "urn:retonr:repository-source:workspace-candidate-v1",
        &[RuntimeSourceBuildInputRole::RetonrRepositorySource],
    ),
    component(
        "lineage/tools/rewrite-runtime-source-archive",
        "Retonr source archive preparation tool",
        "workspace-candidate-v1-rust-lld",
        "urn:retonr:runtime-source-archive:workspace-candidate-v1-rust-lld",
        &[RuntimeSourceBuildInputRole::SourceArchivePreparationTool],
    ),
    component(
        "lineage/tools/rewrite-runtime-source-manifest",
        "Retonr source manifest preparation tool",
        "workspace-candidate-v1-rust-lld",
        "urn:retonr:runtime-source-manifest:workspace-candidate-v1-rust-lld",
        &[RuntimeSourceBuildInputRole::SourceManifestPreparationTool],
    ),
    component(
        "metadata/build-parameters.json",
        "controlled build parameters",
        "v1",
        "urn:retonr:runtime-source-build:parameters:v1",
        &[RuntimeSourceBuildInputRole::BuildParameters],
    ),
    component(
        "metadata/retained-program-lineage.json",
        "retained program lineage",
        "v2",
        "urn:retonr:runtime-source-build:retained-program-lineage:v2",
        &[RuntimeSourceBuildInputRole::RetainedProgramLineage],
    ),
    component(
        "metadata/source-provenance.json",
        "Ollama source provenance",
        "v1",
        "urn:retonr:runtime-source-build:source-provenance:v1",
        &[RuntimeSourceBuildInputRole::SourceProvenance],
    ),
    component(
        "metadata/tool-evidence.json",
        "controlled build tool evidence",
        "v1",
        "urn:retonr:runtime-source-build:tool-evidence:v1",
        &[RuntimeSourceBuildInputRole::ToolEvidence],
    ),
    component(
        "modules/go-checksum-set.json",
        "Ollama Go checksum set",
        "v0.32.15",
        "urn:retonr:ollama:go-checksum-set:v0.32.15",
        &[RuntimeSourceBuildInputRole::GoChecksumSet],
    ),
    component(
        "modules/go-module-cache.tar",
        "Ollama Go module cache",
        "v0.32.15",
        "urn:retonr:ollama:go-module-cache:v0.32.15",
        &[RuntimeSourceBuildInputRole::GoModule],
    ),
    component(
        "patches/llama-cpp-zig-clang20-evex512.patch",
        "llama.cpp Zig Clang 20 EVEX-512 compatibility patch",
        "v1",
        "urn:retonr:runtime-source-build:llama-cpp-zig-clang20-evex512:v1",
        &[RuntimeSourceBuildInputRole::SourcePatch],
    ),
    component(
        "patches/ollama-mlx-reproducible-errors.patch",
        "Ollama MLX reproducible error patch",
        "v1",
        "urn:retonr:runtime-source-build:ollama-mlx-reproducible-errors:v1",
        &[RuntimeSourceBuildInputRole::SourcePatch],
    ),
    component(
        "scripts/build",
        "Retonr runtime source-build coordinator",
        "workspace-candidate-v1",
        "urn:retonr:runtime-source-builder:workspace-candidate-v1",
        &[RuntimeSourceBuildInputRole::BuildScript],
    ),
    component(
        "sources/llama-cpp.tar",
        "llama.cpp source",
        "9d77fa17254e1dee4b9e92504c91611a60b1359f",
        "https://github.com/ggml-org/llama.cpp/archive/refs/tags/b10488.tar.gz",
        &[RuntimeSourceBuildInputRole::LlamaCppSource],
    ),
    component(
        "sources/ollama.tar",
        "Ollama source",
        "b7871fc0d1d82fe109536efa3e0e8e411c766c75",
        "https://github.com/ollama/ollama/archive/refs/tags/v0.32.15.tar.gz",
        &[RuntimeSourceBuildInputRole::OllamaSource],
    ),
    official(
        "toolchains/busybox",
        "Alpine BusyBox static shell",
        "1.37.0-r30",
        "pkg:apk/alpine/busybox-static@1.37.0-r30?arch=x86_64",
        &[
            RuntimeSourceBuildInputRole::NativePackage,
            RuntimeSourceBuildInputRole::PosixShell,
        ],
        1_034_600,
        "82bbbabec12a985ae58810cfe975c3399264dc888aa592d8e460732bdd30a8dd",
    ),
    component(
        "toolchains/cmake.tar",
        "CMake static toolchain",
        "3.31.2-retonr.1",
        "urn:retonr:cmake-static:3.31.2-retonr.1",
        &[RuntimeSourceBuildInputRole::Cmake],
    ),
    component(
        "toolchains/go.tar",
        "Go toolchain",
        "1.26.0",
        "https://go.dev/dl/go1.26.0.linux-amd64.tar.gz",
        &[RuntimeSourceBuildInputRole::GoToolchain],
    ),
    component(
        "toolchains/ninja.tar",
        "Ninja static toolchain",
        "1.12.1-retonr.1",
        "urn:retonr:ninja-static:1.12.1-retonr.1",
        &[RuntimeSourceBuildInputRole::Ninja],
    ),
    component(
        "toolchains/zig.tar",
        "Zig compiler toolchain",
        "0.15.1",
        "https://ziglang.org/download/0.15.1/zig-x86_64-linux-0.15.1.tar.xz",
        &[
            RuntimeSourceBuildInputRole::CCompiler,
            RuntimeSourceBuildInputRole::CxxCompiler,
            RuntimeSourceBuildInputRole::Assembler,
            RuntimeSourceBuildInputRole::Linker,
            RuntimeSourceBuildInputRole::StandardLibrary,
        ],
    ),
];
