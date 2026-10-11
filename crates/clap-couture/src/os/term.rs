//! The terminal: what kind it is, and drawing on it the same way on every platform.

use std::{ffi::OsString, io};

use anstream::adapter::strip_str;
use unicode_width::UnicodeWidthStr as _;

/// Representation of the `$TERM` variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Term {
    /// `TERM=dumb`: the terminal can't move the cursor or erase.
    Dumb,
    /// Any other terminal, by name, like `xterm-256color`.
    Named(OsString),
}

impl Term {
    /// Check whether the terminal is dumb.
    pub(crate) const fn is_dumb(&self) -> bool {
        matches!(self, Self::Dumb)
    }

    /// `value` as `TERM` holds it; `None` when it is empty, which counts as unset.
    fn parse(value: OsString) -> Option<Self> {
        match value {
            name if name.is_empty() => None,
            name if name == "dumb" => Some(Self::Dumb),
            name => Some(Self::Named(name)),
        }
    }

    /// Read the `$TERM` variable; `None` when it is unset or empty.
    pub(crate) fn read() -> Option<Self> {
        std::env::var_os("TERM").and_then(Self::parse)
    }
}

/// Blank the `columns` cells before the cursor on its line, leaving the cursor at the line's
/// start.
pub(crate) fn erase_line(out: &mut impl io::Write, columns: usize, vt: bool) -> io::Result<()> {
    if vt {
        write!(out, "\r\x1b[K")
    } else {
        // `\x1b[K` needs VT processing, which a Windows console may not have on.
        write!(out, "\r{}\r", " ".repeat(columns))
    }
}

/// The columns `text` takes on the terminal.
pub(crate) fn visible_width(text: &str) -> usize {
    strip_str(text).to_string().width()
}

/// Check whether `stream`, a terminal, takes VT escape sequences.
#[cfg(windows)]
pub(crate) fn has_vt(stream: &impl std::os::windows::io::AsHandle) -> bool {
    use std::os::windows::io::AsRawHandle as _;

    use windows_sys::Win32::System::Console::{ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode};

    let mut mode = 0;
    // SAFETY: the handle is borrowed from `stream` for the call; `mode` is only written to.
    let is_console =
        unsafe { GetConsoleMode(stream.as_handle().as_raw_handle(), &raw mut mode) } != 0i32;

    !is_console || mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING != 0
}

/// Outside Windows, every terminal takes VT escape sequences.
#[cfg(not(windows))]
pub(crate) const fn has_vt<S>(_stream: &S) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::dumb("dumb", Some(Term::Dumb))]
    #[case::named("xterm-256color", Some(Term::Named(OsString::from("xterm-256color"))))]
    #[case::empty_is_unset("", None)]
    fn parse_reads_the_value(#[case] value: &str, #[case] expected: Option<Term>) {
        assert_eq!(Term::parse(OsString::from(value)), expected);
    }

    #[rstest]
    #[case::dumb(Term::Dumb, true)]
    #[case::named(Term::Named(OsString::from("xterm")), false)]
    fn is_dumb_only_for_dumb(#[case] term: Term, #[case] dumb: bool) {
        assert_eq!(term.is_dumb(), dumb);
    }

    #[rstest]
    #[case::plain("hint: wait", 10)]
    #[case::colored("\x1b[90m\u{2502}\x1b[0m  hint", 7)]
    #[case::wide("\u{4f60}\u{597d}", 4)]
    fn visible_width_skips_escape_sequences(#[case] text: &str, #[case] width: usize) {
        assert_eq!(visible_width(text), width);
    }

    #[rstest]
    #[case::vt(true, b"\r\x1b[K")]
    #[case::no_vt(false, b"\r   \r")]
    fn erase_line_blanks_and_returns(#[case] vt: bool, #[case] expected: &[u8]) {
        let mut out = Vec::new();
        let written = erase_line(&mut out, 3, vt).ok();
        assert_eq!((written, out.as_slice()), (Some(()), expected));
    }
}
