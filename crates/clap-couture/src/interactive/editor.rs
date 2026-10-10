//! Answer a prompted arg in the user's editor, found the way git finds its own.
//!
//! The file holds the answer, then a header asking the question. Every header line starts with
//! [`HEADER`], `#|`, so stripping the header leaves the user's own `#` lines alone. It is plain
//! ASCII, so it survives an editor that saves in another encoding.
//!
//! The editor is spawned directly, not through `sh`: its exit status is its own, so Ctrl-C and a
//! missing program read plainly, and nothing in `$EDITOR` gets expanded behind the user's back.

use core::time::Duration;
use std::{
    ffi::{OsStr, OsString},
    fs,
    io::{self, IsTerminal as _, Write as _},
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    time::Instant,
};

use unicode_width::UnicodeWidthStr as _;
#[cfg(windows)]
use windows_sys::{
    Win32::{
        Foundation::{FALSE, STATUS_CONTROL_C_EXIT, TRUE},
        System::Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, SetConsoleCtrlHandler},
    },
    core::BOOL,
};

use super::{EditorPrompt, PromptError};

/// What every header line starts with. Markdown headings take `# ` and shell comments `# `, so
/// a user's own line almost never starts with `#|`.
const HEADER: &str = "#|";

/// An editor that exits sooner than this and leaves the file as it was most likely handed the
/// file to a running instance, like `code` without `--wait`, rather than being done with it.
const RETURNED_EARLY: Duration = Duration::from_secs(2);

/// Editors known to hand the file off and return at once, with the flag that makes them wait.
const WAIT_FLAGS: &[(&str, &str)] = &[
    ("atom", "--wait"),
    ("code", "--wait"),
    ("code-insiders", "--wait"),
    ("codium", "--wait"),
    ("cursor", "--wait"),
    ("gedit", "--wait"),
    ("gvim", "-f"),
    ("kate", "--block"),
    ("mate", "-w"),
    ("mvim", "-f"),
    ("open", "-W"),
    ("subl", "-w"),
    ("windsurf", "--wait"),
    ("zed", "--wait"),
];

/// The editor git would pick, from `$VISUAL` and `$EDITOR`.
#[derive(Debug)]
struct Editor {
    command: OsString,
    /// `TERM` says the terminal is dumb, so the waiting hint is not erased afterwards.
    dumb_terminal: bool,
    /// Where `command` came from, `VISUAL` or `EDITOR`; `None` for the `vi` or `notepad` fallback.
    variable: Option<&'static str>,
}

impl Editor {
    /// The editor's process for `path`: its words, then the file.
    ///
    /// # Errors
    /// - [`PromptError::Io`] when the command can't be split into words.
    fn command(&self, path: &Path) -> Result<Command, PromptError> {
        let mut words = self.words()?.into_iter();
        let program = words.next().unwrap_or_default();
        // Rust's own `PATH` search only tries `.exe`, and `code` is `code.cmd`.
        #[cfg(windows)]
        let program = resolve_windows(
            &program.to_string_lossy(),
            std::env::var_os("PATH").as_deref(),
            std::env::var_os("PATHEXT").as_deref(),
            Path::is_file,
        );
        let mut command = Command::new(program);
        command.args(words).arg(path);
        Ok(command)
    }

    /// Look the editor up as git does: `$VISUAL` unless the terminal is dumb, then `$EDITOR`,
    /// then `vi` (`notepad` on Windows). An empty variable counts as unset.
    ///
    /// The terminal is dumb when `TERM` is `dumb`, or unset outside Windows. Windows terminals
    /// don't set `TERM`; git for Windows sets it itself before looking.
    ///
    /// # Errors
    /// - [`PromptError::Io`] on a dumb terminal with no `$EDITOR`.
    fn from_env(
        var: impl Fn(&str) -> Option<OsString>,
        windows: bool,
    ) -> Result<Self, PromptError> {
        let set = |name: &str| var(name).filter(|value| !value.is_empty());
        let dumb_terminal = set("TERM").map_or(!windows, |term| term == "dumb");
        let visual = if dumb_terminal { None } else { set("VISUAL") };
        let (command, variable) = match visual
            .map(|command| (command, "VISUAL"))
            .or_else(|| set("EDITOR").map(|command| (command, "EDITOR")))
        {
            Some((command, variable)) => (command, Some(variable)),
            None if dumb_terminal => {
                return Err(PromptError::Io(io::Error::other(
                    "Terminal is dumb, but EDITOR unset",
                )));
            }
            None => (OsString::from(if windows { "notepad" } else { "vi" }), None),
        };
        Ok(Self { command, dumb_terminal, variable })
    }

    /// Open `path` and wait for the editor to exit, returning whether it ran: `:` is git's no-op
    /// editor and is not run.
    ///
    /// # Errors
    /// - [`PromptError::Cancelled`] when the editor dies of Ctrl-C.
    /// - [`PromptError::Io`] when the editor cannot be found or started, or exits unsuccessfully.
    fn run(&self, path: &Path, waiting: &str) -> Result<bool, PromptError> {
        if self.command == ":" {
            return Ok(false);
        }
        let mut command = self.command(path)?;
        let hinted = io::stderr().is_terminal();
        if hinted {
            // On a smart terminal the hint stays on its line, to be erased once the editor exits.
            let end = if self.dumb_terminal { '\n' } else { ' ' };
            write!(io::stderr(), "{waiting}{end}").map_err(PromptError::Io)?;
        }
        let status = wait(&mut command);
        let succeeded = status.as_ref().is_ok_and(ExitStatus::success);
        if hinted && !self.dumb_terminal {
            // Erase the hint after a clean exit; otherwise keep it, ending its line. Spaces erase
            // it on every console, where `\x1b[K` needs VT processing on Windows.
            let tail = if succeeded {
                format!("\r{}\r", " ".repeat(waiting.width().saturating_add(1)))
            } else {
                "\n".to_owned()
            };
            write!(io::stderr(), "{tail}").map_err(PromptError::Io)?;
        }
        let editor = self.command.to_string_lossy();
        match status {
            Ok(status) if status.success() => Ok(true),
            // The user pressed Ctrl-C in the editor: back out like any cancelled prompt.
            Ok(status) if interrupted(status) => Err(PromptError::Cancelled),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Err(PromptError::Io(
                io::Error::new(err.kind(), format!("Could not find the editor '{editor}'.")),
            )),
            Ok(_) | Err(_) => Err(PromptError::Io(io::Error::other(format!(
                "There was a problem with the editor '{editor}'."
            )))),
        }
    }

    /// The setting that would make an editor known to return at once wait, like
    /// `EDITOR="code --wait"`; `None` for any other editor, or one already given its flag.
    fn wait_fix(&self) -> Option<String> {
        let words = self.words().ok()?;
        let program = Path::new(words.first()?).file_stem()?.to_string_lossy().to_lowercase();
        let (_, flag) = WAIT_FLAGS.iter().find(|(name, _)| *name == program)?;
        let given = words.iter().skip(1).any(|word| word == *flag || word == "--wait");
        (!given).then(|| {
            let variable = self.variable.unwrap_or("EDITOR");
            format!("{variable}=\"{} {flag}\"", self.command.to_string_lossy())
        })
    }

    /// The editor string split into words the way a shell splits them, without expanding
    /// anything. A string that isn't Unicode is a single word.
    ///
    /// # Errors
    /// - [`PromptError::Io`] on unbalanced quotes.
    #[cfg(not(windows))]
    fn words(&self) -> Result<Vec<OsString>, PromptError> {
        let Some(command) = self.command.to_str() else { return Ok(vec![self.command.clone()]) };
        let words = shell_words::split(command).map_err(|_unclosed_quote| {
            PromptError::Io(io::Error::other(format!(
                "Could not split the editor '{command}' into words: a quote is not closed."
            )))
        })?;
        Ok(words.into_iter().map(OsString::from).collect())
    }

    /// The editor string split on spaces outside double quotes. Backslashes are kept: they
    /// separate a Windows path, they don't escape.
    #[cfg(windows)]
    #[expect(
        clippy::unnecessary_wraps,
        reason = "the same signature as the Unix version, whose quotes can be unbalanced"
    )]
    fn words(&self) -> Result<Vec<OsString>, PromptError> {
        let words = split_windows(&self.command.to_string_lossy());
        Ok(words.into_iter().map(OsString::from).collect())
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

/// Ctrl+C and Ctrl+Break ignored in this process while alive; see the Unix version.
///
/// A handler of our own, removed on drop, rather than the process-wide ignore flag: clearing the
/// flag would also clear it for an app that had set it. Handlers aren't inherited, so the editor
/// keeps the default handling.
#[cfg(windows)]
struct IgnoredInterrupts {
    /// The handler was added, so dropping removes it.
    registered: bool,
}

#[cfg(windows)]
impl IgnoredInterrupts {
    /// Swallow Ctrl+C and Ctrl+Break; leave closing, logoff and shutdown to the next handler.
    const extern "system" fn ignore(ctrl_type: u32) -> BOOL {
        if matches!(ctrl_type, CTRL_C_EVENT | CTRL_BREAK_EVENT) { TRUE } else { FALSE }
    }

    fn new() -> Self {
        // SAFETY: `ignore` is a valid handler for the whole program, as a plain function.
        let registered = unsafe { SetConsoleCtrlHandler(Some(Self::ignore), TRUE) } != FALSE;
        Self { registered }
    }
}

#[cfg(windows)]
impl Drop for IgnoredInterrupts {
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
#[cfg(not(any(unix, windows)))]
struct IgnoredInterrupts;

#[cfg(not(any(unix, windows)))]
impl IgnoredInterrupts {
    const fn new() -> Self {
        Self
    }
}

/// Check whether the editor died of Ctrl-C (or Ctrl-\\). It is our own child, so its status
/// carries the signal itself.
#[cfg(unix)]
fn interrupted(status: ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt as _;

    status.signal().is_some_and(|signal| [libc::SIGINT, libc::SIGQUIT].contains(&signal))
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
/// The editor is `$VISUAL`, then `$EDITOR`, then `vi` (`notepad` on Windows). Its string is split
/// into words like a shell would, without expanding `~` or variables, and the program is spawned
/// directly with the file as its last argument. On Windows the split honors double quotes only,
/// so backslashed paths survive, and the program is looked up with `PATHEXT`, so `code` finds
/// `code.cmd`. The answer is what the user saved, minus the header and any blank lines around
/// it. Whether an empty answer is allowed is the caller's call.
///
/// An editor that returns within two seconds and leaves the file as it was probably handed it to
/// a running instance and isn't done; `confirm_done` is asked
/// [`prompt.returned_early`](EditorPrompt::returned_early), with a hint at the editor's wait flag
/// when it is a known one. A yes reads the file, a no reopens the editor.
///
/// # Errors
/// - [`PromptError::Cancelled`] when the editor dies of Ctrl-C, or `confirm_done` is cancelled.
/// - [`PromptError::Io`] when the temp file cannot be written or read, or the editor cannot be
///   found or started, or exits unsuccessfully.
pub fn edit<F>(
    prompt: &EditorPrompt<'_>,
    mut confirm_done: F,
) -> Result<Option<String>, PromptError>
where
    F: FnMut(&str) -> Result<bool, PromptError>,
{
    let editor = Editor::from_env(|name| std::env::var_os(name), cfg!(windows))?;
    let rendered = render(prompt);
    let mut file = tempfile::Builder::new()
        .prefix(&format!("{}-", file_safe(prompt.file_stem)))
        .suffix(&format!(".{}", prompt.extension))
        .tempfile()?;
    file.write_all(rendered.as_bytes())?;
    // Closed but kept until dropped, so the editor may replace the file and we read the new one.
    let path = file.into_temp_path();
    loop {
        let started = Instant::now();
        let ran = editor.run(&path, prompt.waiting)?;
        let saved = fs::read_to_string(&path)?;
        if !ran || !returned_early(started.elapsed(), &rendered, &saved) {
            return Ok(strip(&saved));
        }
        let question = editor.wait_fix().map_or_else(
            || prompt.returned_early.to_owned(),
            |fix| format!("{}\n{} {fix}", prompt.returned_early, prompt.wait_hint),
        );
        if confirm_done(&question)? {
            // The user may have saved in the running instance meanwhile.
            return Ok(strip(&fs::read_to_string(&path)?));
        }
    }
}

/// Check whether the editor came back too soon to have been used: within [`RETURNED_EARLY`], with
/// the file exactly as it was written.
fn returned_early(elapsed: Duration, rendered: &str, saved: &str) -> bool {
    elapsed < RETURNED_EARLY && saved == rendered
}

/// Run `command`, ignoring Ctrl-C until it exits, as git does: the editor gets Ctrl-C too, and
/// decides what it means.
fn wait(command: &mut Command) -> io::Result<ExitStatus> {
    let mut child = command.spawn()?;
    // Ignored only once the editor is spawned, so it starts with the default Ctrl-C handling.
    let _interrupts_ignored = IgnoredInterrupts::new();
    child.wait()
}

/// `stem` with anything but letters, digits, `-` and `_` replaced, to name a file with.
fn file_safe(stem: &str) -> String {
    stem.chars()
        .map(|char| if char.is_alphanumeric() || char == '-' || char == '_' { char } else { '-' })
        .collect()
}

/// `program` as found on `path` with one of `pathext`'s extensions, as `cmd` finds it, or
/// unchanged when it has a directory or an extension or isn't found.
///
/// Rust's own `PATH` search only tries `.exe`, and `code` is `code.cmd`.
#[cfg_attr(
    not(any(windows, test)),
    expect(dead_code, reason = "elsewhere `Command` finds the editor on `PATH` itself")
)]
fn resolve_windows(
    program: &str,
    path: Option<&OsStr>,
    pathext: Option<&OsStr>,
    is_file: impl Fn(&Path) -> bool,
) -> PathBuf {
    if program.contains(['/', '\\']) || Path::new(program).extension().is_some() {
        return PathBuf::from(program);
    }
    let pathext = pathext.map_or_else(|| ".COM;.EXE;.BAT;.CMD".into(), OsStr::to_string_lossy);
    let extensions: Vec<&str> = pathext.split(';').filter(|ext| !ext.is_empty()).collect();
    path.into_iter()
        .flat_map(std::env::split_paths)
        .flat_map(|dir| extensions.iter().map(move |ext| dir.join(format!("{program}{ext}"))))
        .find(|candidate| is_file(candidate))
        .unwrap_or_else(|| PathBuf::from(program))
}

/// `command_line` split on whitespace outside double quotes, the quotes dropped.
#[cfg_attr(
    not(any(windows, test)),
    expect(dead_code, reason = "elsewhere `shell_words` splits the editor string")
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

/// The answer in `file`: any byte order mark, the header lines and the blank lines around the
/// rest dropped. `None` when nothing is left.
fn strip(file: &str) -> Option<String> {
    // A byte order mark some Windows editors save is no part of the answer.
    let file = file.strip_prefix('\u{FEFF}').unwrap_or(file);
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

    fn editor(command: &str) -> Editor {
        Editor { command: OsString::from(command), dumb_terminal: false, variable: Some("EDITOR") }
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
            returned_early: "Your editor returned right away. Done editing the file?",
            styles: CommandStyles(&STYLES),
            wait_hint: "To make it wait, set",
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
    fn lookup_follows_git(#[case] env: &[(&str, &str)], #[case] expected: Option<&str>) {
        let editor = Editor::from_env(
            |name| env.iter().find(|(key, _)| *key == name).map(|(_, value)| OsString::from(value)),
            false,
        );
        let command = editor.ok().map(|editor| editor.command);
        assert_eq!(command.as_deref(), expected.map(OsStr::new));
    }

    #[rstest]
    #[case::visual(&[("TERM", "xterm"), ("VISUAL", "zed"), ("EDITOR", "vim")], Some("VISUAL"))]
    #[case::editor(&[("TERM", "xterm"), ("EDITOR", "vim")], Some("EDITOR"))]
    #[case::fallback(&[("TERM", "xterm")], None)]
    fn lookup_remembers_the_variable(#[case] env: &[(&str, &str)], #[case] expected: Option<&str>) {
        let editor = Editor::from_env(
            |name| env.iter().find(|(key, _)| *key == name).map(|(_, value)| OsString::from(value)),
            false,
        );
        assert_eq!(editor.ok().and_then(|editor| editor.variable), expected);
    }

    /// Windows terminals don't set `TERM`, so its absence isn't a dumb terminal there.
    #[rstest]
    #[case::nothing_set(&[], Some("notepad"))]
    #[case::visual_without_term(&[("VISUAL", "code --wait"), ("EDITOR", "vim")], Some("code --wait"))]
    #[case::editor_without_term(&[("EDITOR", "vim")], Some("vim"))]
    #[case::dumb_skips_visual(&[("TERM", "dumb"), ("VISUAL", "code"), ("EDITOR", "ed")], Some("ed"))]
    #[case::dumb_without_editor(&[("TERM", "dumb")], None)]
    fn lookup_on_windows_follows_git_for_windows(
        #[case] env: &[(&str, &str)],
        #[case] expected: Option<&str>,
    ) {
        let editor = Editor::from_env(
            |name| env.iter().find(|(key, _)| *key == name).map(|(_, value)| OsString::from(value)),
            true,
        );
        let command = editor.ok().map(|editor| editor.command);
        assert_eq!(command.as_deref(), expected.map(OsStr::new));
    }

    #[rstest]
    #[case::cmd_on_path("code", Some(("bin2", "code.CMD")))]
    #[case::first_extension_wins("vim", Some(("bin1", "vim.EXE")))]
    #[case::first_dir_wins("both", Some(("bin1", "both.EXE")))]
    #[case::not_found("nano", None)]
    fn resolve_windows_follows_pathext(
        #[case] program: &str,
        #[case] expected: Option<(&str, &str)>,
    ) {
        let (bin1, bin2) = (Path::new("bin1"), Path::new("bin2"));
        let files = [
            bin2.join("code.CMD"),
            bin1.join("vim.EXE"),
            bin1.join("vim.CMD"),
            bin1.join("both.EXE"),
            bin2.join("both.EXE"),
        ];
        let path = std::env::join_paths([bin1, bin2]).ok();
        let resolved =
            resolve_windows(program, path.as_deref(), Some(OsStr::new(".EXE;.CMD")), |candidate| {
                files.iter().any(|file| file == candidate)
            });
        let expected = expected
            .map_or_else(|| PathBuf::from(program), |(dir, file)| Path::new(dir).join(file));
        assert_eq!(resolved, expected);
    }

    #[rstest]
    #[case::directory(r"C:\Tools\vim")]
    #[case::extension("code.cmd")]
    fn resolve_windows_keeps_a_named_file(#[case] program: &str) {
        let resolved = resolve_windows(program, Some(OsStr::new("bin")), None, |_| true);
        assert_eq!(resolved, PathBuf::from(program));
    }

    #[test]
    fn render_puts_the_answer_before_the_header() {
        let file = render(&prompt(Some("Fixed it."), None));
        assert_eq!(file, "Fixed it.\n\n#| Release notes\n#|\n#| Leave the file empty to cancel.\n");
    }

    #[test]
    fn render_shows_the_rejected_answers_error() {
        let file = render(&prompt(Some("oops"), Some("invalid value 'oops'")));
        assert!(file.contains("#| error: invalid value 'oops'\n"));
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
    #[case::byte_order_mark("\u{FEFF}hello\n", Some("hello"))]
    #[case::byte_order_mark_alone("\u{FEFF}\n#| Question\n", None)]
    #[case::edited_header_line_goes("body\n#| I changed this\n", Some("body"))]
    #[case::markdown_and_shell_comments_stay(
        "## Notes\n#!/bin/sh\n#|x\n",
        Some("## Notes\n#!/bin/sh")
    )]
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
    #[case::bare("vim", &["vim"])]
    #[case::args("code --wait", &["code", "--wait"])]
    #[case::single_quotes("'/opt/my editor/bin/ed' -s", &["/opt/my editor/bin/ed", "-s"])]
    #[case::escaped_space(r"/opt/my\ editor/ed", &["/opt/my editor/ed"])]
    #[case::nothing_expanded("~/bin/ed $HOME", &["~/bin/ed", "$HOME"])]
    fn words_split_like_a_shell_without_expanding(
        #[case] command: &str,
        #[case] expected: &[&str],
    ) {
        let words = editor(command).words().ok();
        let expected: Vec<OsString> = expected.iter().map(OsString::from).collect();
        assert_eq!(words, Some(expected));
    }

    #[cfg(unix)]
    #[test]
    fn words_reject_an_unclosed_quote() {
        let words = editor("vim 'oops").words();
        let message = words.err().map(|err| err.to_string());
        assert_eq!(
            message.as_deref(),
            Some("Could not split the editor 'vim 'oops' into words: a quote is not closed.")
        );
    }

    #[rstest]
    #[case::code("code", Some("EDITOR=\"code --wait\""))]
    #[case::code_with_args("code -n", Some("EDITOR=\"code -n --wait\""))]
    #[case::code_cmd_path(r"C:\Tools\Code.cmd", None)]
    #[case::subl_path("/usr/local/bin/subl", Some("EDITOR=\"/usr/local/bin/subl -w\""))]
    #[case::already_waits("code --wait", None)]
    #[case::subl_long_flag("subl --wait", None)]
    #[case::terminal_editor("vim", None)]
    fn wait_fix_names_the_flag_for_known_editors(
        #[case] command: &str,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(editor(command).wait_fix().as_deref(), expected);
    }

    #[test]
    fn wait_fix_names_the_variable_the_editor_came_from() {
        let visual = Editor { variable: Some("VISUAL"), ..editor("zed") };
        assert_eq!(visual.wait_fix().as_deref(), Some("VISUAL=\"zed --wait\""));
    }

    #[rstest]
    #[case::fast_and_unchanged(Duration::from_millis(300), "file", true)]
    #[case::fast_but_edited(Duration::from_millis(300), "edited", false)]
    #[case::slow_and_unchanged(Duration::from_secs(5), "file", false)]
    fn returned_early_needs_fast_and_unchanged(
        #[case] elapsed: Duration,
        #[case] saved: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(returned_early(elapsed, "file", saved), expected);
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
    #[case::exits_130(130i32 << 8i32, false)]
    #[case::exits_1(1i32 << 8i32, false)]
    fn interrupted_reads_the_signal(#[case] raw: i32, #[case] expected: bool) {
        use std::os::unix::process::ExitStatusExt as _;

        assert_eq!(interrupted(ExitStatus::from_raw(raw)), expected);
    }

    #[cfg(unix)]
    #[test]
    fn run_reports_a_failing_editor() {
        let _signal_handling = lock_signal_handling();
        let ran = editor("false").run(Path::new("/dev/null"), "");
        let message = ran.err().map(|err| err.to_string());
        assert_eq!(message.as_deref(), Some("There was a problem with the editor 'false'."));
    }

    #[test]
    fn run_reports_a_missing_editor() {
        #[cfg(unix)]
        let _signal_handling = lock_signal_handling();
        let ran = editor("clap-couture-no-such-editor").run(Path::new("missing.txt"), "");
        let message = ran.err().map(|err| err.to_string());
        assert_eq!(
            message.as_deref(),
            Some("Could not find the editor 'clap-couture-no-such-editor'.")
        );
    }
}
