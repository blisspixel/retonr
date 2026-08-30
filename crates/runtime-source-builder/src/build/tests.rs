use super::*;

#[test]
fn ninja_contract_matches_the_checked_in_fixture_byte_for_byte() {
    assert_eq!(
        NINJA_CONTRACT,
        include_bytes!("../../../../support/runtime-source-build/retonr-shell-contract.json")
    );
}

#[test]
fn native_cpu_flag_matches_the_canonical_parameter_record() {
    let parameters: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../support/runtime-source-build/ollama-v0.32.15-parameters.json"
    ))
    .expect("canonical parameters");
    assert_eq!(parameters["c_flags"][1], NATIVE_CPU_FLAG);
    assert_eq!(
        parameters["x86_repack_compile_flags"],
        serde_json::json!(["-O0", "-fno-vectorize", "-fno-slp-vectorize"])
    );
    assert_eq!(
        parameters["cmake_definitions"]["HOST_CXX_COMPILER"],
        "<work>/shims/retonr-zig-host-cxx"
    );
    assert_eq!(
        parameters["host_cxx_flags"],
        serde_json::json!(["-target", "x86_64-linux-musl", "-static"])
    );
    for name in [
        "CMAKE_EXE_LINKER_FLAGS",
        "CMAKE_MODULE_LINKER_FLAGS",
        "CMAKE_SHARED_LINKER_FLAGS",
    ] {
        assert_eq!(parameters["cmake_definitions"][name], "-Wl,--strip-all");
    }
    assert_eq!(parameters["cmake_definitions"]["CMAKE_STRIP"], "");
    assert_eq!(
        parameters["go"]["ldflags"][3],
        "-extldflags=-Wl,--strip-all"
    );
    assert_eq!(parameters["reported_version"], "0.32.15");
    assert_eq!(
        parameters["go"]["ldflags"][4],
        "-X=github.com/ollama/ollama/version.Version=0.32.15"
    );
    assert_eq!(
        parameters["cmake_definitions"]["LLAMA_BUILD_NUMBER"],
        "10488"
    );
    assert_eq!(
        parameters["cmake_definitions"]["LLAMA_BUILD_COMMIT"],
        "9d77fa17254e1dee4b9e92504c91611a60b1359f"
    );
    assert_eq!(parameters["maximum_parallel_jobs"], 1);
}

#[test]
fn retained_llama_cpp_patch_matches_the_checked_in_fixture() {
    let patch = include_bytes!(
        "../../../../support/runtime-source-build/llama-cpp-zig-clang20-evex512.patch"
    );
    assert!(patch.ends_with(b"\n"));
    assert!(!patch.contains(&b'\r'));
    assert_eq!(
        LLAMA_CPP_ZIG_PATCH,
        "patches/llama-cpp-zig-clang20-evex512.patch"
    );

    let root = tempfile::tempdir().expect("temporary source root");
    let source = root.path().join("ggml/src/ggml-cpu/CMakeLists.txt");
    std::fs::create_dir_all(source.parent().expect("source parent")).expect("create source parent");
    let mut upstream = "unrelated upstream line\n".repeat(239);
    upstream.push_str(concat!(
        "    elseif (GGML_SYSTEM_ARCH STREQUAL \"x86\")\n",
        "        message(STATUS \"x86 detected\")\n",
        "        list(APPEND GGML_CPU_SOURCES\n",
        "            ggml-cpu/arch/x86/quants.c\n",
        "            ggml-cpu/arch/x86/repack.cpp\n",
        "            )\n",
        "\n",
        "        if (MSVC)\n",
    ));
    upstream.push_str(&"unrelated upstream line\n".repeat(88));
    upstream.push_str(concat!(
        "                if (GGML_AVX512)\n",
        "                    list(APPEND ARCH_FLAGS -mavx512f)\n",
        "                    list(APPEND ARCH_FLAGS -mavx512cd)\n",
        "                    list(APPEND ARCH_FLAGS -mavx512vl)\n",
        "                    list(APPEND ARCH_FLAGS -mavx512dq)\n",
        "                    list(APPEND ARCH_FLAGS -mavx512bw)\n",
        "                    list(APPEND ARCH_DEFINITIONS GGML_AVX512)\n",
        "                endif()\n",
        "                if (GGML_AVX512_VBMI)\n",
    ));
    std::fs::write(&source, upstream).expect("write exact upstream context");
    let patch_path = root.path().join("retained.patch");
    std::fs::write(&patch_path, patch).expect("write retained patch");
    crate::patch::apply_exact_unified_diff(&patch_path, root.path())
        .expect("retained patch applies to exact upstream context");
    let updated = std::fs::read_to_string(source).expect("read patched source");
    assert!(updated.contains("list(APPEND ARCH_FLAGS -mevex512)"));
    assert!(updated.contains("COMPILE_OPTIONS \"-O0;-fno-vectorize;-fno-slp-vectorize\""));
}

#[test]
fn retained_ollama_patch_removes_nondeterministic_error_timestamps() {
    let patch = include_bytes!(
        "../../../../support/runtime-source-build/ollama-mlx-reproducible-errors.patch"
    );
    assert!(patch.ends_with(b"\n"));
    assert!(!patch.contains(&b'\r'));
    assert_eq!(
        OLLAMA_MLX_REPRODUCIBILITY_PATCH,
        "patches/ollama-mlx-reproducible-errors.patch"
    );

    let root = tempfile::tempdir().expect("temporary source root");
    let source = root.path().join("x/mlxrunner/mlx/dynamic.h");
    std::fs::create_dir_all(source.parent().expect("source parent")).expect("create source parent");
    let mut upstream = "unrelated upstream line\n".repeat(26);
    upstream.push_str(concat!(
        "#ifdef ERROR\n",
        "#undef ERROR\n",
        "#endif\n",
        "#define MLX_ERROR(fmt, ...) fprintf(stderr, \"%s %s - ERROR - %s:%d - \" fmt \"\\n\", __DATE__, __TIME__, __FILE__, __LINE__, ##__VA_ARGS__); return 1\n",
        "#define CHECK(x) if (!(x)) { MLX_ERROR(\"CHECK failed: \" #x); }\n",
        "#define CHECK_LOAD(handle, x) *(void**)(&x##_) = DLSYM(handle, #x); CHECK(x##_)\n",
        "// OPTIONAL_LOAD: load symbol if available, leave function pointer NULL otherwise\n",
    ));
    std::fs::write(&source, upstream).expect("write exact upstream context");
    let patch_path = root.path().join("retained.patch");
    std::fs::write(&patch_path, patch).expect("write retained patch");
    crate::patch::apply_exact_unified_diff(&patch_path, root.path())
        .expect("retained patch applies to exact upstream context");
    let updated = std::fs::read_to_string(source).expect("read patched source");
    assert!(!updated.contains("__DATE__"));
    assert!(!updated.contains("__TIME__"));
    assert!(updated.contains("ERROR - %s:%d"));
}
