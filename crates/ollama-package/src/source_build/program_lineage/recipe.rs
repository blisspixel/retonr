use crate::json::validate_unique_json;

use super::{
    RETAINED_PROGRAM_BUILD_RECIPE_PROCEDURE_ID, RETAINED_PROGRAM_BUILD_RECIPE_PROCEDURE_VERSION,
    RetainedProgramBuildRecipeId,
};
use crate::source_build::{RuntimeSourceBuildInputError, sequence_digest};

pub(super) const MAXIMUM_RECIPE_BYTES: u64 = 64 * 1024;
const RECIPE_DOMAIN: &[u8] = b"runtime-source-build/retained-program-build-recipe/v2";
const EXPECTED_RECIPE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../support/runtime-source-build/retained-program-build-recipe-v2.json"
));

pub(super) fn parse(
    bytes: &[u8],
) -> Result<RetainedProgramBuildRecipeId, RuntimeSourceBuildInputError> {
    if bytes.len() > usize::try_from(MAXIMUM_RECIPE_BYTES).unwrap_or(usize::MAX)
        || bytes.last() != Some(&b'\n')
        || bytes.get(bytes.len().saturating_sub(2)) == Some(&b'\n')
    {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    }
    let body = bytes
        .strip_suffix(b"\n")
        .ok_or(RuntimeSourceBuildInputError::InvalidProgramLineage)?;
    validate_unique_json(body).map_err(|()| RuntimeSourceBuildInputError::InvalidProgramLineage)?;
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|_| RuntimeSourceBuildInputError::InvalidProgramLineage)?;
    if serde_json::to_vec(&value)
        .map_err(|_| RuntimeSourceBuildInputError::InvalidProgramLineage)?
        != body
        || bytes != EXPECTED_RECIPE
        || value
            .get("procedure_id")
            .and_then(serde_json::Value::as_str)
            != Some(RETAINED_PROGRAM_BUILD_RECIPE_PROCEDURE_ID)
        || value
            .get("procedure_version")
            .and_then(serde_json::Value::as_u64)
            != Some(u64::from(RETAINED_PROGRAM_BUILD_RECIPE_PROCEDURE_VERSION))
        || value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            != Some(2)
    {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    }
    Ok(RetainedProgramBuildRecipeId(sequence_digest(
        RECIPE_DOMAIN,
        [bytes],
    )))
}

#[cfg(test)]
mod tests {
    use rewrite_types::Digest;

    use super::*;

    #[test]
    fn checked_in_recipe_is_exact_and_domain_separated() {
        let parsed = parse(EXPECTED_RECIPE).expect("checked-in recipe");
        assert_ne!(parsed.digest(), &Digest::sha256(EXPECTED_RECIPE));
        assert!(parse(&EXPECTED_RECIPE[..EXPECTED_RECIPE.len() - 1]).is_err());
        let mut doubled_lf = EXPECTED_RECIPE.to_vec();
        doubled_lf.push(b'\n');
        assert!(parse(&doubled_lf).is_err());
    }

    #[test]
    fn recipe_fixes_default_musl_host_builds_and_static_output_checks() {
        let value = checked_recipe();
        assert_fixed_builds(&value);
        assert_fixed_rust_distribution(&value);
        assert_static_link_inputs(&value);
        assert_isolated_execution_and_io(&value);
    }

    fn checked_recipe() -> serde_json::Value {
        let body = EXPECTED_RECIPE.strip_suffix(b"\n").expect("terminal LF");
        serde_json::from_slice(body).expect("recipe JSON")
    }

    fn assert_fixed_builds(value: &serde_json::Value) {
        let builds = value["builds"].as_array().expect("builds");
        assert_eq!(builds.len(), 4);
        for (build, (name, output)) in builds.iter().zip([
            (
                "isolation_helper",
                "release/rewrite-runtime-isolation-helper",
            ),
            (
                "source_archive_preparation",
                "release/rewrite-runtime-source-archive",
            ),
            ("source_builder", "release/rewrite-runtime-source-builder"),
            (
                "source_manifest_preparation",
                "release/rewrite-runtime-source-manifest",
            ),
        ]) {
            let arguments = build["arguments"].as_array().expect("build arguments");
            assert_eq!(build["name"], name);
            assert_eq!(build["program"], "/toolchain/bin/cargo");
            assert_eq!(build["output_relative_path"], output);
            assert!(arguments.iter().all(|argument| argument != "--target"));
            assert!(
                arguments
                    .iter()
                    .all(|argument| argument != "x86_64-unknown-linux-musl")
            );
        }
    }

    fn assert_fixed_rust_distribution(value: &serde_json::Value) {
        assert_eq!(
            value["filesystem"]["rust_distribution_extraction"]["required_roots"],
            serde_json::json!([
                "cargo-1.97.1-x86_64-unknown-linux-musl",
                "rustc-1.97.1-x86_64-unknown-linux-musl",
                "rust-std-1.97.1-x86_64-unknown-linux-musl"
            ])
        );
        assert_eq!(
            value["filesystem"]["rust_distribution_extraction"]["compilation_target_selection"],
            "retained_rustc_default_host"
        );
        assert_eq!(
            value["filesystem"]["rust_distribution_extraction"]["host_target"],
            "x86_64-unknown-linux-musl"
        );
        assert_eq!(
            value["output_validation"]["target"],
            "x86_64-unknown-linux-musl"
        );
    }

    fn assert_static_link_inputs(value: &serde_json::Value) {
        let rustflags = value["environment"]["variables"]
            .as_array()
            .expect("environment variables")
            .iter()
            .find(|variable| variable["name"] == "RUSTFLAGS")
            .and_then(|variable| variable["value"].as_str())
            .expect("RUSTFLAGS");
        assert!(
            rustflags
                .contains("linker=/toolchain/lib/rustlib/x86_64-unknown-linux-musl/bin/rust-lld")
        );
        assert!(rustflags.contains("linker-flavor=ld.lld"));
        assert!(!rustflags.contains("link-self-contained"));
        assert!(rustflags.contains("-L native=/lib"));
        assert!(rustflags.contains("-L native=/usr/lib"));
        assert_eq!(
            value["host"]["minirootfs"]["libc_linker_name"],
            serde_json::json!({
                "path": "/lib/libc.so",
                "stored_target": "ld-musl-x86_64.so.1",
                "validation": "exact_relative_symlink_same_regular_device_inode_and_payload_digest"
            })
        );
        assert_eq!(
            value["host"]["libgcc"]["linker_name"],
            serde_json::json!({
                "path": "/usr/lib/libgcc_s.so",
                "stored_target": "libgcc_s.so.1",
                "validation": "exact_relative_symlink_same_regular_device_inode_and_payload_digest"
            })
        );
    }

    fn assert_isolated_execution_and_io(value: &serde_json::Value) {
        assert_eq!(
            value["execution"]["namespace_flags"],
            serde_json::json!([
                "CLONE_NEWUSER",
                "CLONE_NEWNS",
                "CLONE_NEWNET",
                "CLONE_NEWPID",
                "CLONE_NEWIPC",
                "CLONE_NEWUTS"
            ])
        );
        assert_eq!(
            value["filesystem"]["input_snapshot"]["backing"],
            "private_tmpfs"
        );
        assert_eq!(
            value["filesystem"]["input_snapshot"]["copy_source"],
            "retained_descriptors"
        );
        assert_eq!(
            value["filesystem"]["input_snapshot"]["protection"],
            serde_json::json!([
                "private_tmpfs_copy",
                "normalized_read_only_modes",
                "read_only_recursive_bind_remount"
            ])
        );
        assert_eq!(
            value["filesystem"]["input_snapshot"]["reject_drvfs_bind_mounts"],
            true
        );
        assert_eq!(value["reproducibility"]["attempts"], 2);
        assert_eq!(
            value["execution"]["live_result_authority"],
            "opaque_nonserializable_execution_only"
        );
        assert_eq!(
            value["execution"]["required_static_closures"],
            serde_json::json!(["cargo_source", "license_evidence", "upstream_release"])
        );
        assert_eq!(
            value["execution"]["root_transition"]["required_live_evidence"],
            serde_json::json!([
                "alpine_libc_linker_name_digest",
                "alpine_libgcc_linker_name_digest",
                "normalized_alpine_link_plan_digest",
                "private_proc_mtab_terminal_validation"
            ])
        );
        assert!(
            value["environment"]["variables"]
                .as_array()
                .expect("environment variables")
                .iter()
                .all(|variable| variable["name"] != "HOME")
        );
        assert_eq!(
            value["output_validation"]["reject_program_headers"],
            serde_json::json!(["PT_INTERP"])
        );
        assert_eq!(
            value["output_validation"]["reject_dynamic_tags"],
            serde_json::json!(["DT_NEEDED"])
        );
    }
}
