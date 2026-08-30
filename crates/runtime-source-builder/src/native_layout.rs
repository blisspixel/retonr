use std::{fs, path::Path};

use crate::BuildError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeMemberSpec {
    name: &'static str,
    copy_from: &'static str,
    link_target: Option<&'static str>,
}

pub(super) fn assemble(output: &Path) -> Result<(), BuildError> {
    let installed = output.join("work/native-install/lib/ollama");
    verify_install(&installed)?;
    let library = output.join("lib");
    fs::create_dir(&library).map_err(|_| BuildError::OutputInvalid)?;
    let destination = library.join("ollama");
    fs::create_dir(&destination).map_err(|_| BuildError::OutputInvalid)?;
    for specification in NATIVE_MEMBERS {
        fs::copy(
            installed.join(specification.copy_from),
            destination.join(specification.name),
        )
        .map_err(|_| BuildError::OutputInvalid)?;
    }
    Ok(())
}

fn verify_install(installed: &Path) -> Result<(), BuildError> {
    let mut observed = fs::read_dir(installed)
        .map_err(|_| BuildError::NativeLayoutInvalid)?
        .map(|entry| {
            entry
                .map_err(|_| BuildError::NativeLayoutInvalid)?
                .file_name()
                .into_string()
                .map_err(|_name| BuildError::NativeLayoutInvalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    observed.sort_unstable();
    let expected = NATIVE_MEMBERS
        .iter()
        .map(|specification| specification.name)
        .collect::<Vec<_>>();
    if observed != expected {
        return Err(BuildError::NativeLayoutInvalid);
    }
    for specification in NATIVE_MEMBERS {
        let path = installed.join(specification.name);
        let metadata = fs::symlink_metadata(&path).map_err(|_| BuildError::NativeLayoutInvalid)?;
        match specification.link_target {
            Some(expected_target)
                if metadata.file_type().is_symlink()
                    && fs::read_link(&path).ok().as_deref() == Some(Path::new(expected_target)) => {
            }
            None if direct_regular_file(&metadata) => {}
            Some(_) | None => return Err(BuildError::NativeLayoutInvalid),
        }
    }
    Ok(())
}

fn direct_regular_file(metadata: &fs::Metadata) -> bool {
    if !metadata.is_file() || metadata.len() == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        if metadata.nlink() != 1 {
            return false;
        }
    }
    true
}

const NATIVE_MEMBERS: [NativeMemberSpec; 31] = [
    alias(
        "libggml-base.so",
        "libggml-base.so.0.20.2",
        "libggml-base.so.0",
    ),
    alias(
        "libggml-base.so.0",
        "libggml-base.so.0.20.2",
        "libggml-base.so.0.20.2",
    ),
    direct("libggml-base.so.0.20.2"),
    direct("libggml-cpu-alderlake.so"),
    direct("libggml-cpu-cannonlake.so"),
    direct("libggml-cpu-cascadelake.so"),
    direct("libggml-cpu-cooperlake.so"),
    direct("libggml-cpu-haswell.so"),
    direct("libggml-cpu-icelake.so"),
    direct("libggml-cpu-ivybridge.so"),
    direct("libggml-cpu-piledriver.so"),
    direct("libggml-cpu-sandybridge.so"),
    direct("libggml-cpu-sapphirerapids.so"),
    direct("libggml-cpu-skylakex.so"),
    direct("libggml-cpu-sse42.so"),
    direct("libggml-cpu-x64.so"),
    direct("libggml-cpu-zen4.so"),
    alias("libggml.so", "libggml.so.0.20.2", "libggml.so.0"),
    alias("libggml.so.0", "libggml.so.0.20.2", "libggml.so.0.20.2"),
    direct("libggml.so.0.20.2"),
    alias(
        "libllama-common.so",
        "libllama-common.so.0.1.2",
        "libllama-common.so.0",
    ),
    alias(
        "libllama-common.so.0",
        "libllama-common.so.0.1.2",
        "libllama-common.so.0.1.2",
    ),
    direct("libllama-common.so.0.1.2"),
    direct("libllama-server-impl.so"),
    alias("libllama.so", "libllama.so.0.1.2", "libllama.so.0"),
    alias("libllama.so.0", "libllama.so.0.1.2", "libllama.so.0.1.2"),
    direct("libllama.so.0.1.2"),
    alias("libmtmd.so", "libmtmd.so.0.1.2", "libmtmd.so.0"),
    alias("libmtmd.so.0", "libmtmd.so.0.1.2", "libmtmd.so.0.1.2"),
    direct("libmtmd.so.0.1.2"),
    direct("llama-server"),
];

const fn direct(name: &'static str) -> NativeMemberSpec {
    NativeMemberSpec {
        name,
        copy_from: name,
        link_target: None,
    }
}

const fn alias(
    name: &'static str,
    copy_from: &'static str,
    link_target: &'static str,
) -> NativeMemberSpec {
    NativeMemberSpec {
        name,
        copy_from,
        link_target: Some(link_target),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_member_contract_is_sorted_unique_and_closed() {
        let names = NATIVE_MEMBERS
            .iter()
            .map(|specification| specification.name)
            .collect::<Vec<_>>();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
        for specification in NATIVE_MEMBERS {
            assert!(!specification.name.contains('/') && !specification.name.contains('\\'));
            assert!(
                NATIVE_MEMBERS
                    .iter()
                    .any(|candidate| candidate.name == specification.copy_from
                        && candidate.link_target.is_none())
            );
            if let Some(target) = specification.link_target {
                assert!(!target.contains('/') && !target.contains('\\'));
                assert!(
                    NATIVE_MEMBERS
                        .iter()
                        .any(|candidate| candidate.name == target)
                );
            }
        }
        let parameters: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../support/runtime-source-build/ollama-v0.32.15-parameters.json"
        ))
        .expect("canonical parameters");
        let aliases = NATIVE_MEMBERS
            .iter()
            .filter_map(|specification| {
                specification.link_target.map(|link_target| {
                    serde_json::json!({
                        "alias": specification.name,
                        "link_target": link_target,
                        "materialized_from": specification.copy_from
                    })
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            parameters["native_library_alias_mode"],
            "materialize_as_regular_files"
        );
        assert_eq!(
            parameters["native_library_aliases"],
            serde_json::Value::Array(aliases)
        );
    }

    #[test]
    fn incomplete_native_install_fails_before_output_creation() {
        let root = tempfile::tempdir().expect("temporary output");
        fs::create_dir_all(root.path().join("work/native-install/lib/ollama"))
            .expect("incomplete installed tree");
        assert_eq!(assemble(root.path()), Err(BuildError::NativeLayoutInvalid));
        assert!(!root.path().join("lib").exists());
    }

    #[cfg(unix)]
    #[test]
    fn approved_aliases_become_direct_regular_files_and_drift_fails_closed() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("temporary output");
        let installed = root.path().join("work/native-install/lib/ollama");
        fs::create_dir_all(&installed).expect("installed tree");
        for specification in NATIVE_MEMBERS {
            if let Some(target) = specification.link_target {
                symlink(target, installed.join(specification.name)).expect("native alias");
            } else {
                fs::write(
                    installed.join(specification.name),
                    format!("{}\n", specification.name),
                )
                .expect("native member");
            }
        }
        assemble(root.path()).expect("exact native layout");
        for specification in NATIVE_MEMBERS {
            let copied = root.path().join("lib/ollama").join(specification.name);
            let metadata = fs::symlink_metadata(&copied).expect("copied metadata");
            assert!(metadata.is_file());
            assert!(!metadata.file_type().is_symlink());
            assert_eq!(
                fs::read(copied).expect("copied bytes"),
                fs::read(installed.join(specification.copy_from)).expect("source bytes")
            );
        }

        let drifted = tempfile::tempdir().expect("drifted output");
        let drifted_install = drifted.path().join("work/native-install/lib/ollama");
        fs::create_dir_all(&drifted_install).expect("drifted installed tree");
        for specification in NATIVE_MEMBERS {
            if let Some(target) = specification.link_target {
                let target = if specification.name == "libggml.so" {
                    "unexpected.so"
                } else {
                    target
                };
                symlink(target, drifted_install.join(specification.name))
                    .expect("drifted native alias");
            } else {
                fs::write(drifted_install.join(specification.name), b"member\n")
                    .expect("drifted native member");
            }
        }
        assert_eq!(
            assemble(drifted.path()),
            Err(BuildError::NativeLayoutInvalid)
        );
    }
}
