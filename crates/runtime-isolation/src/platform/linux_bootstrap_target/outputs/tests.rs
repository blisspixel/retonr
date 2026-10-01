use std::{
    fs,
    os::unix::fs::{PermissionsExt as _, symlink},
};

use super::*;

fn fixture() -> (tempfile::TempDir, File, File) {
    let directory = tempfile::tempdir().expect("root");
    fs::create_dir(directory.path().join("target")).expect("target");
    fs::create_dir(directory.path().join("target/release")).expect("release");
    fs::create_dir(directory.path().join("output")).expect("output");
    for build in recipe::BUILDS {
        let path = directory.path().join("target").join(build.output());
        fs::write(
            &path,
            [b"\x7fELF".as_slice(), build.program.as_bytes()].concat(),
        )
        .expect("program");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("mode");
    }
    let target = File::open(directory.path().join("target")).expect("target handle");
    let output = File::open(directory.path().join("output")).expect("output handle");
    (directory, target, output)
}

#[test]
fn complete_output_copies_exact_four_executable_bytes_and_refuses_replacement() {
    let (directory, target, output) = fixture();
    publish(&target, &output).expect("publish fixed outputs");
    assert_eq!(
        fs::read_dir(directory.path().join("output"))
            .expect("outputs")
            .count(),
        4
    );
    for build in recipe::BUILDS {
        let path = directory.path().join("output").join(build.program);
        assert_eq!(
            fs::read(&path).expect("copy"),
            fs::read(directory.path().join("target").join(build.output())).expect("candidate")
        );
        assert_ne!(fs::metadata(path).expect("metadata").mode() & 0o111, 0);
    }
    assert_eq!(
        publish(&target, &output),
        Err(HelperFailure::BootstrapRootVerification)
    );
}

#[test]
fn all_candidates_are_validated_before_copying_missing_script_alias_or_nonexecutable() {
    for invalid in ["missing", "script", "symlink", "hardlink", "nonexecutable"] {
        let (directory, target, output) = fixture();
        let candidate = directory
            .path()
            .join("target")
            .join(recipe::BUILDS[3].output());
        match invalid {
            "missing" => fs::remove_file(candidate).expect("remove"),
            "script" => fs::write(candidate, b"#!/bin/sh\n").expect("script"),
            "symlink" => {
                fs::remove_file(&candidate).expect("remove");
                symlink(recipe::BUILDS[0].program, candidate).expect("link");
            }
            "hardlink" => fs::hard_link(&candidate, directory.path().join("alias")).expect("alias"),
            "nonexecutable" => {
                fs::set_permissions(candidate, fs::Permissions::from_mode(0o644)).expect("mode");
            }
            _ => unreachable!(),
        }
        assert_eq!(
            publish(&target, &output),
            Err(HelperFailure::BootstrapRootVerification)
        );
        assert_eq!(
            fs::read_dir(directory.path().join("output"))
                .expect("outputs")
                .count(),
            0
        );
    }
}

#[test]
fn anchored_files_refuse_symlink_parent_special_files_and_oversized_programs() {
    let (directory, target, _) = fixture();
    symlink("release", directory.path().join("target/alias")).expect("parent alias");
    assert!(
        open_regular(
            &target,
            &format!("alias/{}", recipe::BUILDS[0].program),
            true
        )
        .is_err()
    );
    assert!(open_regular(&target, "release", false).is_err());
    let huge = directory.path().join("target/huge");
    let file = File::create(&huge).expect("large");
    file.set_len(MAXIMUM_PROGRAM_BYTES + 1).expect("size");
    fs::set_permissions(huge, fs::Permissions::from_mode(0o755)).expect("mode");
    assert!(open_regular(&target, "huge", true).is_err());
}

fn cargo_alias(directory: &tempfile::TempDir, build: recipe::Build) -> std::path::PathBuf {
    let deps = directory.path().join("target/release/deps");
    fs::create_dir_all(&deps).expect("deps");
    let alias = deps.join(format!(
        "{}-0123456789abcdef",
        build.program.replace('-', "_")
    ));
    fs::hard_link(directory.path().join("target").join(build.output()), &alias)
        .expect("Cargo alias");
    alias
}

#[test]
fn exact_cargo_aliases_materialize_single_link_published_outputs() {
    let (directory, target, output) = fixture();
    for build in recipe::BUILDS {
        cargo_alias(&directory, build);
    }
    publish(&target, &output).expect("Cargo outputs");
    for build in recipe::BUILDS {
        let published = directory.path().join("output").join(build.program);
        assert_eq!(fs::metadata(&published).expect("published").nlink(), 1);
        assert_eq!(
            fs::read(published).expect("bytes"),
            fs::read(directory.path().join("target").join(build.output())).expect("source")
        );
    }
}

#[test]
fn cargo_aliases_refuse_extra_links_foreign_names_symlinks_and_replacement() {
    for invalid in [
        "extra",
        "foreign",
        "symlink",
        "replace-source",
        "replace-alias",
        "change",
    ] {
        let (directory, target, output) = fixture();
        let build = recipe::BUILDS[0];
        let alias = cargo_alias(&directory, build);
        let candidate = cargo_alias::Candidate::open(&target, build).expect("held candidate");
        let source = directory.path().join("target").join(build.output());
        match invalid {
            "extra" => fs::hard_link(&source, directory.path().join("extra")).expect("third link"),
            "foreign" => fs::rename(&alias, alias.with_file_name("foreign-0123456789abcdef"))
                .expect("rename"),
            "symlink" => {
                fs::remove_file(&alias).expect("unlink");
                symlink(&source, &alias).expect("symlink");
            }
            "replace-source" => {
                fs::remove_file(&source).expect("unlink");
                fs::copy(&alias, &source).expect("replace");
            }
            "replace-alias" => {
                fs::remove_file(&alias).expect("unlink");
                fs::copy(&source, &alias).expect("replace");
            }
            "change" => fs::write(&source, b"changed").expect("change"),
            _ => unreachable!(),
        }
        assert!(candidate.revalidate(&target).is_err());
        if matches!(invalid, "extra" | "foreign" | "change") {
            assert!(publish(&target, &output).is_err());
        }
        assert_eq!(
            fs::read_dir(directory.path().join("output"))
                .expect("output")
                .count(),
            0
        );
    }
}

#[test]
fn a_matching_deps_symlink_cannot_authorize_an_unrelated_hardlink() {
    let (directory, target, output) = fixture();
    let build = recipe::BUILDS[0];
    let source = directory.path().join("target").join(build.output());
    fs::hard_link(&source, directory.path().join("unrelated")).expect("foreign alias");
    fs::create_dir(directory.path().join("target/release/deps")).expect("deps");
    symlink(
        &source,
        directory.path().join("target/release/deps").join(format!(
            "{}-0123456789abcdef",
            build.program.replace('-', "_")
        )),
    )
    .expect("matching symlink");
    assert!(publish(&target, &output).is_err());
    assert_eq!(
        fs::read_dir(directory.path().join("output"))
            .expect("output")
            .count(),
        0
    );
}

#[test]
fn a_single_link_candidate_cannot_be_replaced_after_acquisition() {
    let (directory, target, _) = fixture();
    let build = recipe::BUILDS[0];
    let candidate = cargo_alias::Candidate::open(&target, build).expect("candidate");
    let path = directory.path().join("target").join(build.output());
    let bytes = fs::read(&path).expect("bytes");
    fs::remove_file(&path).expect("unlink");
    fs::write(&path, bytes).expect("same bytes different inode");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("mode");
    assert!(candidate.revalidate(&target).is_err());
}
