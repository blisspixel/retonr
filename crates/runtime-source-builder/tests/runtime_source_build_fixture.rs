#![forbid(unsafe_code)]

use std::{error::Error, fs, io::Write as _, path::Path, process::ExitCode};

use rewrite_types::Digest;
use serde_json::json;

const CAPABILITY_ABI_VARIABLE: &str = "RETONR_CONTROLLED_BUILD_CAPABILITY_ABI";
const SOURCE_ID_VARIABLE: &str = "RETONR_FIXTURE_SOURCE_ID";
const FIXTURE_ARGUMENT: &str = "controlled-fixture";
const SUCCESS_OUTPUT: &[u8] = b"{\"schema_version\":1,\"status\":\"success\"}\n";
const BUILD_REVISION: &str = "b7871fc0d1d82fe109536efa3e0e8e411c766c75";

fn main() -> ExitCode {
    if std::env::var_os(CAPABILITY_ABI_VARIABLE).is_none() {
        return ExitCode::SUCCESS;
    }
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = std::io::stderr()
                .write_all(format!("runtime-source-build-fixture-error:{error}\n").as_bytes());
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    if std::env::var(CAPABILITY_ABI_VARIABLE)?.as_str() != "2" {
        return Err("invalid controlled-build capability ABI".into());
    }
    let mut arguments = std::env::args_os();
    let _program = arguments.next().ok_or("missing fixture program")?;
    if arguments.next().as_deref() != Some(FIXTURE_ARGUMENT.as_ref()) || arguments.next().is_some()
    {
        return Err("invalid fixture arguments".into());
    }
    let source_id = std::env::var(SOURCE_ID_VARIABLE)?;
    let members = runtime_members();
    for member in &members {
        let path = Path::new(member.path);
        fs::create_dir_all(path.parent().ok_or("member has no parent")?)?;
        fs::write(path, member.bytes)?;
    }
    let declarations = members
        .iter()
        .map(|member| {
            json!({
                "byte_size": member.bytes.len(),
                "digest": Digest::sha256(member.bytes),
                "load_policy": member.load_policy,
                "relative_path": member.path,
                "roles": member.roles
            })
        })
        .collect::<Vec<_>>();
    let observed_tree = members.iter().map(|member| member.path).collect::<Vec<_>>();
    let layout = serde_json::to_vec(&json!({
        "build_revision": BUILD_REVISION,
        "members": declarations,
        "observed_tree": observed_tree,
        "reported_version": "0.32.15-retonr.fixture",
        "runtime_family": "ollama",
        "schema_version": 1,
        "source": {
            "kind": "repository_revision",
            "locator": "https://github.com/ollama/ollama",
            "provenance_digest": Digest::sha256(b"fixture source provenance"),
            "revision": BUILD_REVISION,
            "schema_version": 1
        },
        "target": {
            "abi": "linux_gnu_libc",
            "architecture": "x86_64",
            "operating_system": "linux"
        },
        "transformation": {
            "kind": "transformed",
            "log_digest": Digest::sha256(SUCCESS_OUTPUT),
            "parameters_digest": Digest::sha256(b"fixture build parameters"),
            "source_artifact_set_id": source_id,
            "tool_evidence_digest": Digest::sha256(b"fixture build tools")
        }
    }))?;
    fs::write("runtime-layout.json", layout)?;
    fs::write("sbom.json", br#"{"packages":[],"schema_version":1}"#)?;
    fs::write(
        "provenance.json",
        br#"{"builder":"retained-fixture","schema_version":1}"#,
    )?;
    fs::write(
        "transformation.json",
        br#"{"schema_version":1,"steps":["fixture"]}"#,
    )?;
    std::io::stdout().write_all(SUCCESS_OUTPUT)?;
    Ok(())
}

struct RuntimeMember {
    path: &'static str,
    roles: &'static [&'static str],
    load_policy: &'static str,
    bytes: &'static [u8],
}

fn runtime_members() -> [RuntimeMember; 7] {
    [
        RuntimeMember {
            path: "bin/ollama",
            roles: &["entrypoint"],
            load_policy: "required_at_ready",
            bytes: b"ollama-entrypoint\n",
        },
        RuntimeMember {
            path: "helper/retonr-isolation",
            roles: &["helper_executable"],
            load_policy: "must_not_be_code_loaded",
            bytes: b"isolation-helper\n",
        },
        RuntimeMember {
            path: "legal/license.txt",
            roles: &["license_text"],
            load_policy: "must_not_be_code_loaded",
            bytes: b"runtime license\n",
        },
        RuntimeMember {
            path: "lib/ollama/libggml-cpu.so",
            roles: &["native_dependency"],
            load_policy: "backend_conditional",
            bytes: b"ggml-cpu\n",
        },
        RuntimeMember {
            path: "lib/ollama/llama-server",
            roles: &["worker_executable"],
            load_policy: "backend_conditional",
            bytes: b"llama-server\n",
        },
        RuntimeMember {
            path: "provenance/source.txt",
            roles: &["provenance_record"],
            load_policy: "must_not_be_code_loaded",
            bytes: b"source provenance\n",
        },
        RuntimeMember {
            path: "review/transformation.json",
            roles: &["transformation_record"],
            load_policy: "must_not_be_code_loaded",
            bytes: b"transformation evidence\n",
        },
    ]
}
