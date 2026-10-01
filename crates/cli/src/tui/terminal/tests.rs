use super::*;
use ratatui::{
    backend::{Backend, TestBackend},
    widgets::Paragraph,
};

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
        restore_for_panic(
            &mut script,
            &RenderGate::default(),
            owner,
            panicking,
            aborts,
            || delegated = true,
        );
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

#[test]
fn background_abort_restoration_waits_for_active_frame_and_refuses_all_later_frames() {
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };
    let deadline = Duration::from_secs(3);
    let gate = Arc::new(RenderGate::default());
    let terminal = Arc::new(Mutex::new(
        Terminal::new(TestBackend::new(40, 10)).expect("terminal"),
    ));
    let (entered, entry) = mpsc::channel();
    let (finish, resume) = mpsc::channel();
    let frame_gate = gate.clone();
    let frame_terminal = terminal.clone();
    let active_frame = std::thread::spawn(move || {
        frame_gate
            .render(|| {
                entered.send(()).expect("frame entered");
                resume.recv_timeout(deadline).expect("finish active frame");
                draw_preview(&frame_terminal, "active frame");
                Ok(())
            })
            .expect("active render allowed");
    });
    entry
        .recv_timeout(deadline)
        .expect("frame holds render lock");
    let (restored, restoration) = mpsc::channel();
    let (delegate_finish, delegate_resume) = mpsc::channel();
    let hook_gate = gate.clone();
    let hook_terminal = terminal.clone();
    let hook = std::thread::spawn(move || {
        hook_gate.restore_for_panic(false, || {
            hook_terminal
                .lock()
                .expect("terminal")
                .backend_mut()
                .show_cursor()
                .expect("restore cursor");
            restored.send(()).expect("restored");
            delegate_resume
                .recv_timeout(deadline)
                .expect("delegated hook finishes");
        });
    });
    let until = Instant::now() + deadline;
    while !gate.panicking.load(Ordering::Acquire) {
        assert!(
            Instant::now() < until,
            "hook sets panic flag before waiting for frame"
        );
        std::thread::yield_now();
    }
    assert!(
        restoration.try_recv().is_err(),
        "active frame must finish before restoration"
    );
    finish.send(()).expect("finish frame");
    active_frame.join().expect("frame completes");
    restoration
        .recv_timeout(deadline)
        .expect("cursor restored after active frame");
    assert!(
        terminal
            .lock()
            .expect("terminal")
            .backend()
            .cursor_visible()
    );
    let later_gate = gate.clone();
    let later_terminal = terminal.clone();
    let later_frame = std::thread::spawn(move || {
        later_gate.render(|| {
            draw_preview(&later_terminal, "must not hide restored cursor");
            Ok(())
        })
    });
    delegate_finish.send(()).expect("delegate completed");
    hook.join().expect("hook completes");
    assert!(later_frame.join().expect("later frame result").is_err());
    assert!(
        terminal
            .lock()
            .expect("terminal")
            .backend()
            .cursor_visible()
    );
}

fn draw_preview(terminal: &Mutex<Terminal<TestBackend>>, text: &str) {
    terminal
        .lock()
        .expect("terminal")
        .draw(|frame| {
            frame.render_widget(Paragraph::new(text), frame.area());
        })
        .expect("draw preview");
}

#[test]
fn owner_panic_restoration_does_not_relock_its_active_frame() {
    let (done, result) = std::sync::mpsc::channel();
    let owner = std::thread::spawn(move || {
        let gate = RenderGate::default();
        let mut script = Script::default();
        gate.render(|| {
            gate.restore_for_panic(true, || restore_before_panic(&mut script, || {}));
            Ok(())
        })
        .expect("owner frame callback returns in this simulation");
        done.send(script.steps).expect("owner reports cleanup");
    });
    assert_eq!(
        result
            .recv_timeout(std::time::Duration::from_secs(3))
            .expect("owner must not deadlock"),
        ["cursor-show", "alternate-off", "raw-off"]
    );
    owner.join().expect("owner completes");
}

#[test]
fn poisoned_render_lock_refuses_frames_but_still_allows_panic_cleanup() {
    let gate = Arc::new(RenderGate::default());
    let poison_gate = gate.clone();
    assert!(
        std::thread::spawn(move || {
            let _guard = poison_gate.render.lock().expect("render lock");
            panic!("scripted frame unwind");
        })
        .join()
        .is_err()
    );
    assert!(gate.render(|| Ok(())).is_err());
    let mut restored = false;
    gate.restore_for_panic(false, || restored = true);
    assert!(restored);
}
