use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

use rewrite_types::{CancellationToken, Digest};

use super::read_bounded;
use crate::{
    ManagedGenerationWorkerError, ManagedGenerationWorkerLimits, ManagedGenerationWorkerProfile,
};

pub(super) struct CommandEvidenceDigests {
    pub(super) portable_configuration_digest: Digest,
    pub(super) observation_digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PortableWorkerConfiguration {
    context_tokens: u64,
    parallel_slots: u64,
    cache_type: u8,
    batch_size: u64,
    micro_batch_size: u64,
    thread_count: u64,
    keep_count: u64,
    context_shift: bool,
    no_jinja_chatml: bool,
}

pub(super) fn command_digest(
    pid: u32,
    expected_model_path: &str,
    profile: ManagedGenerationWorkerProfile,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<CommandEvidenceDigests, ManagedGenerationWorkerError> {
    let bytes = read_bounded(
        &format!("/proc/{pid}/cmdline"),
        limits.maximum_command_bytes,
        cancellation,
        started,
        limits,
    )?;
    command_evidence(
        &bytes,
        expected_model_path,
        profile,
        limits.maximum_command_arguments,
    )
}

fn command_evidence(
    bytes: &[u8],
    expected_model_path: &str,
    profile: ManagedGenerationWorkerProfile,
    maximum_args: usize,
) -> Result<CommandEvidenceDigests, ManagedGenerationWorkerError> {
    let configuration = validate_command(bytes, expected_model_path, maximum_args)?;
    let contract_id = profile.command_contract_id();
    let mut material = Vec::with_capacity(bytes.len().saturating_add(contract_id.len() + 1));
    material.extend_from_slice(contract_id.as_bytes());
    material.push(0);
    material.extend_from_slice(bytes);
    Ok(CommandEvidenceDigests {
        portable_configuration_digest: portable_configuration_digest(profile, &configuration),
        observation_digest: Digest::sha256(&material),
    })
}

fn validate_command(
    bytes: &[u8],
    expected_model_path: &str,
    maximum_args: usize,
) -> Result<PortableWorkerConfiguration, ManagedGenerationWorkerError> {
    if bytes.is_empty() || !bytes.ends_with(&[0]) {
        return Err(ManagedGenerationWorkerError::CommandMismatch);
    }
    let args = bytes[..bytes.len() - 1]
        .split(|byte| *byte == 0)
        .map(|arg| {
            std::str::from_utf8(arg).map_err(|_error| ManagedGenerationWorkerError::CommandMismatch)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if args.len() > maximum_args || args.iter().any(|arg| arg.is_empty()) || args.len() < 21 {
        return Err(ManagedGenerationWorkerError::CommandMismatch);
    }
    let base = [
        "--model",
        expected_model_path,
        "--port",
        args[4],
        "--host",
        "127.0.0.1",
        "--no-webui",
        "--offline",
        "-c",
        args[10],
        "-np",
        args[12],
    ];
    if args[1..13] != base
        || !decimal(args[4], 1, 65_535)
        || !decimal(args[10], 1, 1_048_576)
        || !decimal(args[12], 1, 64)
    {
        return Err(ManagedGenerationWorkerError::CommandMismatch);
    }
    let mut seen = BTreeSet::new();
    let mut values = BTreeMap::new();
    let mut index = 13;
    while index < args.len() {
        let flag = args[index];
        if !seen.insert(flag) {
            return Err(ManagedGenerationWorkerError::CommandMismatch);
        }
        match flag {
            "--no-log-prefix" | "--no-log-timestamps" | "--context-shift" => index += 1,
            "--log-verbosity" if args.get(index + 1) == Some(&"4") => index += 2,
            "--no-jinja"
                if args.get(index + 1) == Some(&"--chat-template")
                    && args.get(index + 2) == Some(&"chatml") =>
            {
                seen.insert("--chat-template");
                index += 3;
            }
            "--cache-type-k" | "--cache-type-v"
                if args
                    .get(index + 1)
                    .is_some_and(|value| matches!(*value, "f16" | "q8_0" | "q4_0")) =>
            {
                values.insert(flag, args[index + 1]);
                index += 2;
            }
            "--flash-attn" if args.get(index + 1) == Some(&"off") => index += 2,
            "-b" | "-ub" | "-t" | "--keep"
                if args
                    .get(index + 1)
                    .is_some_and(|value| decimal(value, 1, 1_048_576)) =>
            {
                values.insert(flag, args[index + 1]);
                index += 2;
            }
            "-ngl" if args.get(index + 1) == Some(&"0") => index += 2,
            _ => return Err(ManagedGenerationWorkerError::CommandMismatch),
        }
    }
    if ![
        "--log-verbosity",
        "--no-log-prefix",
        "--no-log-timestamps",
        "--flash-attn",
        "-ngl",
    ]
    .into_iter()
    .all(|flag| seen.contains(flag))
        || seen.contains("--cache-type-k") != seen.contains("--cache-type-v")
        || values.get("--cache-type-k") != values.get("--cache-type-v")
        || seen.contains("-b") != seen.contains("-ub")
        || values.get("-b") != values.get("-ub")
        || seen.contains("--keep") && !seen.contains("--context-shift")
    {
        return Err(ManagedGenerationWorkerError::CommandMismatch);
    }
    Ok(PortableWorkerConfiguration {
        context_tokens: parse_decimal(args[10])?,
        parallel_slots: parse_decimal(args[12])?,
        cache_type: cache_type_code(values.get("--cache-type-k").copied())?,
        batch_size: optional_decimal(&values, "-b")?,
        micro_batch_size: optional_decimal(&values, "-ub")?,
        thread_count: optional_decimal(&values, "-t")?,
        keep_count: optional_decimal(&values, "--keep")?,
        context_shift: seen.contains("--context-shift"),
        no_jinja_chatml: seen.contains("--no-jinja"),
    })
}

fn portable_configuration_digest(
    profile: ManagedGenerationWorkerProfile,
    configuration: &PortableWorkerConfiguration,
) -> Digest {
    let mut material = Vec::with_capacity(192);
    push_field(
        &mut material,
        b"retonr:managed-worker-portable-configuration:v1",
    );
    push_field(&mut material, profile.command_contract_id().as_bytes());
    for value in [
        configuration.context_tokens,
        configuration.parallel_slots,
        u64::from(configuration.cache_type),
        configuration.batch_size,
        configuration.micro_batch_size,
        configuration.thread_count,
        configuration.keep_count,
        u64::from(configuration.context_shift),
        u64::from(configuration.no_jinja_chatml),
    ] {
        material.extend_from_slice(&value.to_be_bytes());
    }
    Digest::sha256(&material)
}

fn optional_decimal(
    values: &BTreeMap<&str, &str>,
    flag: &str,
) -> Result<u64, ManagedGenerationWorkerError> {
    values.get(flag).map_or(Ok(0), |value| parse_decimal(value))
}

fn parse_decimal(value: &str) -> Result<u64, ManagedGenerationWorkerError> {
    value
        .parse()
        .map_err(|_error| ManagedGenerationWorkerError::CommandMismatch)
}

fn cache_type_code(value: Option<&str>) -> Result<u8, ManagedGenerationWorkerError> {
    match value {
        None => Ok(0),
        Some("f16") => Ok(1),
        Some("q8_0") => Ok(2),
        Some("q4_0") => Ok(3),
        Some(_) => Err(ManagedGenerationWorkerError::CommandMismatch),
    }
}

fn push_field(material: &mut Vec<u8>, value: &[u8]) {
    material.extend_from_slice(&(value.len() as u64).to_be_bytes());
    material.extend_from_slice(value);
}

fn decimal(value: &str, minimum: u64, maximum: u64) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value
            .parse::<u64>()
            .is_ok_and(|value| (minimum..=maximum).contains(&value))
}

#[cfg(test)]
mod tests {
    use super::{command_evidence, validate_command};
    use crate::{ManagedGenerationWorkerError, ManagedGenerationWorkerProfile};

    const MODEL: &str = "/tmp/retonr-managed-runtime-input-v1/blobs/sha256-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OTHER_MODEL: &str = "/tmp/retonr-managed-runtime-input-v1/blobs/sha256-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn command(extra: &[&str]) -> Vec<u8> {
        let mut args = vec![
            "/lib/ollama/llama-server",
            "--model",
            MODEL,
            "--port",
            "49152",
            "--host",
            "127.0.0.1",
            "--no-webui",
            "--offline",
            "-c",
            "4096",
            "-np",
            "1",
            "--log-verbosity",
            "4",
            "--no-log-prefix",
            "--no-log-timestamps",
            "--flash-attn",
            "off",
            "-ngl",
            "0",
        ];
        args.extend_from_slice(extra);
        let mut bytes = args.join("\0").into_bytes();
        bytes.push(0);
        bytes
    }

    #[test]
    fn accepts_exact_cpu_command_and_reviewed_optional_flags() {
        validate_command(
            &command(&[
                "--cache-type-k",
                "q8_0",
                "--cache-type-v",
                "q8_0",
                "-b",
                "512",
                "-ub",
                "512",
            ]),
            MODEL,
            64,
        )
        .expect("closed command");
    }

    #[test]
    fn rejects_wrong_model_unknown_gpu_duplicate_and_truncated_commands() {
        let mut wrong_model = command(&[]);
        let offset = wrong_model
            .windows(MODEL.len())
            .position(|bytes| bytes == MODEL.as_bytes())
            .expect("model");
        wrong_model[offset] = b'x';
        for bytes in [
            wrong_model,
            command(&["--mmproj", "/tmp/retonr-managed-runtime-input-v1/projector"]),
            command(&["-ngl", "1"]),
            command(&["--flash-attn", "on"]),
            command(&["--cache-type-k", "q8_0"]),
        ] {
            assert_eq!(
                validate_command(&bytes, MODEL, 64),
                Err(ManagedGenerationWorkerError::CommandMismatch)
            );
        }
        let mut truncated = command(&[]);
        truncated.pop();
        assert_eq!(
            validate_command(&truncated, MODEL, 64),
            Err(ManagedGenerationWorkerError::CommandMismatch)
        );
    }

    #[test]
    fn portable_configuration_binds_every_variable_execution_setting() {
        let profile = ManagedGenerationWorkerProfile::OllamaV0_32_15Cpu;
        let base = command_evidence(&command(&[]), MODEL, profile, 64)
            .expect("base command")
            .portable_configuration_digest;
        for extra in [
            vec!["--cache-type-k", "q8_0", "--cache-type-v", "q8_0"],
            vec!["-b", "512", "-ub", "512"],
            vec!["-t", "8"],
            vec!["--context-shift"],
            vec!["--context-shift", "--keep", "32"],
            vec!["--no-jinja", "--chat-template", "chatml"],
        ] {
            let changed = command_evidence(&command(&extra), MODEL, profile, 64)
                .expect("changed command")
                .portable_configuration_digest;
            assert_ne!(base, changed);
        }

        let mut context = command(&[]);
        replace_arg_at(&mut context, 10, "8192");
        let mut parallel = command(&[]);
        replace_arg_at(&mut parallel, 12, "2");
        for changed in [context, parallel] {
            assert_ne!(
                base,
                command_evidence(&changed, MODEL, profile, 64)
                    .expect("changed base argument")
                    .portable_configuration_digest
            );
        }
    }

    #[test]
    fn portable_configuration_excludes_port_and_private_model_path() {
        let profile = ManagedGenerationWorkerProfile::OllamaV0_32_15Cpu;
        let base = command_evidence(&command(&[]), MODEL, profile, 64).expect("base command");
        let mut other_port = command(&[]);
        replace_arg_at(&mut other_port, 4, "49153");
        let other_port =
            command_evidence(&other_port, MODEL, profile, 64).expect("other port command");
        assert_eq!(
            base.portable_configuration_digest,
            other_port.portable_configuration_digest
        );
        assert_ne!(base.observation_digest, other_port.observation_digest);

        let other_model = command_for_model(OTHER_MODEL, &[]);
        let other_model = command_evidence(&other_model, OTHER_MODEL, profile, 64)
            .expect("other private model path");
        assert_eq!(
            base.portable_configuration_digest,
            other_model.portable_configuration_digest
        );
        assert_ne!(base.observation_digest, other_model.observation_digest);
    }

    fn replace_arg_at(command: &mut [u8], index: usize, to: &str) {
        let mut start = 0;
        for _ in 0..index {
            start += command[start..]
                .iter()
                .position(|byte| *byte == 0)
                .expect("argument delimiter")
                + 1;
        }
        let length = command[start..]
            .iter()
            .position(|byte| *byte == 0)
            .expect("argument end");
        assert_eq!(length, to.len());
        command[start..start + length].copy_from_slice(to.as_bytes());
    }

    fn command_for_model(model: &str, extra: &[&str]) -> Vec<u8> {
        let mut bytes = command(extra);
        let offset = bytes
            .windows(MODEL.len())
            .position(|candidate| candidate == MODEL.as_bytes())
            .expect("model");
        bytes.splice(offset..offset + MODEL.len(), model.bytes());
        bytes
    }
}
