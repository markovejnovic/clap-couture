//! The terminal: what kind it is, and drawing on it the same way on every platform.

use std::{ffi::OsStr, io};

/// Check whether `term`, the value of `TERM`, says the terminal is dumb, as git decides it: `TERM`
/// is `dumb`, or unset outside Windows. Windows terminals don't set `TERM`; git for Windows sets
/// it itself before looking. An empty `TERM` counts as unset.
pub(crate) fn is_dumb(term: Option<&OsStr>, windows: bool) -> bool {
    term.filter(|term| !term.is_empty()).map_or(!windows, |term| term == "dumb")
}

/// Blank the `columns` cells before the cursor on its line, leaving the cursor at the line's
/// start. Spaces blank on every console, where `\x1b[K` needs VT processing on Windows.
///
/// # Errors
/// - When writing to `out` fails.
pub(crate) fn erase_line(out: &mut impl io::Write, columns: usize) -> io::Result<()> {
    write!(out, "\r{}\r", " ".repeat(columns))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::dumb(Some("dumb"), false, true)]
    #[case::xterm(Some("xterm-256color"), false, false)]
    #[case::unset(None, false, true)]
    #[case::empty_is_unset(Some(""), false, true)]
    #[case::unset_on_windows(None, true, false)]
    #[case::empty_on_windows(Some(""), true, false)]
    #[case::dumb_on_windows(Some("dumb"), true, true)]
    fn is_dumb_follows_git(#[case] term: Option<&str>, #[case] windows: bool, #[case] dumb: bool) {
        assert_eq!(is_dumb(term.map(OsStr::new), windows), dumb);
    }

    #[test]
    fn erase_line_blanks_and_returns() {
        let mut out = Vec::new();
        let written = erase_line(&mut out, 3).ok();
        assert_eq!((written, out.as_slice()), (Some(()), b"\r   \r".as_slice()));
    }
}
