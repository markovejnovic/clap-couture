//! Utilities for handling process signals.

#[cfg(all(unix, feature = "interactive-editor"))]
use core::num::NonZeroUsize;
#[cfg(feature = "interactive-editor")]
use std::process::ExitStatus;
#[cfg(all(unix, feature = "interactive-editor"))]
use std::sync::{Mutex, MutexGuard, PoisonError};

#[cfg(windows)]
use windows_sys::Win32::Foundation::STATUS_CONTROL_C_EXIT;
#[cfg(all(windows, feature = "interactive-editor"))]
use windows_sys::{
    Win32::{
        Foundation::{FALSE, TRUE},
        System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, SetConsoleCtrlHandler},
    },
    core::BOOL,
};

/// How this process handles each [`Signal`], changed only with it locked, so that guards on
/// different threads never interleave. [`Signal::die`] takes it for good.
#[cfg(all(unix, feature = "interactive-editor"))]
static HANDLING: Mutex<Handling> =
    Mutex::new(Handling { hangup: None, interrupt: None, quit: None });

/// Each signal [`IgnoredSignals`] guards are ignoring.
#[cfg(all(unix, feature = "interactive-editor"))]
struct Handling {
    hangup: Option<Ignoring>,
    interrupt: Option<Ignoring>,
    quit: Option<Ignoring>,
}

#[cfg(all(unix, feature = "interactive-editor"))]
impl Handling {
    /// Ignore `signal` for one more guard: the first saves its action and ignores it. `false`
    /// when it can't be ignored.
    fn hold(&mut self, signal: Signal) -> bool {
        let slot = self.slot(signal);
        if let Some(ignoring) = slot {
            ignoring.holders = ignoring.holders.saturating_add(1);
            return true;
        }
        let Some(previous) = signal.ignore() else { return false };
        *slot = Some(Ignoring { holders: NonZeroUsize::MIN, previous });
        true
    }

    /// Lock the process's handling, even if a thread panicked holding it: the lock guards no
    /// invariant a panic can break halfway.
    fn lock() -> MutexGuard<'static, Self> {
        HANDLING.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Let go of `signal` for one guard: the last restores the action the first saved.
    fn release(&mut self, signal: Signal) {
        let slot = self.slot(signal);
        let Some(ignoring) = slot else { return };
        if let Some(holders) = NonZeroUsize::new(ignoring.holders.get().saturating_sub(1)) {
            ignoring.holders = holders;
        } else {
            let previous = ignoring.previous;
            *slot = None;
            signal.restore(&previous);
        }
    }

    /// Whether guards are ignoring `signal`, and how many.
    const fn slot(&mut self, signal: Signal) -> &mut Option<Ignoring> {
        match signal {
            Signal::Hangup => &mut self.hangup,
            Signal::Interrupt => &mut self.interrupt,
            Signal::Quit => &mut self.quit,
        }
    }
}

/// A signal some [`IgnoredSignals`] guards are ignoring.
#[cfg(all(unix, feature = "interactive-editor"))]
struct Ignoring {
    /// The guards ignoring it; at least one.
    holders: NonZeroUsize,
    /// The action it had before the first of them.
    previous: libc::sigaction,
}

/// A known signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Signal {
    /// `SIGHUP`: the terminal closed.
    Hangup,
    /// `SIGINT`: Ctrl-C.
    Interrupt,
    /// `SIGQUIT`: Ctrl-\\, or Ctrl+Break on Windows.
    Quit,
}

impl Signal {
    /// The action this signal has now.
    #[cfg(all(unix, feature = "interactive-editor"))]
    fn action(self) -> Option<libc::sigaction> {
        // SAFETY: an all-zero `sigaction` is valid storage, only written to.
        let mut current: libc::sigaction = unsafe { core::mem::zeroed() };
        // SAFETY: a null new action only reads the current one into `current`.
        let status = unsafe { libc::sigaction(self.to_raw(), core::ptr::null(), &raw mut current) };

        (status == 0i32).then_some(current)
    }

    /// Die of this signal.
    ///
    /// Normally, signals which interrupt processes can abort the process. This allows you to exit
    /// mimicking as if that signal was fired to the app.
    pub(crate) fn die(self) -> ! {
        #[cfg(unix)]
        {
            use std::io::Write as _;

            // No guard may ignore the signal between setting its default action and raising it.
            #[cfg(feature = "interactive-editor")]
            let _handling = Handling::lock();
            let signal = self.to_raw();
            drop(std::io::stdout().flush());
            drop(std::io::stderr().flush());

            // SAFETY: an all-zero `sigset_t` is valid storage, filled in by `sigemptyset`.
            let mut set: libc::sigset_t = unsafe { core::mem::zeroed() };
            // A blocked signal would stay pending instead of killing, so it is unblocked first.
            // SAFETY: `set` is live for the call.
            unsafe {
                libc::sigemptyset(&raw mut set);
            }
            // SAFETY: as above; `signal` is a real signal.
            unsafe {
                libc::sigaddset(&raw mut set, signal);
            }
            // SAFETY: `set` is live, and the old mask is not asked for.
            unsafe {
                libc::pthread_sigmask(libc::SIG_UNBLOCK, &raw const set, core::ptr::null_mut());
            }
            // SAFETY: the default action is valid for every signal.
            unsafe {
                libc::signal(signal, libc::SIG_DFL);
            }
            // SAFETY: raising a real signal; with its default action, it ends the process.
            unsafe {
                libc::raise(signal);
            }
        }

        std::process::exit(self.exit_code())
    }

    /// The status that reports a death by this signal.
    const fn exit_code(self) -> i32 {
        #[cfg(windows)]
        match self {
            Self::Hangup | Self::Interrupt | Self::Quit => STATUS_CONTROL_C_EXIT,
        }
        // 128 plus the signal is how a shell reports a death by it: 130 for SIGINT.
        #[cfg(unix)]
        {
            128i32.saturating_add(self.to_raw())
        }
        // Without signals or a shell convention for them, a plain failure.
        #[cfg(not(any(unix, windows)))]
        match self {
            Self::Hangup | Self::Interrupt | Self::Quit => 1i32,
        }
    }

    /// The signal `raw` is, if it is one of these.
    #[cfg(unix)]
    #[cfg_attr(
        not(any(test, feature = "interactive-editor")),
        expect(dead_code, reason = "only a child's death names a signal by number")
    )]
    pub(crate) const fn from_raw(raw: libc::c_int) -> Option<Self> {
        match raw {
            libc::SIGHUP => Some(Self::Hangup),
            libc::SIGINT => Some(Self::Interrupt),
            libc::SIGQUIT => Some(Self::Quit),
            _ => None,
        }
    }

    /// Ignore this signal, returning its previous action.
    #[cfg(all(unix, feature = "interactive-editor"))]
    fn ignore(self) -> Option<libc::sigaction> {
        // SAFETY: an all-zero `sigaction` is valid: `SIG_DFL`, no flags, an empty mask.
        let mut ignore: libc::sigaction = unsafe { core::mem::zeroed() };
        ignore.sa_sigaction = libc::SIG_IGN;
        // SAFETY: as above; it is only written to.
        let mut previous: libc::sigaction = unsafe { core::mem::zeroed() };
        // SAFETY: both pointers come from live references for the length of the call.
        let status =
            unsafe { libc::sigaction(self.to_raw(), &raw const ignore, &raw mut previous) };

        (status == 0i32).then_some(previous)
    }

    /// Check whether the current process ignores this signal, looking past the [`IgnoredSignals`]
    /// guards: while one holds it, what counts is the action it had before the first of them.
    #[cfg(all(unix, feature = "interactive-editor"))]
    pub(crate) fn ignored_by_current_process(self) -> bool {
        let before_guards = Handling::lock().slot(self).as_ref().map(|ignoring| ignoring.previous);
        before_guards
            .or_else(|| self.action())
            .is_some_and(|action| action.sa_sigaction == libc::SIG_IGN)
    }

    /// The signal `status`'s process died of, if it is one of these. On Windows, a process ended
    /// by Ctrl+C or Ctrl+Break exits with `STATUS_CONTROL_C_EXIT`, read as Ctrl-C.
    #[cfg(feature = "interactive-editor")]
    pub(crate) fn killed(status: ExitStatus) -> Option<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt as _;

            status.signal().and_then(Self::from_raw)
        }
        #[cfg(windows)]
        {
            (status.code() == Some(STATUS_CONTROL_C_EXIT)).then_some(Self::Interrupt)
        }
        #[cfg(not(any(unix, windows)))]
        {
            drop(status);
            None
        }
    }

    /// Put `previous` back, unless the signal is no longer ignored: a handler some other code
    /// installed meanwhile is put back in turn. Swapping in one call, rather than checking first,
    /// leaves the other code's handler replaced only for a moment, not for good.
    #[cfg(all(unix, feature = "interactive-editor"))]
    fn restore(self, previous: &libc::sigaction) {
        // SAFETY: an all-zero `sigaction` is valid storage, only written to.
        let mut replaced: libc::sigaction = unsafe { core::mem::zeroed() };
        // SAFETY: `previous` is an action `sigaction` reported, so valid to reinstall.
        let status = unsafe { libc::sigaction(self.to_raw(), previous, &raw mut replaced) };
        if status == 0i32 && replaced.sa_sigaction != libc::SIG_IGN {
            // SAFETY: `replaced` is the action `sigaction` reported.
            unsafe {
                libc::sigaction(self.to_raw(), &raw const replaced, core::ptr::null_mut());
            }
        }
    }

    /// The signal's number.
    #[cfg(unix)]
    pub(crate) const fn to_raw(self) -> libc::c_int {
        match self {
            Self::Hangup => libc::SIGHUP,
            Self::Interrupt => libc::SIGINT,
            Self::Quit => libc::SIGQUIT,
        }
    }
}

/// Signals ignored in this process while alive; the previous handling is restored on drop.
#[cfg(all(unix, feature = "interactive-editor"))]
pub(crate) struct IgnoredSignals {
    /// The signals this guard holds in [`HANDLING`].
    held: Vec<Signal>,
}

#[cfg(all(unix, feature = "interactive-editor"))]
impl IgnoredSignals {
    /// Ignore `signals`. Best effort: a signal that can't be ignored keeps its handling.
    pub(crate) fn new(signals: &[Signal]) -> Self {
        let mut handling = Handling::lock();
        let held = signals.iter().copied().filter(|signal| handling.hold(*signal)).collect();
        Self { held }
    }
}

#[cfg(all(unix, feature = "interactive-editor"))]
impl Drop for IgnoredSignals {
    /// Restore each signal still ignored. One whose handling changed meanwhile, say by another
    /// thread installing a handler, keeps the new handling.
    fn drop(&mut self) {
        let mut handling = Handling::lock();
        for signal in &self.held {
            handling.release(*signal);
        }
    }
}

/// Ctrl+C and Ctrl+Break ignored in this process while alive; see the Unix version.
///
/// A handler of our own, removed on drop, rather than the process-wide ignore flag: clearing the
/// flag would also clear it for an app that had set it. Handlers aren't inherited, so a child
/// keeps the default handling. A console handler sees both keys or neither, so asking for
/// [`Signal::Interrupt`] or [`Signal::Quit`] ignores both; there is no hang-up to ignore.
#[cfg(all(windows, feature = "interactive-editor"))]
pub(crate) struct IgnoredSignals {
    /// The handler was added, so dropping removes it.
    registered: bool,
}

#[cfg(all(windows, feature = "interactive-editor"))]
impl IgnoredSignals {
    /// Swallow Ctrl+C and Ctrl+Break; leave closing, logoff and shutdown to the next handler.
    const extern "system" fn ignore(ctrl_type: u32) -> BOOL {
        if matches!(ctrl_type, CTRL_C_EVENT | CTRL_BREAK_EVENT) { TRUE } else { FALSE }
    }

    pub(crate) fn new(signals: &[Signal]) -> Self {
        let wanted =
            signals.iter().any(|signal| matches!(signal, Signal::Interrupt | Signal::Quit));
        // SAFETY: `ignore` is a valid handler for the whole program, as a plain function.
        let registered =
            wanted && unsafe { SetConsoleCtrlHandler(Some(Self::ignore), TRUE) } != FALSE;
        Self { registered }
    }
}

#[cfg(all(windows, feature = "interactive-editor"))]
impl Drop for IgnoredSignals {
    fn drop(&mut self) {
        if self.registered {
            // SAFETY: removes only the handler `new` added.
            unsafe {
                SetConsoleCtrlHandler(Some(Self::ignore), FALSE);
            }
        }
    }
}

/// Nothing to ignore where there are no console signals.
#[cfg(all(not(any(unix, windows)), feature = "interactive-editor"))]
pub(crate) struct IgnoredSignals;

#[cfg(all(not(any(unix, windows)), feature = "interactive-editor"))]
impl IgnoredSignals {
    pub(crate) const fn new(_signals: &[Signal]) -> Self {
        Self
    }
}

#[cfg(test)]
#[cfg(unix)]
pub(crate) mod tests {
    #[cfg(feature = "interactive-editor")]
    use std::{os::unix::process::ExitStatusExt as _, process::ExitStatus};

    use rstest::rstest;

    use super::*;

    #[cfg(feature = "interactive-editor")]
    const ALL: [Signal; 3] = [Signal::Interrupt, Signal::Quit, Signal::Hangup];

    /// The handler `signal` has now.
    #[cfg(feature = "interactive-editor")]
    fn handler_of(signal: Signal) -> Option<libc::sighandler_t> {
        signal.action().map(|current| current.sa_sigaction)
    }

    /// Held by every test that changes or reads this process's signal handling, which is
    /// process-wide, so that no two overlap.
    #[cfg(feature = "interactive-editor")]
    pub(crate) fn lock_handling() -> std::sync::MutexGuard<'static, ()> {
        static HANDLING: std::sync::Mutex<()> = std::sync::Mutex::new(());
        HANDLING.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[rstest]
    #[case::hangup(Signal::Hangup)]
    #[case::interrupt(Signal::Interrupt)]
    #[case::quit(Signal::Quit)]
    fn raw_round_trips(#[case] signal: Signal) {
        assert_eq!(Signal::from_raw(signal.to_raw()), Some(signal));
    }

    #[test]
    fn from_raw_knows_only_its_own() {
        assert_eq!(Signal::from_raw(libc::SIGTERM), None);
    }

    #[rstest]
    #[case::hangup(Signal::Hangup, 128i32 + libc::SIGHUP)]
    #[case::interrupt(Signal::Interrupt, 128i32 + libc::SIGINT)]
    #[case::quit(Signal::Quit, 128i32 + libc::SIGQUIT)]
    fn exit_code_is_the_shells(#[case] signal: Signal, #[case] code: i32) {
        assert_eq!(signal.exit_code(), code);
    }

    #[cfg(feature = "interactive-editor")]
    #[test]
    fn signals_are_ignored_until_dropped() {
        let _handling = lock_handling();
        let before = ALL.map(handler_of);
        let ignored = IgnoredSignals::new(&ALL);
        let during = ALL.map(handler_of);
        drop(ignored);
        let after = ALL.map(handler_of);
        assert_eq!(during, [Some(libc::SIG_IGN); 3]);
        assert_eq!(after, before);
    }

    #[cfg(feature = "interactive-editor")]
    #[test]
    fn nested_guards_restore_when_the_last_drops() {
        let _handling = lock_handling();
        let before = ALL.map(handler_of);
        let outer = IgnoredSignals::new(&ALL);
        let inner = IgnoredSignals::new(&[Signal::Interrupt]);
        drop(outer);
        let after_outer = handler_of(Signal::Interrupt);
        drop(inner);
        let after = ALL.map(handler_of);
        assert_eq!((after_outer, after), (Some(libc::SIG_IGN), before));
    }

    #[cfg(feature = "interactive-editor")]
    #[test]
    fn a_handler_installed_meanwhile_is_kept() {
        extern "C" fn installed_meanwhile(_signal: libc::c_int) {}

        #[expect(clippy::as_conversions, reason = "`sigaction` takes a handler as an address")]
        let handler = (installed_meanwhile as *const ()).addr();
        let _handling = lock_handling();
        // Reading the current action can't fail for a valid signal.
        let Some(before) = Signal::Quit.action() else { return };
        let ignored = IgnoredSignals::new(&ALL);
        let mut meanwhile = before;
        meanwhile.sa_sigaction = handler;
        // SAFETY: `meanwhile` is a valid action with a plain function as its handler.
        unsafe {
            libc::sigaction(libc::SIGQUIT, &raw const meanwhile, core::ptr::null_mut());
        }
        drop(ignored);
        let after = handler_of(Signal::Quit);
        // SAFETY: `before` is the action `sigaction` reported.
        unsafe {
            libc::sigaction(libc::SIGQUIT, &raw const before, core::ptr::null_mut());
        }
        assert_eq!(after, Some(handler));
    }

    #[cfg(feature = "interactive-editor")]
    #[test]
    fn ignored_by_current_process_looks_past_guards() {
        let _handling = lock_handling();
        let by_default = Signal::Interrupt.ignored_by_current_process();
        let guarded = IgnoredSignals::new(&[Signal::Interrupt]);
        let under_guard = Signal::Interrupt.ignored_by_current_process();
        drop(guarded);
        // SAFETY: `SIG_IGN` is a valid disposition for SIGINT.
        let before = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
        let app_ignores = Signal::Interrupt.ignored_by_current_process();
        let guarded = IgnoredSignals::new(&[Signal::Interrupt]);
        let app_ignores_under_guard = Signal::Interrupt.ignored_by_current_process();
        drop(guarded);
        // SAFETY: `before` is the disposition `signal` reported.
        unsafe {
            libc::signal(libc::SIGINT, before);
        }
        assert_eq!(
            (by_default, under_guard, app_ignores, app_ignores_under_guard),
            (false, false, true, true)
        );
    }

    #[cfg(feature = "interactive-editor")]
    #[rstest]
    #[case::killed_by_sigint(libc::SIGINT, Some(Signal::Interrupt))]
    #[case::killed_by_sigquit(libc::SIGQUIT, Some(Signal::Quit))]
    #[case::killed_by_sighup(libc::SIGHUP, Some(Signal::Hangup))]
    #[case::killed_by_sigterm(libc::SIGTERM, None)]
    #[case::exits_130(130i32 << 8i32, None)]
    #[case::exits_1(1i32 << 8i32, None)]
    fn killed_names_the_signal(#[case] raw: i32, #[case] expected: Option<Signal>) {
        assert_eq!(Signal::killed(ExitStatus::from_raw(raw)), expected);
    }
}
