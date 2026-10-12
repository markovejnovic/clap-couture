//! The terminal's modes, put back when a signal ends the process before a prompt could.

use core::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::{
    io::{self, IsTerminal as _},
    os::fd::AsRawFd as _,
};

/// The terminal [`TerminalModes::put_back`] restores; -1 while no guard holds one.
static TERMINAL: AtomicI32 = AtomicI32::new(-1i32);

/// The terminal's local modes, like echo, as the guard found them.
static LOCAL_MODES: AtomicUsize = AtomicUsize::new(0);

/// The signals typed at the terminal that end the process by default.
const KEYBOARD_SIGNALS: [libc::c_int; 2] = [libc::SIGINT, libc::SIGQUIT];

/// The terminal's local modes, put back if Ctrl-C or Ctrl-\\ ends the process while alive, as
/// when a prompt turned echo off. A signal the app handles or ignores keeps its handling.
pub(crate) struct TerminalModes {
    /// Each keyboard signal's action before this guard, where its handler took the signal over.
    previous: [Option<libc::sigaction>; 2],
}

impl TerminalModes {
    /// The handler, as `sigaction` takes it.
    fn handler() -> libc::sighandler_t {
        #[expect(clippy::as_conversions, reason = "`sigaction` takes a handler as an address")]
        let put_back_then_die =
            (Self::put_back_then_die as extern "C" fn(libc::c_int) as *const ()).addr();
        put_back_then_die
    }

    /// Hold the modes of `terminal`, which outlives the guard. `None` when they can't be read.
    fn of(terminal: libc::c_int) -> Option<Self> {
        // SAFETY: an all-zero `termios` is valid storage, only written to.
        let mut modes: libc::termios = unsafe { core::mem::zeroed() };
        // SAFETY: `modes` is live for the call.
        if unsafe { libc::tcgetattr(terminal, &raw mut modes) } != 0i32 {
            return None;
        }
        LOCAL_MODES.store(usize::try_from(modes.c_lflag).ok()?, Ordering::SeqCst);
        TERMINAL.store(terminal, Ordering::SeqCst);
        Some(Self { previous: KEYBOARD_SIGNALS.map(Self::take_over) })
    }

    /// Hold stdin's modes. `None` when it isn't a terminal.
    pub(crate) fn of_stdin() -> Option<Self> {
        let stdin = io::stdin();
        stdin.is_terminal().then(|| stdin.as_raw_fd()).and_then(Self::of)
    }

    /// Restore the terminal's local modes as the guard found them. Async-signal-safe.
    fn put_back() {
        let terminal = TERMINAL.load(Ordering::SeqCst);
        let Ok(local_modes) = libc::tcflag_t::try_from(LOCAL_MODES.load(Ordering::SeqCst)) else {
            return;
        };
        if terminal < 0i32 {
            return;
        }
        // SAFETY: an all-zero `termios` is valid storage, only written to.
        let mut modes: libc::termios = unsafe { core::mem::zeroed() };
        // SAFETY: `modes` is live for the call.
        if unsafe { libc::tcgetattr(terminal, &raw mut modes) } != 0i32 {
            return;
        }
        modes.c_lflag = local_modes;
        // SAFETY: `modes` is the terminal's own, with only its local modes changed.
        unsafe {
            libc::tcsetattr(terminal, libc::TCSANOW, &raw const modes);
        }
    }

    /// The handler: put the modes back, then die of `signal` as it would have killed.
    /// Async-signal-safe: it only touches lock-free atomics and calls `tcgetattr`, `tcsetattr`,
    /// `signal` and `raise`.
    extern "C" fn put_back_then_die(signal: libc::c_int) {
        Self::put_back();
        // SAFETY: the default action is valid for every signal.
        unsafe {
            libc::signal(signal, libc::SIG_DFL);
        }
        // SAFETY: masked while its handler runs, the signal lands, by default, once it returns.
        unsafe {
            libc::raise(signal);
        }
    }

    /// Install the handler for `signal`, returning the action it replaced. `None` when it can't
    /// be, or `signal` doesn't have its default action.
    fn take_over(signal: libc::c_int) -> Option<libc::sigaction> {
        // SAFETY: an all-zero `sigaction` is valid: `SIG_DFL`, no flags, an empty mask.
        let mut action: libc::sigaction = unsafe { core::mem::zeroed() };
        action.sa_sigaction = Self::handler();
        // SAFETY: an all-zero `sigaction` is valid storage, only written to.
        let mut previous: libc::sigaction = unsafe { core::mem::zeroed() };
        // SAFETY: both pointers come from live locals for the length of the call.
        if unsafe { libc::sigaction(signal, &raw const action, &raw mut previous) } != 0i32 {
            return None;
        }
        if previous.sa_sigaction == libc::SIG_DFL {
            return Some(previous);
        }
        // SAFETY: `previous` is the action `sigaction` reported.
        unsafe {
            libc::sigaction(signal, &raw const previous, core::ptr::null_mut());
        }
        None
    }
}

impl Drop for TerminalModes {
    /// Put back each action replaced. One some other code installed meanwhile is kept.
    fn drop(&mut self) {
        for (signal, previous) in KEYBOARD_SIGNALS.into_iter().zip(&self.previous) {
            let Some(previous) = previous else { continue };
            // SAFETY: an all-zero `sigaction` is valid storage, only written to.
            let mut replaced: libc::sigaction = unsafe { core::mem::zeroed() };
            // SAFETY: `previous` is an action `sigaction` reported, so valid to reinstall.
            let status = unsafe { libc::sigaction(signal, previous, &raw mut replaced) };
            if status == 0i32 && replaced.sa_sigaction != Self::handler() {
                // SAFETY: `replaced` is the action `sigaction` reported.
                unsafe {
                    libc::sigaction(signal, &raw const replaced, core::ptr::null_mut());
                }
            }
        }
        TERMINAL.store(-1i32, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::signal::tests::lock_handling;

    /// A pseudo-terminal's two ends: the one a program would read, and the other.
    struct Pty {
        controller: libc::c_int,
        terminal: libc::c_int,
    }

    impl Pty {
        fn echoes(&self) -> bool {
            // SAFETY: an all-zero `termios` is valid storage, only written to.
            let mut modes: libc::termios = unsafe { core::mem::zeroed() };
            // SAFETY: `modes` is live for the call.
            unsafe {
                libc::tcgetattr(self.terminal, &raw mut modes);
            }
            modes.c_lflag & libc::ECHO != 0
        }

        fn open() -> Self {
            let (mut controller, mut terminal) = (-1i32, -1i32);
            // SAFETY: both ends are written to; the name, modes and size are left to the system.
            let status = unsafe {
                libc::openpty(
                    &raw mut controller,
                    &raw mut terminal,
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                )
            };
            assert_eq!(status, 0i32, "openpty failed");
            Self { controller, terminal }
        }

        fn stop_echoing(&self) {
            // SAFETY: an all-zero `termios` is valid storage, only written to.
            let mut modes: libc::termios = unsafe { core::mem::zeroed() };
            // SAFETY: `modes` is live for the call.
            unsafe {
                libc::tcgetattr(self.terminal, &raw mut modes);
            }
            modes.c_lflag &= !libc::ECHO;
            // SAFETY: `modes` is the terminal's own, with echo turned off.
            unsafe {
                libc::tcsetattr(self.terminal, libc::TCSANOW, &raw const modes);
            }
        }
    }

    impl Drop for Pty {
        fn drop(&mut self) {
            // SAFETY: both ends were opened by `openpty` and are closed only here.
            unsafe {
                libc::close(self.terminal);
            }
            // SAFETY: as above.
            unsafe {
                libc::close(self.controller);
            }
        }
    }

    /// The handler `signal` has now.
    fn handler_of(signal: libc::c_int) -> libc::sighandler_t {
        // SAFETY: an all-zero `sigaction` is valid storage, only written to.
        let mut current: libc::sigaction = unsafe { core::mem::zeroed() };
        // SAFETY: a null new action only reads the current one into `current`.
        unsafe {
            libc::sigaction(signal, core::ptr::null(), &raw mut current);
        }
        current.sa_sigaction
    }

    #[test]
    fn echo_turned_off_under_the_guard_is_put_back() {
        let _handling = lock_handling();
        let pty = Pty::open();
        let modes = TerminalModes::of(pty.terminal);
        pty.stop_echoing();
        let stopped = pty.echoes();
        TerminalModes::put_back();
        let put_back = pty.echoes();
        drop(modes);
        assert_eq!((stopped, put_back), (false, true));
    }

    #[test]
    fn keyboard_signals_are_handled_until_dropped() {
        let _handling = lock_handling();
        let pty = Pty::open();
        let before = KEYBOARD_SIGNALS.map(handler_of);
        let modes = TerminalModes::of(pty.terminal);
        let during = KEYBOARD_SIGNALS.map(handler_of);
        drop(modes);
        let after = KEYBOARD_SIGNALS.map(handler_of);
        assert_eq!(
            (before, during, after, TERMINAL.load(Ordering::SeqCst)),
            ([libc::SIG_DFL; 2], [TerminalModes::handler(); 2], [libc::SIG_DFL; 2], -1i32)
        );
    }

    /// An app's own handling, here ignoring Ctrl-C, is left alone.
    #[test]
    fn an_ignored_interrupt_stays_ignored() {
        let _handling = lock_handling();
        let pty = Pty::open();
        // SAFETY: `SIG_IGN` is a valid disposition for SIGINT.
        let before = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
        let modes = TerminalModes::of(pty.terminal);
        let during = handler_of(libc::SIGINT);
        drop(modes);
        // SAFETY: `before` is the disposition `signal` reported.
        unsafe {
            libc::signal(libc::SIGINT, before);
        }
        assert_eq!(during, libc::SIG_IGN);
    }
}
