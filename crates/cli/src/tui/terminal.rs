//! Terminal lifecycle with restoration on initialization and operation failures.

use std::{
    io::{self, Stdout},
    panic::PanicHookInfo,
    sync::Arc,
};

use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub(super) type NativeTerminal = Terminal<CrosstermBackend<Stdout>>;

pub(super) trait Lifecycle {
    fn raw(&mut self, enabled: bool) -> io::Result<()>;
    fn alternate(&mut self, enabled: bool) -> io::Result<()>;
    fn cursor(&mut self, visible: bool) -> io::Result<()>;
}

struct NativeLifecycle;

impl Lifecycle for NativeLifecycle {
    fn raw(&mut self, enabled: bool) -> io::Result<()> {
        if enabled {
            enable_raw_mode()
        } else {
            disable_raw_mode()
        }
    }
    fn alternate(&mut self, enabled: bool) -> io::Result<()> {
        if enabled {
            execute!(io::stdout(), EnterAlternateScreen)
        } else {
            execute!(io::stdout(), LeaveAlternateScreen)
        }
    }
    fn cursor(&mut self, visible: bool) -> io::Result<()> {
        if visible {
            execute!(io::stdout(), Show)
        } else {
            execute!(io::stdout(), Hide)
        }
    }
}

pub(super) fn with_terminal<T>(
    run: impl FnOnce(&mut NativeTerminal) -> io::Result<T>,
) -> io::Result<T> {
    let _panic_hook = PanicRestoration::install();
    with_lifecycle(&mut NativeLifecycle, || {
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
        run(&mut terminal)
    })
}

type Hook = Arc<dyn Fn(&PanicHookInfo<'_>) + Send + Sync>;

struct PanicRestoration {
    previous: Hook,
}

impl PanicRestoration {
    fn install() -> Self {
        let previous: Hook = Arc::from(std::panic::take_hook());
        let delegate = previous.clone();
        let owner = std::thread::current().id();
        std::panic::set_hook(Box::new(move |info| {
            let mut lifecycle = NativeLifecycle;
            restore_for_panic(
                &mut lifecycle,
                owner,
                std::thread::current().id(),
                cfg!(panic = "abort"),
                || delegate(info),
            );
        }));
        Self { previous }
    }
}

fn restore_for_panic(
    lifecycle: &mut impl Lifecycle,
    owner: std::thread::ThreadId,
    panicking: std::thread::ThreadId,
    aborts: bool,
    delegate: impl FnOnce(),
) {
    // An unwinding reader reports disconnection; the UI restores its own modes.
    // Abort hooks must restore before the process exits because Drop cannot run.
    if aborts || panicking == owner {
        restore_before_panic(lifecycle, delegate);
    } else {
        delegate();
    }
}

fn restore_before_panic(lifecycle: &mut impl Lifecycle, delegate: impl FnOnce()) {
    let _ = lifecycle.cursor(true);
    let _ = lifecycle.alternate(false);
    let _ = lifecycle.raw(false);
    delegate();
}

impl Drop for PanicRestoration {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let previous = self.previous.clone();
            std::panic::set_hook(Box::new(move |info| previous(info)));
        }
    }
}

struct Guard<'a, L: Lifecycle> {
    lifecycle: &'a mut L,
    raw: bool,
    alternate: bool,
    hidden: bool,
}

impl<L: Lifecycle> Guard<'_, L> {
    fn restore(&mut self) -> io::Result<()> {
        let mut error = None;
        if self.hidden {
            self.hidden = false;
            if let Err(failure) = self.lifecycle.cursor(true) {
                error = Some(failure);
            }
        }
        if self.alternate {
            self.alternate = false;
            if let Err(failure) = self.lifecycle.alternate(false) {
                error.get_or_insert(failure);
            }
        }
        if self.raw {
            self.raw = false;
            if let Err(failure) = self.lifecycle.raw(false) {
                error.get_or_insert(failure);
            }
        }
        error.map_or(Ok(()), Err)
    }
}

impl<L: Lifecycle> Drop for Guard<'_, L> {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub(super) fn with_lifecycle<L: Lifecycle, T>(
    lifecycle: &mut L,
    run: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
    let mut guard = Guard {
        lifecycle,
        raw: true,
        alternate: false,
        hidden: false,
    };
    guard.lifecycle.raw(true)?;
    guard.alternate = true;
    guard.lifecycle.alternate(true)?;
    guard.hidden = true;
    guard.lifecycle.cursor(false)?;
    let result = run();
    let restored = guard.restore();
    match result {
        Ok(value) => restored.map(|()| value),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
