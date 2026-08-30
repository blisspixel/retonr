use crate::{BuildError, arguments::BuildArguments};

#[cfg(target_os = "linux")]
use std::{fs, io::Write as _, path::Path};

#[cfg(target_os = "linux")]
use crate::{
    archive::extract_exact_tar,
    command::{BuildEnvironment, owned_values},
    evidence::{self, SUCCESS_OUTPUT},
    filesystem::{
        capability_path, copy_file, digest_file, exact_capability_fd, exact_capability_root,
        require_empty_output, verify_self_contained,
    },
    native_layout,
    patch::apply_exact_unified_diff,
};

#[cfg(target_os = "linux")]
const INPUT_FD: &str = "RETONR_CONTROLLED_BUILD_INPUT_FD";
#[cfg(target_os = "linux")]
const OUTPUT_FD: &str = "RETONR_CONTROLLED_BUILD_OUTPUT_FD";
#[cfg(target_os = "linux")]
const CAPABILITY_ABI: &str = "RETONR_CONTROLLED_BUILD_CAPABILITY_ABI";
#[cfg(target_os = "linux")]
const INPUT_ROOT: &str = "RETONR_CONTROLLED_BUILD_INPUT_ROOT";
#[cfg(target_os = "linux")]
const OUTPUT_ROOT: &str = "RETONR_CONTROLLED_BUILD_OUTPUT_ROOT";
#[cfg(target_os = "linux")]
const EXPECTED_INPUT_ROOT: &str = "/tmp/retonr-controlled-build/input";
#[cfg(target_os = "linux")]
const EXPECTED_OUTPUT_ROOT: &str = "/tmp/retonr-controlled-build/output";
#[cfg(target_os = "linux")]
const BUILD_PROGRAM: &str = "scripts/build";
#[cfg(any(target_os = "linux", test))]
const LLAMA_CPP_ZIG_PATCH: &str = "patches/llama-cpp-zig-clang20-evex512.patch";
#[cfg(any(target_os = "linux", test))]
const OLLAMA_MLX_REPRODUCIBILITY_PATCH: &str = "patches/ollama-mlx-reproducible-errors.patch";
#[cfg(any(target_os = "linux", test))]
const NINJA_CONTRACT: &[u8] = b"{\"environment\":\"RETONR_NINJA_SHELL\",\"schema_version\":1}\n";
#[cfg(any(target_os = "linux", test))]
const NATIVE_CPU_FLAG: &str = "-march=x86_64_v2";
#[cfg(target_os = "linux")]
const ZIG_TARGET: &str = "x86_64-linux-gnu.2.28";

#[cfg(not(target_os = "linux"))]
pub(super) fn run(_arguments: &BuildArguments) -> Result<(), BuildError> {
    Err(BuildError::InvalidCapability)
}

#[cfg(target_os = "linux")]
pub(super) fn run(arguments: &BuildArguments) -> Result<(), BuildError> {
    if std::env::var(CAPABILITY_ABI).ok().as_deref() != Some("2") {
        return Err(BuildError::InvalidCapability);
    }
    let input_fd = exact_capability_fd(INPUT_FD)?;
    let output_fd = exact_capability_fd(OUTPUT_FD)?;
    let input_capability = capability_path(input_fd)?;
    let output_capability = capability_path(output_fd)?;
    require_same_output(&output_capability)?;
    require_empty_output(&output_capability)?;
    let input = exact_capability_root(INPUT_ROOT, EXPECTED_INPUT_ROOT, &input_capability)?;
    let output = exact_capability_root(OUTPUT_ROOT, EXPECTED_OUTPUT_ROOT, &output_capability)?;
    require_same_output(&output)?;
    write_stage("capabilities-verified")?;
    let builder_digest = digest_file(&input.join(BUILD_PROGRAM))?.1;
    let work = output.join("work");
    fs::create_dir(&work).map_err(|_| BuildError::OutputInvalid)?;
    write_stage("output-prepared")?;
    let trees = extract_inputs(&input, &work)?;
    write_stage("inputs-extracted")?;
    let tools = ToolPaths::new(&input, &trees);
    tools.verify()?;
    write_stage("tools-verified")?;
    let shims = prepare_shims(&work, &input.join(BUILD_PROGRAM))?;
    let busybox = retain_shell(&tools.busybox)?;
    let mut environment = BuildEnvironment::from_controlled_process()?;
    configure_environment(
        &mut environment,
        &output,
        &work,
        &trees,
        &tools,
        &shims,
        busybox.fd_path(),
    )?;
    write_stage("environment-configured")?;
    apply_compatibility_patch(&trees)?;
    write_stage("compatibility-patch-applied")?;
    apply_exact_unified_diff(&input.join(LLAMA_CPP_ZIG_PATCH), &trees.llama)?;
    write_stage("retained-source-patch-applied")?;
    apply_exact_unified_diff(&input.join(OLLAMA_MLX_REPRODUCIBILITY_PATCH), &trees.ollama)?;
    write_stage("retained-ollama-patch-applied")?;
    build_native(&environment, &work, &trees, &tools, &shims, arguments.jobs)?;
    write_stage("native-build-complete")?;
    assemble_native(&output)?;
    write_stage("native-layout-assembled")?;
    build_go(&environment, &output, &trees, &tools, arguments)?;
    write_stage("go-build-complete")?;
    copy_file(
        &input.join("helper/isolation"),
        &output.join("helper/retonr-isolation"),
    )?;
    copy_file(
        &trees.ollama.join("LICENSE"),
        &output.join("legal/ollama-license.txt"),
    )?;
    copy_file(
        &trees.llama.join("LICENSE"),
        &output.join("legal/llama-cpp-license.txt"),
    )?;
    drop(busybox);
    fs::remove_dir_all(&work).map_err(|_| BuildError::OutputInvalid)?;
    evidence::write(&output, arguments, &builder_digest)?;
    write_stage("evidence-written")?;
    std::io::stdout()
        .write_all(SUCCESS_OUTPUT)
        .map_err(|_| BuildError::OutputInvalid)
}

#[cfg(target_os = "linux")]
fn apply_compatibility_patch(trees: &ExtractedTrees) -> Result<(), BuildError> {
    apply_exact_unified_diff(
        &trees.ollama.join("llama/compat/001-llama-cpp-hooks.patch"),
        &trees.llama,
    )
}

#[cfg(target_os = "linux")]
struct ExtractedTrees {
    ollama: std::path::PathBuf,
    llama: std::path::PathBuf,
    go: std::path::PathBuf,
    module_cache: std::path::PathBuf,
    cmake: std::path::PathBuf,
    ninja: std::path::PathBuf,
    zig: std::path::PathBuf,
}

#[cfg(target_os = "linux")]
fn extract_inputs(input: &Path, work: &Path) -> Result<ExtractedTrees, BuildError> {
    let extraction = work.join("extracted");
    fs::create_dir(&extraction).map_err(|_| BuildError::OutputInvalid)?;
    let specifications = [
        ("sources/ollama.tar", "ollama"),
        ("sources/llama-cpp.tar", "llama-cpp"),
        ("toolchains/go.tar", "go"),
        ("modules/go-module-cache.tar", "gomodcache"),
        ("toolchains/cmake.tar", "cmake"),
        ("toolchains/ninja.tar", "ninja"),
        ("toolchains/zig.tar", "zig"),
    ];
    let mut roots = Vec::new();
    for (index, (archive, root)) in specifications.into_iter().enumerate() {
        let destination = extraction.join(index.to_string());
        write_stage(match root {
            "ollama" => "extract-ollama",
            "llama-cpp" => "extract-llama-cpp",
            "go" => "extract-go",
            "gomodcache" => "extract-go-module-cache",
            "cmake" => "extract-cmake",
            "ninja" => "extract-ninja",
            "zig" => "extract-zig",
            _ => return Err(BuildError::InvalidArchive),
        })?;
        extract_exact_tar(&input.join(archive), &destination, root)?;
        roots.push(destination.join(root));
    }
    let mut roots = roots.into_iter();
    Ok(ExtractedTrees {
        ollama: roots.next().ok_or(BuildError::InvalidArchive)?,
        llama: roots.next().ok_or(BuildError::InvalidArchive)?,
        go: roots.next().ok_or(BuildError::InvalidArchive)?,
        module_cache: roots.next().ok_or(BuildError::InvalidArchive)?,
        cmake: roots.next().ok_or(BuildError::InvalidArchive)?,
        ninja: roots.next().ok_or(BuildError::InvalidArchive)?,
        zig: roots.next().ok_or(BuildError::InvalidArchive)?,
    })
}

#[cfg(target_os = "linux")]
pub(super) fn write_stage(stage: &str) -> Result<(), BuildError> {
    let mut stderr = std::io::stderr();
    stderr
        .write_all(b"runtime-source-build-stage:")
        .and_then(|()| stderr.write_all(stage.as_bytes()))
        .and_then(|()| stderr.write_all(b"\n"))
        .and_then(|()| stderr.flush())
        .map_err(|_| BuildError::OutputInvalid)
}

#[cfg(target_os = "linux")]
struct ToolPaths {
    go: std::path::PathBuf,
    cmake: std::path::PathBuf,
    ninja: std::path::PathBuf,
    zig: std::path::PathBuf,
    busybox: std::path::PathBuf,
    ninja_contract: std::path::PathBuf,
}

#[cfg(target_os = "linux")]
impl ToolPaths {
    fn new(input: &Path, trees: &ExtractedTrees) -> Self {
        Self {
            go: trees.go.join("bin/go"),
            cmake: trees.cmake.join("bin/cmake"),
            ninja: trees.ninja.join("ninja"),
            zig: trees.zig.join("zig"),
            busybox: input.join("toolchains/busybox"),
            ninja_contract: trees.ninja.join("retonr-shell-contract.json"),
        }
    }

    fn verify(&self) -> Result<(), BuildError> {
        for executable in [&self.go, &self.cmake, &self.ninja, &self.zig, &self.busybox] {
            verify_self_contained(executable)?;
        }
        if fs::read(&self.ninja_contract).map_err(|_| BuildError::InvalidInput)? != NINJA_CONTRACT {
            return Err(BuildError::InvalidInput);
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
struct ShimPaths {
    cc: std::path::PathBuf,
    cxx: std::path::PathBuf,
    host_cxx: std::path::PathBuf,
    ar: std::path::PathBuf,
    ranlib: std::path::PathBuf,
}

#[cfg(target_os = "linux")]
fn prepare_shims(work: &Path, build_program: &Path) -> Result<ShimPaths, BuildError> {
    use std::os::unix::fs::PermissionsExt as _;

    let root = work.join("shims");
    fs::create_dir(&root).map_err(|_| BuildError::OutputInvalid)?;
    let destination = |name: &str| root.join(name);
    let paths = ShimPaths {
        cc: destination("retonr-zig-cc"),
        cxx: destination("retonr-zig-cxx"),
        host_cxx: destination("retonr-zig-host-cxx"),
        ar: destination("retonr-zig-ar"),
        ranlib: destination("retonr-zig-ranlib"),
    };
    for path in [
        &paths.cc,
        &paths.cxx,
        &paths.host_cxx,
        &paths.ar,
        &paths.ranlib,
    ] {
        copy_file(build_program, path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .map_err(|_| BuildError::OutputInvalid)?;
    }
    Ok(paths)
}

#[cfg(target_os = "linux")]
struct RetainedShell {
    file: fs::File,
}

#[cfg(target_os = "linux")]
impl RetainedShell {
    fn fd_path(&self) -> String {
        use std::os::fd::AsRawFd as _;
        format!("/proc/self/fd/{}", self.file.as_raw_fd())
    }
}

#[cfg(target_os = "linux")]
fn retain_shell(path: &Path) -> Result<RetainedShell, BuildError> {
    use rustix::io::{FdFlags, fcntl_setfd};

    let file = fs::File::open(path).map_err(|_| BuildError::InvalidInput)?;
    fcntl_setfd(&file, FdFlags::empty()).map_err(|_| BuildError::InvalidCapability)?;
    Ok(RetainedShell { file })
}

#[cfg(target_os = "linux")]
fn configure_environment(
    environment: &mut BuildEnvironment,
    output: &Path,
    work: &Path,
    trees: &ExtractedTrees,
    tools: &ToolPaths,
    shims: &ShimPaths,
    shell: String,
) -> Result<(), BuildError> {
    let path = [
        work.join("shims"),
        trees.go.join("bin"),
        trees.cmake.join("bin"),
        trees.ninja.clone(),
        trees.zig.clone(),
    ]
    .iter()
    .map(|path| path_text(path))
    .collect::<Result<Vec<_>, _>>()?
    .join(":");
    for (name, value) in [
        ("AR", path_text(&shims.ar)?),
        ("CC", path_text(&shims.cc)?),
        ("CMAKE_GENERATOR", "Ninja".to_owned()),
        ("CXX", path_text(&shims.cxx)?),
        ("GOENV", "off".to_owned()),
        ("GOCACHE", path_text(&work.join("go-cache"))?),
        ("GOMODCACHE", path_text(&trees.module_cache)?),
        ("GOTOOLCHAIN", "local".to_owned()),
        ("HOME", path_text(&work.join("home"))?),
        ("PATH", path),
        ("RETONR_NINJA_SHELL", shell),
        ("RETONR_ZIG_EXECUTABLE", path_text(&tools.zig)?),
        ("RETONR_ZIG_TARGET", ZIG_TARGET.to_owned()),
        ("TMPDIR", path_text(&work.join("tmp"))?),
        (
            "ZIG_GLOBAL_CACHE_DIR",
            path_text(&work.join("zig-global-cache"))?,
        ),
        (
            "ZIG_LOCAL_CACHE_DIR",
            path_text(&work.join("zig-local-cache"))?,
        ),
    ] {
        environment.insert(name, value);
    }
    for directory in [
        "go-cache",
        "home",
        "tmp",
        "zig-global-cache",
        "zig-local-cache",
    ] {
        fs::create_dir(work.join(directory)).map_err(|_| BuildError::OutputInvalid)?;
    }
    fs::create_dir_all(output.join("bin")).map_err(|_| BuildError::OutputInvalid)
}

#[cfg(target_os = "linux")]
fn build_native(
    environment: &BuildEnvironment,
    work: &Path,
    trees: &ExtractedTrees,
    tools: &ToolPaths,
    shims: &ShimPaths,
    jobs: u8,
) -> Result<(), BuildError> {
    let source = trees.ollama.join("llama/server");
    let build = work.join("native-build");
    let install = work.join("native-install");
    let mapped_root = path_text(work)?;
    let prefix_flags = format!(
        "-O3 {NATIVE_CPU_FLAG} -ffile-prefix-map={mapped_root}=/build \
         -fdebug-prefix-map={mapped_root}=/build"
    );
    let configure = owned_values([
        "-S".to_owned(),
        path_text(&source)?,
        "-B".to_owned(),
        path_text(&build)?,
        "-G".to_owned(),
        "Ninja".to_owned(),
        "-DBUILD_SHARED_LIBS=ON".to_owned(),
        "-DCMAKE_BUILD_TYPE=Release".to_owned(),
        format!("-DCMAKE_C_COMPILER={}", path_text(&shims.cc)?),
        format!("-DCMAKE_CXX_COMPILER={}", path_text(&shims.cxx)?),
        format!("-DCMAKE_AR={}", path_text(&shims.ar)?),
        format!("-DCMAKE_RANLIB={}", path_text(&shims.ranlib)?),
        format!("-DCMAKE_MAKE_PROGRAM={}", path_text(&tools.ninja)?),
        format!("-DHOST_CXX_COMPILER={}", path_text(&shims.host_cxx)?),
        format!("-DCMAKE_C_FLAGS={prefix_flags}"),
        format!("-DCMAKE_CXX_FLAGS={prefix_flags}"),
        "-DCMAKE_EXE_LINKER_FLAGS=-Wl,--strip-all".to_owned(),
        "-DCMAKE_MODULE_LINKER_FLAGS=-Wl,--strip-all".to_owned(),
        "-DCMAKE_SHARED_LINKER_FLAGS=-Wl,--strip-all".to_owned(),
        "-DCMAKE_STRIP=".to_owned(),
        format!("-DCMAKE_INSTALL_PREFIX={}", path_text(&install)?),
        "-DCMAKE_SYSTEM_NAME=Linux".to_owned(),
        "-DCMAKE_SYSTEM_PROCESSOR=x86_64".to_owned(),
        "-DCMAKE_TRY_COMPILE_TARGET_TYPE=STATIC_LIBRARY".to_owned(),
        format!(
            "-DFETCHCONTENT_SOURCE_DIR_LLAMA_CPP={}",
            path_text(&trees.llama)?
        ),
        "-DGGML_BACKEND_DL=ON".to_owned(),
        "-DGGML_CCACHE=OFF".to_owned(),
        "-DGGML_CPU_ALL_VARIANTS=ON".to_owned(),
        "-DGGML_NATIVE=OFF".to_owned(),
        "-DGGML_OPENMP=OFF".to_owned(),
        "-DLLAMA_BUILD_COMMIT=9d77fa17254e1dee4b9e92504c91611a60b1359f".to_owned(),
        "-DLLAMA_BUILD_NUMBER=10488".to_owned(),
        "-DLLAMA_CURL=OFF".to_owned(),
        "-DLLAMA_OPENSSL=OFF".to_owned(),
        "-DOLLAMA_RUNNER_DIR=".to_owned(),
        "-DOLLAMA_LLAMA_CPP_SKIP_COMPAT_PATCH=ON".to_owned(),
    ]);
    environment.run("configure-native", &tools.cmake, work, &configure)?;
    environment.run(
        "build-native",
        &tools.cmake,
        work,
        &owned_values([
            "--build".to_owned(),
            path_text(&build)?,
            "--target".to_owned(),
            "llama-server".to_owned(),
            "--parallel".to_owned(),
            jobs.to_string(),
        ]),
    )?;
    environment.run(
        "install-native",
        &tools.cmake,
        work,
        &owned_values([
            "--install".to_owned(),
            path_text(&build)?,
            "--component".to_owned(),
            "llama-server".to_owned(),
        ]),
    )
}

#[cfg(target_os = "linux")]
fn assemble_native(output: &Path) -> Result<(), BuildError> {
    native_layout::assemble(output)
}

#[cfg(target_os = "linux")]
fn build_go(
    environment: &BuildEnvironment,
    output: &Path,
    trees: &ExtractedTrees,
    tools: &ToolPaths,
    arguments: &BuildArguments,
) -> Result<(), BuildError> {
    let ldflags = format!(
        "-buildid= -s -w -extldflags=-Wl,--strip-all -X=github.com/ollama/ollama/version.Version={} -X=github.com/ollama/ollama/server.mode=release",
        arguments.reported_version
    );
    environment.run(
        "build-go",
        &tools.go,
        &trees.ollama,
        &owned_values([
            "build".to_owned(),
            "-trimpath".to_owned(),
            "-buildmode=pie".to_owned(),
            "-mod=readonly".to_owned(),
            format!("-ldflags={ldflags}"),
            "-o".to_owned(),
            path_text(&output.join("bin/ollama"))?,
            ".".to_owned(),
        ]),
    )
}

#[cfg(target_os = "linux")]
fn path_text(path: &Path) -> Result<String, BuildError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or(BuildError::InvalidCapability)
}

#[cfg(target_os = "linux")]
fn require_same_output(output: &Path) -> Result<(), BuildError> {
    use std::os::unix::fs::MetadataExt as _;

    let current = fs::metadata(".").map_err(|_| BuildError::InvalidCapability)?;
    let retained = fs::metadata(output).map_err(|_| BuildError::InvalidCapability)?;
    if current.dev() == retained.dev() && current.ino() == retained.ino() {
        Ok(())
    } else {
        Err(BuildError::InvalidCapability)
    }
}

#[cfg(test)]
#[path = "build/tests.rs"]
mod tests;
