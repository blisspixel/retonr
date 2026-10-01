use super::*;

#[derive(Default)]
struct Script {
    steps: Vec<&'static str>,
    fail: Option<&'static str>,
}

impl Script {
    fn step(&mut self, name: &'static str) -> io::Result<()> {
        self.steps.push(name);
        if self.fail == Some(name) {
            Err(io::Error::other("scripted terminal failure"))
        } else {
            Ok(())
        }
    }
}

impl Lifecycle for Script {
    fn raw(&mut self, enabled: bool) -> io::Result<()> {
        self.step(if enabled { "raw-on" } else { "raw-off" })
    }
    fn alternate(&mut self, enabled: bool) -> io::Result<()> {
        self.step(if enabled {
            "alternate-on"
        } else {
            "alternate-off"
        })
    }
    fn cursor(&mut self, visible: bool) -> io::Result<()> {
        self.step(if visible {
            "cursor-show"
        } else {
            "cursor-hide"
        })
    }
}

#[test]
fn normal_exit_and_operation_failure_restore_every_terminal_mode() {
    for fail_operation in [false, true] {
        let mut script = Script::default();
        let result = with_lifecycle(&mut script, || {
            if fail_operation {
                Err(io::Error::other("draw failure"))
            } else {
                Ok(42)
            }
        });
        assert_eq!(result.is_err(), fail_operation);
        assert_eq!(
            script.steps,
            [
                "raw-on",
                "alternate-on",
                "cursor-hide",
                "cursor-show",
                "alternate-off",
                "raw-off"
            ]
        );
    }
}

#[test]
fn initialization_failure_and_cleanup_failure_still_restore_remaining_modes() {
    for (failure, expected) in [
        ("raw-on", vec!["raw-on", "raw-off"]),
        (
            "alternate-on",
            vec!["raw-on", "alternate-on", "alternate-off", "raw-off"],
        ),
        (
            "cursor-hide",
            vec![
                "raw-on",
                "alternate-on",
                "cursor-hide",
                "cursor-show",
                "alternate-off",
                "raw-off",
            ],
        ),
        (
            "cursor-show",
            vec![
                "raw-on",
                "alternate-on",
                "cursor-hide",
                "cursor-show",
                "alternate-off",
                "raw-off",
            ],
        ),
        (
            "alternate-off",
            vec![
                "raw-on",
                "alternate-on",
                "cursor-hide",
                "cursor-show",
                "alternate-off",
                "raw-off",
            ],
        ),
        (
            "raw-off",
            vec![
                "raw-on",
                "alternate-on",
                "cursor-hide",
                "cursor-show",
                "alternate-off",
                "raw-off",
            ],
        ),
    ] {
        let mut script = Script {
            fail: Some(failure),
            ..Script::default()
        };
        assert!(with_lifecycle(&mut script, || Ok(())).is_err());
        assert_eq!(script.steps, expected);
    }
}

#[test]
fn unwinding_operation_restores_terminal_state() {
    let mut script = Script::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: io::Result<()> = with_lifecycle(&mut script, || panic!("scripted unwind"));
    }));
    assert!(result.is_err());
    assert_eq!(
        &script.steps[3..],
        ["cursor-show", "alternate-off", "raw-off"]
    );
}

#[test]
fn panic_restoration_attempts_all_modes_before_delegating_even_when_one_fails() {
    let mut script = Script {
        fail: Some("cursor-show"),
        ..Script::default()
    };
    let mut delegated = false;
    restore_before_panic(&mut script, || delegated = true);
    assert!(delegated);
    assert_eq!(script.steps, ["cursor-show", "alternate-off", "raw-off"]);
}

#[test]
fn unwinding_reader_panic_defers_cleanup_to_ui_but_owner_and_abort_panics_restore() {
    let owner = std::thread::current().id();
    let reader = std::thread::spawn(|| std::thread::current().id())
        .join()
        .expect("reader thread identity");
    assert_ne!(owner, reader);
    for (panicking, aborts, restored) in [
        (reader, false, false),
        (owner, false, true),
        (reader, true, true),
        (owner, true, true),
    ] {
        let mut script = Script::default();
        let mut delegated = false;
        restore_for_panic(&mut script, owner, panicking, aborts, || delegated = true);
        assert!(delegated);
        if restored {
            assert_eq!(script.steps, ["cursor-show", "alternate-off", "raw-off"]);
        } else {
            assert!(
                script.steps.is_empty(),
                "reader must not change modes while UI draws"
            );
        }
    }
}
