use std::{collections::BTreeMap, ffi::OsString};

use rewrite_model::ArtifactSetId;
use rewrite_types::Digest;

use crate::BuildError;

const REQUIRED_SWITCHES: [&str; 3] = ["--build-runtime", "--cpu-only", "--offline"];
const VALUE_NAMES: [&str; 7] = [
    "--build-revision",
    "--jobs",
    "--parameters-digest",
    "--reported-version",
    "--source-inputs-id",
    "--source-provenance-digest",
    "--tool-evidence-digest",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BuildArguments {
    pub(super) source_inputs_id: ArtifactSetId,
    pub(super) source_provenance_digest: Digest,
    pub(super) tool_evidence_digest: Digest,
    pub(super) parameters_digest: Digest,
    pub(super) build_revision: String,
    pub(super) reported_version: String,
    pub(super) jobs: u8,
}

impl BuildArguments {
    pub(super) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, BuildError> {
        let mut arguments = arguments.into_iter();
        let _program = arguments.next().ok_or(BuildError::InvalidArguments)?;
        let mut switches = Vec::new();
        let mut values = BTreeMap::new();
        while let Some(argument) = arguments.next() {
            let argument = argument
                .into_string()
                .map_err(|_| BuildError::InvalidArguments)?;
            if REQUIRED_SWITCHES.contains(&argument.as_str()) {
                switches.push(argument);
                continue;
            }
            if !VALUE_NAMES.contains(&argument.as_str()) || values.contains_key(&argument) {
                return Err(BuildError::InvalidArguments);
            }
            let value = arguments
                .next()
                .ok_or(BuildError::InvalidArguments)?
                .into_string()
                .map_err(|_| BuildError::InvalidArguments)?;
            values.insert(argument, value);
        }
        switches.sort_unstable();
        let mut expected = REQUIRED_SWITCHES.map(str::to_owned);
        expected.sort_unstable();
        if switches != expected || values.len() != VALUE_NAMES.len() {
            return Err(BuildError::InvalidArguments);
        }
        let build_revision = take(&mut values, "--build-revision")?;
        let reported_version = take(&mut values, "--reported-version")?;
        if !valid_revision(&build_revision) || !valid_identity(&reported_version, 128) {
            return Err(BuildError::InvalidArguments);
        }
        let jobs = take(&mut values, "--jobs")?
            .parse::<u8>()
            .ok()
            .filter(|jobs| (1..=64).contains(jobs))
            .ok_or(BuildError::InvalidArguments)?;
        let source_inputs_id =
            ArtifactSetId::from_digest(parse_digest(take(&mut values, "--source-inputs-id")?)?);
        Ok(Self {
            source_inputs_id,
            source_provenance_digest: parse_digest(take(
                &mut values,
                "--source-provenance-digest",
            )?)?,
            tool_evidence_digest: parse_digest(take(&mut values, "--tool-evidence-digest")?)?,
            parameters_digest: parse_digest(take(&mut values, "--parameters-digest")?)?,
            build_revision,
            reported_version,
            jobs,
        })
    }
}

fn take(values: &mut BTreeMap<String, String>, name: &str) -> Result<String, BuildError> {
    values.remove(name).ok_or(BuildError::InvalidArguments)
}

fn parse_digest(value: String) -> Result<Digest, BuildError> {
    Digest::from_sha256_hex(value).map_err(|_| BuildError::InvalidArguments)
}

fn valid_revision(value: &str) -> bool {
    matches!(value.len(), 7 | 40)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_identity(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.is_ascii()
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact() -> Vec<OsString> {
        [
            "builder",
            "--build-runtime",
            "--cpu-only",
            "--offline",
            "--build-revision",
            "b7871fc0d1d82fe109536efa3e0e8e411c766c75",
            "--jobs",
            "8",
            "--parameters-digest",
            "1111111111111111111111111111111111111111111111111111111111111111",
            "--reported-version",
            "0.32.15",
            "--source-inputs-id",
            "2222222222222222222222222222222222222222222222222222222222222222",
            "--source-provenance-digest",
            "3333333333333333333333333333333333333333333333333333333333333333",
            "--tool-evidence-digest",
            "4444444444444444444444444444444444444444444444444444444444444444",
        ]
        .into_iter()
        .map(OsString::from)
        .collect()
    }

    #[test]
    fn exact_closed_arguments_parse_in_any_pair_order() {
        let parsed = BuildArguments::parse(exact()).expect("exact arguments");
        assert_eq!(parsed.jobs, 8);
        assert_eq!(parsed.reported_version, "0.32.15");
        assert_eq!(parsed.build_revision.len(), 40);
    }

    #[test]
    fn duplicate_unknown_missing_and_invalid_arguments_fail_closed() {
        let mut duplicate = exact();
        duplicate.extend([OsString::from("--offline")]);
        assert_eq!(
            BuildArguments::parse(duplicate),
            Err(BuildError::InvalidArguments)
        );
        let mut unknown = exact();
        unknown.extend([OsString::from("--unknown")]);
        assert_eq!(
            BuildArguments::parse(unknown),
            Err(BuildError::InvalidArguments)
        );
        let mut excessive = exact();
        let jobs = excessive
            .iter()
            .position(|value| value == "--jobs")
            .expect("jobs");
        excessive[jobs + 1] = OsString::from("65");
        assert_eq!(
            BuildArguments::parse(excessive),
            Err(BuildError::InvalidArguments)
        );
    }
}
