use super::{accelerator_path, parse_mapping};
use crate::ManagedGenerationWorkerError;

#[test]
fn mapping_parser_accepts_file_rows_and_kernel_synthetic_exceptions() {
    let mapping = parse_mapping("1000-2000 r-xp 00000000 08:01 42 /lib/ollama/libggml-cpu.so")
        .expect("valid row")
        .expect("file mapping");
    assert!(mapping.executable);
    assert_eq!(mapping.start, 0x1000);
    assert_eq!(mapping.end, 0x2000);
    assert_eq!(mapping.path, "/lib/ollama/libggml-cpu.so");
    assert_eq!(
        parse_mapping("1000-2000 r-xp 00000000 00:00 0 [vdso]").expect("synthetic exception"),
        None
    );
}

#[test]
fn mapping_parser_rejects_anonymous_deleted_malformed_and_ambiguous_paths() {
    for row in [
        "1000-2000 r-xp 00000000 00:00 0",
        "1000-2000 r-xp 00000000 08:01 42 /tmp/code (deleted)",
        "1000-2000 r-xp invalid 08:01 42 /tmp/code",
        "1000-2000 zzzp 00000000 08:01 42 /tmp/code",
        "1000-2000 r--p 00000000 08:01 42 /models/model with-space.gguf",
    ] {
        assert!(parse_mapping(row).is_err(), "row must fail: {row}");
    }
}

#[test]
fn accelerator_library_names_fail_closed_without_matching_cpu_substrings() {
    for path in [
        "/usr/lib/libcuda.so.1",
        "/usr/lib/libnvidia-ml.so",
        "/usr/lib/libamdhip64.so",
        "/usr/lib/libvulkan.so",
        "/usr/lib/libOpenCL.so",
        "/usr/lib/libze_loader.so",
    ] {
        assert!(accelerator_path(path), "accelerator path: {path}");
    }
    assert!(!accelerator_path("/lib/ollama/libggml-cpu.so"));
}

#[test]
fn deleted_data_mapping_is_a_model_mapping_failure() {
    assert_eq!(
        parse_mapping("1000-2000 r--p 00000000 08:01 42 /models/model (deleted)"),
        Err(ManagedGenerationWorkerError::ModelMappingMismatch)
    );
}
