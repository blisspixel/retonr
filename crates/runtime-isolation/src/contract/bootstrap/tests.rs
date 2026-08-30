use std::{fs::File, time::Duration};

use tempfile::NamedTempFile;

use super::*;
use crate::NamespaceIdentity;

fn exact_inputs() -> RetainedProgramBootstrapSignedInputs {
    let inputs = RetainedProgramBootstrapInputKind::ALL
        .into_iter()
        .map(|kind| {
            RetainedProgramBootstrapInputMeasurement::new(
                kind,
                Digest::sha256(kind.relative_path().as_bytes()),
                1,
            )
            .expect("measurement")
        })
        .collect();
    RetainedProgramBootstrapSignedInputs::new(inputs).expect("signed inputs")
}

fn controlled() -> ControlledBuildLaunchSpec {
    ControlledBuildLaunchSpec::new(
        "stage0/source-builder",
        Digest::sha256(b"builder"),
        7,
        Duration::from_secs(1),
    )
    .expect("controlled build")
}

fn busybox_executable() -> AuthenticatedBusyboxExecutable {
    AuthenticatedBusyboxExecutable::new(Digest::sha256(b"busybox executable"), 18)
        .expect("busybox executable")
}

#[test]
fn signed_input_set_is_exact_unique_ordered_and_bounded() {
    let exact = exact_inputs();
    assert_eq!(exact.measurements().len(), 13);
    let mut reordered = exact.measurements().to_vec();
    reordered.swap(0, 1);
    assert!(RetainedProgramBootstrapSignedInputs::new(reordered).is_err());
    let mut missing = exact.measurements().to_vec();
    missing.pop();
    assert!(RetainedProgramBootstrapSignedInputs::new(missing).is_err());
    assert_eq!(
        RetainedProgramBootstrapInputMeasurement::new(
            RetainedProgramBootstrapInputKind::AlpineMinirootfs,
            Digest::sha256(b"empty"),
            0,
        ),
        Err(IsolationError::InvalidBootstrap("signed input size"))
    );
    let excessive = RetainedProgramBootstrapInputKind::ALL
        .into_iter()
        .map(|kind| {
            RetainedProgramBootstrapInputMeasurement::new(
                kind,
                Digest::sha256(kind.relative_path().as_bytes()),
                MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES,
            )
            .expect("individually bounded")
        })
        .collect();
    assert_eq!(
        RetainedProgramBootstrapSignedInputs::new(excessive),
        Err(IsolationError::InvalidBootstrap("signed input aggregate"))
    );
}

#[test]
fn launch_digest_binds_attempt_recipe_inputs_and_controlled_launch() {
    let primary = RetainedProgramBootstrapLaunchSpec::new(
        RetainedProgramBootstrapAttempt::Primary,
        Digest::sha256(b"recipe"),
        6,
        exact_inputs(),
        busybox_executable(),
        controlled(),
    )
    .expect("primary");
    let rebuild = RetainedProgramBootstrapLaunchSpec::new(
        RetainedProgramBootstrapAttempt::Rebuild,
        Digest::sha256(b"recipe"),
        6,
        exact_inputs(),
        busybox_executable(),
        controlled(),
    )
    .expect("rebuild");
    assert_ne!(primary.redacted_digest(), rebuild.redacted_digest());
    assert_eq!(primary.recipe_bytes(), 6);
    assert_eq!(primary.attempt(), RetainedProgramBootstrapAttempt::Primary);
    assert_eq!(primary.recipe_digest(), &Digest::sha256(b"recipe"));
    assert_eq!(primary.signed_inputs().measurements().len(), 13);
    assert_eq!(primary.busybox_executable(), &busybox_executable());
    assert_eq!(primary.controlled_build(), &controlled());
    assert_eq!(
        RetainedProgramBootstrapLaunchSpec::new(
            RetainedProgramBootstrapAttempt::Primary,
            Digest::sha256(b"recipe"),
            0,
            exact_inputs(),
            busybox_executable(),
            controlled(),
        ),
        Err(IsolationError::InvalidBootstrap("recipe size"))
    );
    assert_eq!(
        RetainedProgramBootstrapLaunchSpec::new(
            RetainedProgramBootstrapAttempt::Primary,
            Digest::sha256(b"recipe"),
            MAXIMUM_RECIPE_BYTES + 1,
            exact_inputs(),
            busybox_executable(),
            controlled(),
        ),
        Err(IsolationError::InvalidBootstrap("recipe size"))
    );
    assert_eq!(
        AuthenticatedBusyboxExecutable::new(Digest::sha256(b"empty"), 0),
        Err(IsolationError::InvalidBootstrap("busybox executable size"))
    );
    let changed_busybox = RetainedProgramBootstrapLaunchSpec::new(
        RetainedProgramBootstrapAttempt::Primary,
        Digest::sha256(b"recipe"),
        6,
        exact_inputs(),
        AuthenticatedBusyboxExecutable::new(Digest::sha256(b"changed"), 7)
            .expect("changed busybox"),
        controlled(),
    )
    .expect("changed bootstrap");
    assert_ne!(primary.redacted_digest(), changed_busybox.redacted_digest());
}

#[test]
fn launch_joins_every_signed_measurement_and_recipe_to_input_files() {
    let recipe = NamedTempFile::new().expect("recipe");
    std::fs::write(recipe.path(), b"recipe").expect("write recipe");
    let mut retained = vec![
        ControlledBuildInputFile::new(
            "lineage/build-recipe-v2.json",
            Digest::sha256(b"recipe"),
            6,
            File::open(recipe.path()).expect("open recipe"),
        )
        .expect("recipe input"),
    ];
    let mut files = Vec::new();
    for input in exact_inputs().measurements() {
        let file = NamedTempFile::new().expect("input");
        std::fs::write(file.path(), b"x").expect("write input");
        retained.push(
            ControlledBuildInputFile::new(
                input.relative_path(),
                input.digest().clone(),
                1,
                File::open(file.path()).expect("open input"),
            )
            .expect("input declaration"),
        );
        files.push(file);
    }
    let busybox = NamedTempFile::new().expect("busybox executable");
    std::fs::write(busybox.path(), b"busybox executable").expect("write busybox executable");
    retained.push(
        ControlledBuildInputFile::new(
            busybox_executable().relative_path(),
            busybox_executable().digest().clone(),
            busybox_executable().byte_size(),
            File::open(busybox.path()).expect("open busybox executable"),
        )
        .expect("busybox executable declaration"),
    );
    let specification = RetainedProgramBootstrapLaunchSpec::new(
        RetainedProgramBootstrapAttempt::Primary,
        Digest::sha256(b"recipe"),
        6,
        exact_inputs(),
        busybox_executable(),
        controlled(),
    )
    .expect("specification");
    specification
        .validate_input_files(&retained)
        .expect("joined inputs");
    retained.pop();
    assert_eq!(
        specification.validate_input_files(&retained),
        Err(IsolationError::InvalidBootstrap("input measurement join"))
    );
    drop(files);
    drop(busybox);
}

fn root_evidence() -> RetainedProgramBootstrapRootEvidence {
    RetainedProgramBootstrapRootEvidence {
        root_mount: NamespaceIdentity {
            device: 1,
            inode: 2,
        },
        alpine_installed_database_digest: Digest::sha256(b"database"),
        alpine_loader_digest: Digest::sha256(b"loader"),
        alpine_libc_linker_name_digest: Digest::sha256(b"libc linker name"),
        alpine_libgcc_digest: Digest::sha256(b"libgcc"),
        alpine_libgcc_linker_name_digest: Digest::sha256(b"libgcc linker name"),
        alpine_busybox_digest: Digest::sha256(b"busybox"),
        rust_toolchain_layout_digest: Digest::sha256(b"toolchain"),
        normalized_alpine_link_plan_digest: Digest::sha256(b"link plan"),
        postconditions: RetainedProgramBootstrapRootPostconditions::from_observations([
            true, true, true, true, true,
        ]),
    }
}

#[test]
fn root_canaries_require_every_irreversible_postcondition() {
    let evidence = root_evidence();
    assert!(evidence.all_canaries_passed());
    assert_eq!(
        evidence.root_mount(),
        NamespaceIdentity {
            device: 1,
            inode: 2,
        }
    );
    assert_eq!(evidence.alpine_loader_digest(), &Digest::sha256(b"loader"));
    assert_eq!(
        evidence.alpine_libc_linker_name_digest(),
        &Digest::sha256(b"libc linker name")
    );
    assert_eq!(
        evidence.alpine_installed_database_digest(),
        &Digest::sha256(b"database")
    );
    assert_eq!(evidence.alpine_libgcc_digest(), &Digest::sha256(b"libgcc"));
    assert_eq!(
        evidence.alpine_libgcc_linker_name_digest(),
        &Digest::sha256(b"libgcc linker name")
    );
    assert_eq!(
        evidence.alpine_busybox_digest(),
        &Digest::sha256(b"busybox")
    );
    assert_eq!(
        evidence.rust_toolchain_layout_digest(),
        &Digest::sha256(b"toolchain")
    );
    assert_eq!(
        evidence.normalized_alpine_link_plan_digest(),
        &Digest::sha256(b"link plan")
    );
    assert_ne!(evidence.redacted_digest(), Digest::sha256(b"toolchain"));
    for field in 0..5 {
        let mut changed = root_evidence();
        let mut observations = [true; 5];
        observations[field] = false;
        changed.postconditions =
            RetainedProgramBootstrapRootPostconditions::from_observations(observations);
        assert!(!changed.all_canaries_passed());
    }
    let mut missing_mount = root_evidence();
    missing_mount.root_mount.device = 0;
    assert!(!missing_mount.all_canaries_passed());
    missing_mount.root_mount.device = 1;
    missing_mount.root_mount.inode = 0;
    assert!(!missing_mount.all_canaries_passed());
}

#[test]
fn retained_capability_group_preserves_owned_files_and_declarations() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let program_path = temporary.path().join("program");
    let input_path = temporary.path().join("input");
    let output_path = temporary.path().join("output");
    std::fs::write(&program_path, b"program").expect("program");
    std::fs::write(&input_path, b"input").expect("input file");
    std::fs::write(&output_path, b"output").expect("output file");
    let declaration_path = temporary.path().join("declaration");
    std::fs::write(&declaration_path, b"x").expect("declaration");
    let declarations = vec![
        ControlledBuildInputFile::new(
            "input",
            Digest::sha256(b"x"),
            1,
            File::open(declaration_path).expect("open declaration"),
        )
        .expect("declaration"),
    ];
    let capabilities = RetainedProgramBootstrapCapabilities::new(
        File::open(program_path).expect("open program"),
        File::open(input_path).expect("open input"),
        declarations,
        File::open(output_path).expect("open output"),
    );
    assert_eq!(capabilities.input_files().len(), 1);
    let (program, input, declarations, output) = capabilities.into_parts();
    assert!(program.metadata().expect("program metadata").is_file());
    assert!(input.metadata().expect("input metadata").is_file());
    assert_eq!(declarations[0].relative_path(), "input");
    assert!(output.metadata().expect("output metadata").is_file());
}
