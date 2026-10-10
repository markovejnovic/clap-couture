//! Answer a prompted arg in the user's editor, found and run the way git runs its own.
//!
//! The file holds the answer, then a header asking the question. Every header line starts with
//! [`HEADER`], an invisible separator before the `#`, so stripping the header leaves the user's
//! own `#` lines alone.

use std::{
    ffi::OsString,
    fs,
    io::{self, IsTerminal as _, Write as _},
    path::Path,
    process::{Command, ExitStatus},
};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{FALSE, STATUS_CONTROL_C_EXIT, TRUE},
    System::Console::SetConsoleCtrlHandler,
};

use super::{EditorPrompt, PromptError};

/// What every header line starts with: U+2063 INVISIBLE SEPARATOR, then `#`.
const HEADER: &str = "\u{2063}#";

/// The editor git would pick, from `$VISUAL` and `$EDITOR`.
#[derive(Debug)]
struct Editor {
    command: OsString,
    /// `TERM` is unset or `dumb`, so the waiting hint is not erased afterwards.
    dumb_terminal: bool,
}

impl Editor {
    /// `sh` expands the editor string as git's `run_command` does: quotes, `~`, env vars and
    /// args like `code --wait` all work.
    #[cfg(not(windows))]
    fn command(&self, path: &Path) -> Command {
        let mut script = self.command.clone();
        script.push(" \"$@\"");
        let mut command = Command::new("sh");
        command.arg("-c").arg(script).arg(&self.command).arg(path);
        command
    }

    /// Windows has no `sh` to rely on, so the editor string is split on spaces outside double
    /// quotes. Backslashes are kept: they separate a Windows path, they don't escape.
    #[cfg(windows)]
    fn command(&self, path: &Path) -> Command {
        let command_line = self.command.to_string_lossy();
        let mut words = split_windows(&command_line).into_iter();
        let mut command = Command::new(words.next().unwrap_or_default());
        command.args(words).arg(path);
        command
    }

    /// Look the editor up as git does: `$VISUAL` unless the terminal is dumb, then `$EDITOR`,
    /// then `vi`. An empty variable counts as unset.
    ///
    /// # Errors
    /// - [`PromptError::Io`] on a dumb terminal with no `$EDITOR`.
    fn from_env(var: impl Fn(&str) -> Option<OsString>) -> Result<Self, PromptError> {
        let set = |name: &str| var(name).filter(|value| !value.is_empty());
        let dumb_terminal = set("TERM").is_none_or(|term| term == "dumb");
        let visual = if dumb_terminal { None } else { set("VISUAL") };
        let command = match visual.or_else(|| set("EDITOR")) {
            Some(command) => command,
            None if dumb_terminal => {
                return Err(PromptError::Io(io::Error::other(
                    "Terminal is dumb, but EDITOR unset",
                )));
            }
            None => OsString::from(if cfg!(windows) { "notepad" } else { "vi" }),
        };
        Ok(Self { command, dumb_terminal })
    }

    /// Open `path` and wait for the editor to exit; `:` is git's no-op editor and is not run.
    ///
    /// # Errors
    /// - [`PromptError::Io`] when the editor cannot start or exits unsuccessfully.
    fn run(&self, path: &Path, waiting: &str) -> Result<(), PromptError> {
        if self.command == ":" {
            return Ok(());
        }
        let hinted = io::stderr().is_terminal();
        if hinted {
            // On a smart terminal the hint stays on its line, to be erased once the editor exits.
            let end = if self.dumb_terminal { '\n' } else { ' ' };
            write!(io::stderr(), "{waiting}{end}").map_err(PromptError::Io)?;
        }
        let status = self.wait(path);
        let succeeded = status.as_ref().is_ok_and(ExitStatus::success);
        if hinted && !self.dumb_terminal {
            // Erase the hint after a clean exit; otherwise keep it, ending its line.
            let tail = if succeeded { "\r\x1b[K" } else { "\n" };
            write!(io::stderr(), "{tail}").map_err(PromptError::Io)?;
        }
        match status {
            Ok(status) if status.success() => Ok(()),
            // The user pressed Ctrl-C in the editor: back out like any cancelled prompt.
            Ok(status) if interrupted(status) => Err(PromptError::Cancelled),
            Ok(_) | Err(_) => Err(PromptError::Io(io::Error::other(format!(
                "There was a problem with the editor '{}'.",
                self.command.to_string_lossy()
            )))),
        }
    }

    /// Run the editor on `path`, ignoring Ctrl-C until it exits, as git does: the editor gets
    /// Ctrl-C too, and decides what it means.
    fn wait(&self, path: &Path) -> io::Result<ExitStatus> {
        let mut child = self.command(path).spawn()?;
        // Ignored only once the editor is spawned, so it starts with the default Ctrl-C handling.
        let _interrupts_ignored = IgnoredInterrupts::new();
        child.wait()
    }
}

/// Ctrl-C ignored in this process while alive; the previous handling is restored on drop.
///
/// Ctrl-C reaches both us and the editor. Ignoring it here, like git does, leaves the editor to
/// decide: a terminal editor uses it, `code --wait` dies of it and the prompt is cancelled.
#[cfg(unix)]
struct IgnoredInterrupts {
    /// Each signal ignored, with the action it had before.
    previous: Vec<(libc::c_int, libc::sigaction)>,
}

#[cfg(unix)]
impl IgnoredInterrupts {
    /// Ignore `signal`, returning its previous action.
    fn ignore(signal: libc::c_int) -> Option<libc::sigaction> {
        // SAFETY: an all-zero `sigaction` is valid: `SIG_DFL`, no flags, an empty mask.
        let mut ignore: libc::sigaction = unsafe { core::mem::zeroed() };
        ignore.sa_sigaction = libc::SIG_IGN;
        // SAFETY: as above; it is only written to.
        let mut previous: libc::sigaction = unsafe { core::mem::zeroed() };
        // SAFETY: both pointers come from live references for the length of the call.
        let status = unsafe { libc::sigaction(signal, &raw const ignore, &raw mut previous) };
        (status == 0).then_some(previous)
    }

    /// Ignore `SIGINT` and `SIGQUIT`, as git does around its editor. Best effort: a signal that
    /// can't be ignored keeps its handling.
    fn new() -> Self {
        let previous = [libc::SIGINT, libc::SIGQUIT]
            .into_iter()
            .filter_map(|signal| Self::ignore(signal).map(|action| (signal, action)))
            .collect();
        Self { previous }
    }
}

#[cfg(unix)]
impl Drop for IgnoredInterrupts {
    fn drop(&mut self) {
        for (signal, previous) in &self.previous {
            // SAFETY: `previous` is the action `sigaction` reported, so it is valid to reinstall;
            // the old action is not asked for.
            unsafe {
                libc::sigaction(*signal, previous, core::ptr::null_mut());
            }
        }
    }
}

/// Ctrl-C ignored in this process while alive; see the Unix version. Spawned before, the editor
/// does not inherit the flag.
#[cfg(windows)]
struct IgnoredInterrupts {
    /// The flag was set, so dropping clears it.
    ignoring: bool,
}

#[cfg(windows)]
impl IgnoredInterrupts {
    fn new() -> Self {
        // SAFETY: with no handler, `TRUE` only sets this process's ignore-Ctrl+C flag.
        let ignoring = unsafe { SetConsoleCtrlHandler(None, TRUE) } != FALSE;
        Self { ignoring }
    }
}

#[cfg(windows)]
impl Drop for IgnoredInterrupts {
    fn drop(&mut self) {
        if self.ignoring {
            // SAFETY: with no handler, `FALSE` only clears the flag `new` set.
            unsafe {
                SetConsoleCtrlHandler(None, FALSE);
            }
        }
    }
}

/// Nothing to ignore where there are no console signals.
#[cfg(not(any(unix, windows)))]
struct IgnoredInterrupts;

#[cfg(not(any(unix, windows)))]
impl IgnoredInterrupts {
    const fn new() -> Self {
        Self
    }
}

/// Check whether the editor died of Ctrl-C (or Ctrl-\\): by the signal itself, or by `sh`
/// reporting it as `128 + signal`, the way git reads it.
#[cfg(unix)]
fn interrupted(status: ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt as _;

    const INTERRUPTS: [libc::c_int; 2] = [libc::SIGINT, libc::SIGQUIT];
    status.signal().is_some_and(|signal| INTERRUPTS.contains(&signal))
        || status.code().is_some_and(|code| {
            code.checked_sub(128).is_some_and(|signal| INTERRUPTS.contains(&signal))
        })
}

/// Check whether the editor died of Ctrl-C.
#[cfg(windows)]
fn interrupted(status: ExitStatus) -> bool {
    status.code() == Some(STATUS_CONTROL_C_EXIT)
}

#[cfg(not(any(unix, windows)))]
const fn interrupted(_status: ExitStatus) -> bool {
    false
}

/// Ask `prompt` in the user's editor, as [`Backend::editor`](super::Backend::editor) does by
/// default, returning `None` for an empty answer.
///
/// The editor is `$VISUAL`, then `$EDITOR`, then `vi` (`notepad` on Windows), run through `sh`
/// like git runs it; on Windows, split on spaces outside double quotes and run directly. The answer
/// is what the user saved, minus the header and any blank lines around it. Whether an empty answer
/// is allowed is the caller's call.
///
/// # Errors
/// - [`PromptError::Io`] when the temp file cannot be written or read, or the editor cannot start
///   or exits unsuccessfully.
pub fn edit(prompt: &EditorPrompt<'_>) -> Result<Option<String>, PromptError> {
    let editor = Editor::from_env(|name| std::env::var_os(name))?;
    let mut file = tempfile::Builder::new()
        .prefix(&format!("{}-", file_safe(prompt.file_stem)))
        .suffix(&format!(".{}", prompt.extension))
        .tempfile()?;
    file.write_all(render(prompt).as_bytes())?;
    // Closed but kept until dropped, so the editor may replace the file and we read the new one.
    let path = file.into_temp_path();
    editor.run(&path, prompt.waiting)?;
    Ok(strip(&fs::read_to_string(&path)?))
}

/// `stem` with anything but letters, digits, `-` and `_` replaced, to name a file with.
fn file_safe(stem: &str) -> String {
    stem.chars()
        .map(|char| if char.is_alphanumeric() || char == '-' || char == '_' { char } else { '-' })
        .collect()
}

/// `command_line` split on whitespace outside double quotes, the quotes dropped.
#[cfg_attr(
    not(any(windows, test)),
    expect(dead_code, reason = "elsewhere `sh` splits the editor string")
)]
fn split_windows(command_line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut quoted = false;
    for char in command_line.chars() {
        match char {
            '"' => {
                quoted = !quoted;
                word.get_or_insert_default();
            }
            _ if char.is_whitespace() && !quoted => words.extend(word.take()),
            _ => word.get_or_insert_default().push(char),
        }
    }
    words.extend(word);
    words
}

fn push_header_line(file: &mut String, text: &str) {
    file.push_str(HEADER);
    if !text.is_empty() {
        file.push(' ');
        file.push_str(text);
    }
    file.push('\n');
}

/// The file the editor opens: the answer so far, a blank line, then the header.
fn render(prompt: &EditorPrompt<'_>) -> String {
    let mut file = String::new();
    if let Some(default) = prompt.default {
        file.push_str(default);
        file.push('\n');
    }
    file.push('\n');
    for line in prompt.question.lines() {
        push_header_line(&mut file, line);
    }
    push_header_line(&mut file, "");
    if let Some(error) = prompt.error {
        push_header_line(&mut file, &format!("error: {error}"));
    }
    push_header_line(&mut file, prompt.instructions);
    file
}

/// The answer in `file`: the header lines and the blank lines around the rest dropped. `None`
/// when nothing is left.
fn strip(file: &str) -> Option<String> {
    let is_blank = |line: &&str| line.trim().is_empty();
    let mut kept: Vec<&str> =
        file.lines().filter(|line| !line.starts_with(HEADER)).skip_while(is_blank).collect();
    while kept.last().is_some_and(is_blank) {
        kept.pop();
    }
    (!kept.is_empty()).then(|| kept.join("\n"))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use clap::builder::Styles;
    use rstest::rstest;

    use super::*;
    use crate::interactive::CommandStyles;

    const STYLES: Styles = Styles::plain();

    /// Held by every test that runs the editor: signal handling is process-wide, so a test
    /// checking it must not overlap one that ignores Ctrl-C around an editor.
    #[cfg(unix)]
    static SIGNAL_HANDLING: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[cfg(unix)]
    fn lock_signal_handling() -> std::sync::MutexGuard<'static, ()> {
        SIGNAL_HANDLING.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[cfg(unix)]
    fn editor(command: &str) -> Editor {
        Editor { command: OsString::from(command), dumb_terminal: false }
    }

    fn prompt<'prompt>(
        default: Option<&'prompt str>,
        error: Option<&'prompt str>,
    ) -> EditorPrompt<'prompt> {
        EditorPrompt {
            default,
            error,
            extension: "md",
            file_stem: "deploy-notes",
            instructions: "Leave the file empty to cancel.",
            optional: false,
            question: "Release notes",
            styles: CommandStyles(&STYLES),
            waiting: "hint: Waiting for your editor to close the file...",
        }
    }

    #[rstest]
    #[case::nothing_set(&[("TERM", "xterm")], Some("vi"))]
    #[case::visual_first(&[("TERM", "xterm"), ("VISUAL", "code --wait"), ("EDITOR", "nano")], Some("code --wait"))]
    #[case::editor(&[("TERM", "xterm"), ("EDITOR", "nano")], Some("nano"))]
    #[case::empty_visual_is_unset(&[("TERM", "xterm"), ("VISUAL", ""), ("EDITOR", "nano")], Some("nano"))]
    #[case::dumb_skips_visual(&[("TERM", "dumb"), ("VISUAL", "code"), ("EDITOR", "ed")], Some("ed"))]
    #[case::no_term_skips_visual(&[("VISUAL", "code"), ("EDITOR", "ed")], Some("ed"))]
    #[case::dumb_without_editor(&[("TERM", "dumb"), ("VISUAL", "code")], None)]
    #[cfg_attr(windows, ignore = "the fallback is notepad")]
    fn lookup_follows_git(#[case] env: &[(&str, &str)], #[case] expected: Option<&str>) {
        let editor = Editor::from_env(|name| {
            env.iter().find(|(key, _)| *key == name).map(|(_, value)| OsString::from(value))
        });
        let command = editor.ok().map(|editor| editor.command);
        assert_eq!(command.as_deref(), expected.map(OsStr::new));
    }

    #[test]
    fn render_puts_the_answer_before_the_header() {
        let file = render(&prompt(Some("Fixed it."), None));
        assert_eq!(
            file,
            "Fixed it.\n\n\u{2063}# Release notes\n\u{2063}#\n\u{2063}# Leave the file empty to \
             cancel.\n"
        );
    }

    #[test]
    fn render_shows_the_rejected_answers_error() {
        let file = render(&prompt(Some("oops"), Some("invalid value 'oops'")));
        assert!(file.contains("\u{2063}# error: invalid value 'oops'\n"));
    }

    #[test]
    fn strip_round_trips_a_rendered_answer() {
        let answer = "# Heading\n\n  indented: yes  \n\nlast";
        let file = render(&prompt(Some(answer), None));
        assert_eq!(strip(&file).as_deref(), Some(answer));
    }

    #[rstest]
    #[case::only_the_header(&render(&prompt(None, None)), None)]
    #[case::whitespace(" \n\t\n", None)]
    #[case::blank_lines_around("\n\nhello\n\n", Some("hello"))]
    #[case::users_hash_lines_stay("# not a header\nbody\n", Some("# not a header\nbody"))]
    #[case::edited_header_line_goes("body\n\u{2063}# I changed this\n", Some("body"))]
    fn strip_drops_only_header_and_surrounding_blanks(
        #[case] file: &str,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(strip(file).as_deref(), expected);
    }

    #[rstest]
    #[case::bare("notepad", &["notepad"])]
    #[case::backslashes_kept(r"C:\Tools\vim.exe -f", &[r"C:\Tools\vim.exe", "-f"])]
    #[case::quoted_spaces(r#""C:\Program Files\Code\code.exe"  --wait"#, &[r"C:\Program Files\Code\code.exe", "--wait"])]
    #[case::empty_quotes(r#"ed """#, &["ed", ""])]
    fn split_windows_honors_quotes_not_backslashes(
        #[case] command_line: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(split_windows(command_line), expected);
    }

    #[test]
    fn file_safe_replaces_path_separators() {
        assert_eq!(file_safe("my tool/sub-notes_1"), "my-tool-sub-notes_1");
    }

    #[cfg(unix)]
    #[rstest]
    #[case::args_and_quotes("printf 'from \"%s\"' 'the editor' >")]
    #[case::env_var("EDITOR_TEST_WORD=editor; printf 'from \"the %s\"' \"$EDITOR_TEST_WORD\" >")]
    fn run_goes_through_sh(#[case] command: &str) {
        let _signal_handling = lock_signal_handling();
        let file = tempfile::NamedTempFile::new();
        let path = file.as_ref().map(tempfile::NamedTempFile::path).ok();
        let ran = path.map(|path| {
            editor(command).run(path, "").and_then(|()| Ok(fs::read_to_string(path)?)).ok()
        });
        assert_eq!(ran.flatten().as_deref(), Some("from \"the editor\""));
    }

    /// The action `signal` has now.
    #[cfg(unix)]
    fn handler_of(signal: libc::c_int) -> Option<libc::sighandler_t> {
        // SAFETY: an all-zero `sigaction` is valid storage, only written to.
        let mut current: libc::sigaction = unsafe { core::mem::zeroed() };
        // SAFETY: a null new action only reads the current one into `current`.
        let status = unsafe { libc::sigaction(signal, core::ptr::null(), &raw mut current) };
        (status == 0).then_some(current.sa_sigaction)
    }

    #[cfg(unix)]
    #[test]
    fn interrupts_are_ignored_until_dropped() {
        let _signal_handling = lock_signal_handling();
        let before = [libc::SIGINT, libc::SIGQUIT].map(handler_of);
        let ignored = IgnoredInterrupts::new();
        let during = [libc::SIGINT, libc::SIGQUIT].map(handler_of);
        drop(ignored);
        let after = [libc::SIGINT, libc::SIGQUIT].map(handler_of);
        assert_eq!(during, [Some(libc::SIG_IGN); 2]);
        assert_eq!(after, before);
    }

    #[cfg(unix)]
    #[rstest]
    #[case::killed_by_sigint(libc::SIGINT, true)]
    #[case::killed_by_sigquit(libc::SIGQUIT, true)]
    #[case::killed_by_sigterm(libc::SIGTERM, false)]
    #[case::sh_reports_sigint(130i32 << 8i32, true)]
    #[case::sh_reports_sigquit(131i32 << 8i32, true)]
    #[case::exits_1(1i32 << 8i32, false)]
    fn interrupted_reads_the_signal_or_shs_report(#[case] raw: i32, #[case] expected: bool) {
        use std::os::unix::process::ExitStatusExt as _;

        assert_eq!(interrupted(ExitStatus::from_raw(raw)), expected);
    }

    #[cfg(unix)]
    #[test]
    fn run_cancels_when_the_editor_dies_of_ctrl_c() {
        let _signal_handling = lock_signal_handling();
        let ran = editor("kill -INT $$; :").run(Path::new("/dev/null"), "");
        assert!(matches!(ran, Err(PromptError::Cancelled)), "{ran:?}");
    }

    #[cfg(unix)]
    #[test]
    fn run_reports_a_failing_editor() {
        let _signal_handling = lock_signal_handling();
        let ran = editor("false").run(Path::new("/dev/null"), "");
        let message = ran.err().map(|err| err.to_string());
        assert_eq!(message.as_deref(), Some("There was a problem with the editor 'false'."));
    }
}
