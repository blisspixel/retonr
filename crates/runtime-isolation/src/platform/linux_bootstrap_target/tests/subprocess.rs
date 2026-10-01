use std::{fs, process::Command};

use super::*;

const FIXTURE: &str = r#"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
int main(int argc, char **argv) {
    if (argc != 8 || strcmp(argv[1], "build") || strcmp(argv[2], "--frozen") ||
        strcmp(argv[3], "--release") || strcmp(argv[4], "--package") || strcmp(argv[6], "--bin")) return 71;
    if (getenv("RETONR_CONTROLLED_BUILD_CAPABILITY_ABI") || getenv("BOOTSTRAP_AMBIENT_CANARY")) return 72;
    const char *target = getenv("CARGO_TARGET_DIR");
    if (!target || strcmp(getenv("CARGO_NET_OFFLINE"), "true")) return 73;
    fprintf(stderr, "compiled:%s\n", argv[7]);
    if (getenv("FIXTURE_FAIL")) return 70;
    char directory[4096], destination[4096];
    if (snprintf(directory, sizeof directory, "%s/release", target) >= sizeof directory) return 74;
    mkdir(directory, 0755);
    if (snprintf(destination, sizeof destination, "%s/%s", directory, argv[7]) >= sizeof destination) return 74;
    FILE *source = fopen("/proc/self/exe", "rb"), *output = fopen(destination, "wb");
    if (!source || !output) return 75;
    unsigned char buffer[4096]; size_t count;
    while ((count = fread(buffer, 1, sizeof buffer, source))) if (fwrite(buffer, 1, count, output) != count) return 76;
    fclose(source); if (fclose(output)) return 77;
    if (chmod(destination, 0755)) return 78;
    return 0;
}
"#;

#[test]
fn held_process_executes_fixed_builds_clears_ambient_environment_and_publishes_only_on_success() {
    if std::env::var_os("RETONR_BOOTSTRAP_SUBPROCESS_FIXTURE").is_none() {
        let completed = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "platform::linux_bootstrap_target::tests::subprocess::held_process_executes_fixed_builds_clears_ambient_environment_and_publishes_only_on_success",
                "--test-threads=1",
            ])
            .env("RETONR_BOOTSTRAP_SUBPROCESS_FIXTURE", "1")
            .env("BOOTSTRAP_AMBIENT_CANARY", "must-not-reach-compiler")
            .env("RETONR_CONTROLLED_BUILD_CAPABILITY_ABI", "2")
            .output()
            .expect("isolated fixture process");
        assert!(completed.status.success(), "fixture process: {completed:?}");
        return;
    }
    let directory = tempfile::tempdir().expect("compiler fixture");
    let fixture = directory.path().join("compiler.c");
    let executable = directory.path().join("compiler");
    fs::write(&fixture, FIXTURE).expect("fixture source");
    let compiled = Command::new("cc")
        .args(["-O0", "-o"])
        .arg(&executable)
        .arg(fixture)
        .status()
        .expect("compile native fixture");
    assert!(compiled.success());
    for failure in [false, true] {
        let target_path = directory
            .path()
            .join(if failure { "failed-target" } else { "target" });
        let output_path = directory
            .path()
            .join(if failure { "failed-output" } else { "output" });
        fs::create_dir(&target_path).expect("target");
        fs::create_dir(&output_path).expect("output");
        let mut environment = vec![
            (
                OsString::from("CARGO_TARGET_DIR"),
                target_path.as_os_str().to_owned(),
            ),
            (OsString::from("CARGO_NET_OFFLINE"), OsString::from("true")),
        ];
        if failure {
            environment.push((OsString::from("FIXTURE_FAIL"), OsString::from("1")));
        }
        let target = PreparedBootstrapTarget {
            cargo: File::open(&executable).expect("held fixture"),
            target: File::open(&target_path).expect("target"),
            output: File::open(&output_path).expect("output"),
            null: File::open("/dev/null").expect("null"),
            environment,
        };
        let completed = target
            .run_at(directory.path())
            .expect("subprocess completion");
        if failure {
            assert_eq!(
                completed.status(),
                ControlledBuildProcessStatus::ExitCode(70)
            );
            assert_eq!(fs::read_dir(output_path).expect("outputs").count(), 0);
            assert_eq!(
                completed.streams().standard_error(),
                b"compiled:rewrite-runtime-isolation-helper\n"
            );
        } else {
            assert_eq!(completed.status(), ControlledBuildProcessStatus::Success);
            assert_eq!(fs::read_dir(&output_path).expect("outputs").count(), 4);
            for build in recipe::BUILDS {
                assert_eq!(
                    fs::read(output_path.join(build.program)).expect("copied executable"),
                    fs::read(&executable).expect("retained source")
                );
            }
        }
    }
}
