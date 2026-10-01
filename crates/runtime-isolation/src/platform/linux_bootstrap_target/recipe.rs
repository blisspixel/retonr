use std::{ffi::OsString, fs::File, io::Read as _};

use super::{HelperFailure, outputs};

pub(super) const CARGO: &str = "/toolchain/bin/cargo";
const RECIPE_PATH: &str = "lineage/build-recipe-v2.json";
const MAXIMUM_RECIPE_BYTES: u64 = 64 * 1024;
pub(super) const EXPECTED_RECIPE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../support/runtime-source-build/retained-program-build-recipe-v2.json"
));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Build {
    pub(super) package: &'static str,
    pub(super) program: &'static str,
}

impl Build {
    pub(super) fn arguments(self) -> Vec<OsString> {
        [
            "build",
            "--frozen",
            "--release",
            "--package",
            self.package,
            "--bin",
            self.program,
        ]
        .map(OsString::from)
        .to_vec()
    }

    pub(super) fn output(self) -> String {
        format!("release/{}", self.program)
    }
}

pub(super) const BUILDS: [Build; 4] = [
    Build {
        package: "rewrite-runtime-isolation",
        program: "rewrite-runtime-isolation-helper",
    },
    Build {
        package: "rewrite-runtime-source-builder",
        program: "rewrite-runtime-source-archive",
    },
    Build {
        package: "rewrite-runtime-source-builder",
        program: "rewrite-runtime-source-builder",
    },
    Build {
        package: "rewrite-runtime-source-builder",
        program: "rewrite-runtime-source-manifest",
    },
];

pub(super) fn read_environment(input: &File) -> Result<Vec<(OsString, OsString)>, HelperFailure> {
    let file = outputs::open_regular(input, RECIPE_PATH, false)?;
    let mut bytes = Vec::new();
    file.take(MAXIMUM_RECIPE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    environment(&bytes)
}

fn environment(bytes: &[u8]) -> Result<Vec<(OsString, OsString)>, HelperFailure> {
    if bytes != EXPECTED_RECIPE
        || bytes.len() > usize::try_from(MAXIMUM_RECIPE_BYTES).unwrap_or(usize::MAX)
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    // Compare bytes first. JSON avoids a second copy of the fixed environment;
    // no caller-provided command is interpreted or executed.
    value["environment"]["variables"]
        .as_array()
        .ok_or(HelperFailure::BootstrapRootVerification)?
        .iter()
        .map(|variable| {
            Ok((
                OsString::from(
                    variable["name"]
                        .as_str()
                        .ok_or(HelperFailure::BootstrapRootVerification)?,
                ),
                OsString::from(
                    variable["value"]
                        .as_str()
                        .ok_or(HelperFailure::BootstrapRootVerification)?,
                ),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_commands_and_environment_match_the_single_canonical_recipe() {
        let value: serde_json::Value = serde_json::from_slice(EXPECTED_RECIPE).expect("recipe");
        let commands = value["builds"].as_array().expect("builds");
        assert_eq!(commands.len(), BUILDS.len());
        for (declared, build) in commands.iter().zip(BUILDS) {
            assert_eq!(declared["program"], CARGO);
            assert_eq!(declared["output_relative_path"], build.output());
            let arguments: Vec<_> = build
                .arguments()
                .iter()
                .map(|value| value.to_string_lossy().into_owned())
                .collect();
            assert_eq!(declared["arguments"], serde_json::json!(arguments));
        }
        let environment = environment(EXPECTED_RECIPE).expect("environment");
        assert_eq!(value["environment"]["clear"], true);
        assert!(
            environment
                .iter()
                .any(|(name, value)| name == "CARGO_NET_OFFLINE" && value == "true")
        );
        assert!(environment.iter().all(|(name, _)| {
            !name
                .to_string_lossy()
                .starts_with("RETONR_CONTROLLED_BUILD_")
        }));
    }

    #[test]
    fn recipe_rejects_changed_commands_environment_trailing_bytes_and_oversize() {
        let mut changed = EXPECTED_RECIPE.to_vec();
        changed[0] = b' ';
        for invalid in [
            EXPECTED_RECIPE
                .iter()
                .copied()
                .chain(*b"\n")
                .collect::<Vec<_>>(),
            changed,
            vec![b' '; 65_537],
        ] {
            assert!(environment(&invalid).is_err());
        }
    }
}
