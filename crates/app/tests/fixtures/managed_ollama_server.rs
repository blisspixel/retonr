use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs,
    io::{Read as _, Write as _},
    net::TcpListener,
    path::Path,
};

const INPUT_ROOT: &str = "/tmp/retonr-managed-runtime-input-v1";
const MANIFEST: &str =
    "/tmp/retonr-managed-runtime-input-v1/manifests/registry.ollama.ai/library/fixture/verified";

fn main() {
    if !exact_command() || !exact_environment() || !exact_private_inputs() {
        std::process::exit(64);
    }
    let listener =
        TcpListener::bind("127.0.0.1:11434").unwrap_or_else(|_error| std::process::exit(65));
    for incoming in listener.incoming() {
        let Ok(mut stream) = incoming else {
            std::process::exit(66);
        };
        let mut request = [0_u8; 4];
        if stream.read_exact(&mut request).is_err() || request != *b"PING" {
            std::process::exit(67);
        }
        if stream.write_all(b"PONG").is_err() {
            std::process::exit(68);
        }
    }
}

fn exact_command() -> bool {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    arguments.len() == 2 && arguments[1] == "serve"
}

fn exact_environment() -> bool {
    let observed = std::env::vars_os().collect::<BTreeMap<_, _>>();
    let expected = [
        ("HOME", "/tmp"),
        ("TMPDIR", "/tmp"),
        ("OLLAMA_HOST", "127.0.0.1:11434"),
        ("OLLAMA_MODELS", INPUT_ROOT),
        ("OLLAMA_NO_CLOUD", "1"),
        ("OLLAMA_NOPRUNE", "1"),
        ("OLLAMA_NUM_PARALLEL", "1"),
        ("OLLAMA_MAX_LOADED_MODELS", "1"),
        ("OLLAMA_MAX_QUEUE", "1"),
        ("OLLAMA_FLASH_ATTENTION", "0"),
        ("OLLAMA_VULKAN", "0"),
        ("CUDA_VISIBLE_DEVICES", "-1"),
        ("HIP_VISIBLE_DEVICES", "-1"),
        ("ROCR_VISIBLE_DEVICES", "-1"),
        ("GGML_VK_VISIBLE_DEVICES", "-1"),
        ("GPU_DEVICE_ORDINAL", "-1"),
    ]
    .into_iter()
    .map(|(key, value)| (OsString::from(key), OsString::from(value)))
    .collect::<BTreeMap<_, _>>();
    observed == expected
}

fn exact_private_inputs() -> bool {
    let root = Path::new(INPUT_ROOT);
    let Ok(manifest) = fs::metadata(MANIFEST) else {
        return false;
    };
    if !manifest.is_file() || manifest.len() == 0 {
        return false;
    }
    let mut files = 0_u32;
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            return false;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                return false;
            };
            let Ok(kind) = entry.file_type() else {
                return false;
            };
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                files = files.saturating_add(1);
            } else {
                return false;
            }
        }
    }
    files == 6 && Path::new(MANIFEST).file_name() == Some(OsStr::new("verified"))
}
