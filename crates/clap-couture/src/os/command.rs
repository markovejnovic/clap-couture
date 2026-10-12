//! Running child processes.

#[cfg(any(not(unix), test))]
use core::iter;
use std::{
    ffi::{OsStr, OsString},
    io,
    process::{Child, Command},
};

use super::signal::{ForwardedTermination, IgnoredSignals, Signal};

/// The signals a terminal sends every process in its foreground: Ctrl-C, Ctrl-\\, and the
/// terminal closing.
const TERMINAL_SIGNALS: [Signal; 3] = [Signal::Interrupt, Signal::Quit, Signal::Hangup];

/// How a child process ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Exit {
    /// It exited unsuccessfully, or died of a signal other than the terminal's.
    Failed,
    /// It died of one of the terminal's signals: the user pressed Ctrl-C in it, or closed the
    /// terminal.
    Killed(Signal),
    /// It exited successfully.
    Success,
}

/// [`Command`], run as an interactive child.
pub(crate) trait CommandExt {
    /// Spawn the command, leaving the terminal's signals to it: an editor uses Ctrl-C itself, or
    /// dies of it.
    ///
    /// Until the child exits, the parent process ignores the signals, so that they can't kill it
    /// and orphan the child. It passes `SIGTERM` on to the child; see [`ForwardedTermination`].
    fn spawn_interactive(&mut self) -> io::Result<InteractiveChild>;

    /// Spawn the command and wait for it to exit, as [`Command::status`] does, leaving the
    /// terminal's signals to it.
    #[cfg_attr(
        not(all(test, unix)),
        expect(dead_code, reason = "the editor spawns and waits apart")
    )]
    fn status_interactive(&mut self) -> io::Result<Exit> {
        self.spawn_interactive()?.wait()
    }
}

impl CommandExt for Command {
    fn spawn_interactive(&mut self) -> io::Result<InteractiveChild> {
        // An ignored signal stays ignored across `exec`, so the child resets each one the current
        // process doesn't ignore itself.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;

            let defaulted: Vec<libc::c_int> = TERMINAL_SIGNALS
                .iter()
                .filter(|signal| !signal.ignored_by_current_process())
                .map(|signal| signal.to_raw())
                .collect();
            let reset = move || {
                for signal in &defaulted {
                    // SAFETY: `signal` is async-signal-safe, so it may run between `fork` and
                    // `exec`.
                    unsafe {
                        libc::signal(*signal, libc::SIG_DFL);
                    }
                }
                Ok(())
            };
            // SAFETY: `reset` only calls `signal`, and allocates nothing.
            unsafe {
                self.pre_exec(reset);
            }
        }

        let ignored = IgnoredSignals::new(&TERMINAL_SIGNALS);
        let mut forwarded = ForwardedTermination::new();
        let child = self.spawn()?;
        forwarded.forward_to(child.id());
        Ok(InteractiveChild { _forwarded: forwarded, _ignored: ignored, child: Some(child) })
    }
}

/// A child started by [`CommandExt::spawn_interactive`]. Dropping it waits for the child.
#[must_use = "dropping it waits for the child to exit"]
pub(crate) struct InteractiveChild {
    /// Held until the child exits.
    _forwarded: ForwardedTermination,
    /// Held until the child exits.
    _ignored: IgnoredSignals,
    child: Option<Child>,
}

impl InteractiveChild {
    /// Wait for the child to exit.
    pub(crate) fn wait(mut self) -> io::Result<Exit> {
        let Some(mut child) = self.child.take() else { return Ok(Exit::Failed) };
        let status = child.wait()?;
        Ok(if status.success() {
            Exit::Success
        } else {
            Signal::killed(status).map_or(Exit::Failed, Exit::Killed)
        })
    }
}

impl Drop for InteractiveChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Nothing is left to report a failed wait to.
            drop(child.wait());
        }
    }
}

/// A quote in a command line that is never closed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct UnclosedQuote;

/// `command_line` split into words as a shell would, without expanding anything; on Windows,
/// on whitespace outside double quotes, keeping backslashes. Ends with [`UnclosedQuote`] for an
/// unclosed quote.
pub(crate) fn split(
    command_line: &OsStr,
) -> impl Iterator<Item = Result<OsString, UnclosedQuote>> + '_ {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};

        let mut words = shlex::bytes::Shlex::new(command_line.as_bytes());
        let mut ended = false;
        core::iter::from_fn(move || {
            if ended {
                return None;
            }
            let word = words.next();
            ended = word.is_none();
            match word {
                Some(word) => Some(Ok(OsString::from_vec(word))),
                None => words.had_error.then_some(Err(UnclosedQuote)),
            }
        })
    }

    #[cfg(not(unix))]
    {
        split_windows(command_line).map(Ok)
    }
}

/// `command_line` split on spaces and tabs outside double quotes, the quotes dropped.
#[cfg(any(not(unix), test))]
fn split_windows(command_line: &OsStr) -> impl Iterator<Item = OsString> + '_ {
    let delimiters = |byte: &u8| matches!(byte, b'"' | b' ' | b'\t');
    let mut pieces = command_line.as_encoded_bytes().split_inclusive(delimiters);
    let mut quoted = false;
    iter::from_fn(move || {
        let mut word: Option<OsString> = None;
        for piece in pieces.by_ref() {
            let (text, delimiter) = match piece.split_last() {
                Some((b'"', text)) => (text, Some(b'"')),
                Some((b' ' | b'\t', text)) if !quoted => (text, Some(b' ')),
                _ => (piece, None),
            };
            if !text.is_empty() {
                // SAFETY: split from `as_encoded_bytes` at ASCII bytes only.
                let text = unsafe { OsStr::from_encoded_bytes_unchecked(text) };
                word.get_or_insert_default().push(text);
            }
            match delimiter {
                Some(b'"') => {
                    quoted = !quoted;
                    word.get_or_insert_default();
                }
                Some(_) if word.is_some() => return word,
                Some(_) | None => {}
            }
        }
        word
    })
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::os::signal::tests::lock_handling;

    /// `script` run by `sh` as an interactive child.
    fn sh(script: &str) -> Option<Exit> {
        Command::new("sh").args(["-c", script]).status_interactive().ok()
    }

    #[rstest]
    #[case::success("exit 0", Exit::Success)]
    #[case::failure("exit 3", Exit::Failed)]
    #[case::exit_130_is_no_signal("exit 130", Exit::Failed)]
    #[case::other_signal("kill -TERM $$", Exit::Failed)]
    #[case::interrupted("kill -INT $$", Exit::Killed(Signal::Interrupt))]
    #[case::quit("kill -QUIT $$", Exit::Killed(Signal::Quit))]
    #[case::hung_up("kill -HUP $$", Exit::Killed(Signal::Hangup))]
    fn reads_how_the_child_ended(#[case] script: &str, #[case] expected: Exit) {
        let _handling = lock_handling();
        assert_eq!(sh(script), Some(expected));
    }

    /// The terminal's signals reach this process too, which ignores them while the child runs.
    /// Without that, this test process would die here.
    #[rstest]
    #[case::interrupt("INT")]
    #[case::hangup("HUP")]
    fn survives_the_terminals_signals(#[case] signal: &str) {
        let _handling = lock_handling();
        assert_eq!(sh(&format!("kill -{signal} $PPID")), Some(Exit::Success));
    }

    /// A signal the app ignored itself stays ignored in the child, as it would have anyway.
    #[test]
    fn a_signal_the_app_ignores_stays_ignored() {
        let _handling = lock_handling();
        // SAFETY: `SIG_IGN` is a valid disposition for SIGINT.
        let before = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
        let exit = sh("kill -INT $$");
        // SAFETY: `before` is the disposition `signal` reported.
        unsafe {
            libc::signal(libc::SIGINT, before);
        }
        assert_eq!(exit, Some(Exit::Success));
    }

    /// Another thread's interactive child has this process ignoring the signal, which isn't the
    /// app's doing: this child still gets the default handling.
    #[test]
    fn another_childs_ignoring_doesnt_reach_this_one() {
        let _handling = lock_handling();
        let other_child = IgnoredSignals::new(&[Signal::Interrupt]);
        let exit = sh("kill -INT $$");
        drop(other_child);
        assert_eq!(exit, Some(Exit::Killed(Signal::Interrupt)));
    }

    #[test]
    fn a_missing_program_is_an_error() {
        let _handling = lock_handling();
        let status = Command::new("clap-couture-no-such-program").status_interactive();
        assert_eq!(status.err().map(|err| err.kind()), Some(io::ErrorKind::NotFound));
    }

    #[rstest]
    #[case::bare("vim", &["vim"])]
    #[case::args("code --wait", &["code", "--wait"])]
    #[case::single_quotes("'/opt/my editor/bin/ed' -s", &["/opt/my editor/bin/ed", "-s"])]
    #[case::escaped_space(r"/opt/my\ editor/ed", &["/opt/my editor/ed"])]
    #[case::nothing_expanded("~/bin/ed $HOME", &["~/bin/ed", "$HOME"])]
    fn split_like_a_shell_without_expanding(#[case] command_line: &str, #[case] expected: &[&str]) {
        let expected: Vec<OsString> = expected.iter().map(OsString::from).collect();
        assert_eq!(split(OsStr::new(command_line)).collect::<Result<Vec<_>, _>>(), Ok(expected));
    }

    #[test]
    fn split_rejects_an_unclosed_quote() {
        let words: Vec<_> = split(OsStr::new("vim 'oops")).collect();
        assert_eq!(words, [Ok(OsString::from("vim")), Err(UnclosedQuote)]);
    }

    #[test]
    fn split_keeps_bytes_that_arent_unicode() {
        use std::os::unix::ffi::OsStrExt as _;

        let words: Result<Vec<_>, _> = split(OsStr::from_bytes(b"/opt/\xFF/ed -f")).collect();
        let expected = [OsStr::from_bytes(b"/opt/\xFF/ed"), OsStr::new("-f")].map(OsString::from);
        assert_eq!(words, Ok(expected.to_vec()));
    }

    #[rstest]
    #[case::bare("notepad", &["notepad"])]
    #[case::backslashes_kept(r"C:\Tools\vim.exe -f", &[r"C:\Tools\vim.exe", "-f"])]
    #[case::quoted_spaces(r#""C:\Program Files\Code\code.exe"  --wait"#, &[r"C:\Program Files\Code\code.exe", "--wait"])]
    #[case::empty_quotes(r#"ed """#, &["ed", ""])]
    #[case::tabs("ed\t-s", &["ed", "-s"])]
    #[case::quoted_tab("\"a\tb\"", &["a\tb"])]
    #[case::other_whitespace_kept("ed\u{a0}-s", &["ed\u{a0}-s"])]
    fn split_windows_honors_quotes_not_backslashes(
        #[case] command_line: &str,
        #[case] expected: &[&str],
    ) {
        let expected: Vec<OsString> = expected.iter().map(OsString::from).collect();
        assert_eq!(split_windows(OsStr::new(command_line)).collect::<Vec<_>>(), expected);
    }
}
