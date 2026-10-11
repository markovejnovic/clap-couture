//! The user's editor, found the way git finds its own.

mod active;
mod prompt_marker;

use alloc::borrow::Cow;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) use active::{Opened, SpawnError, WaitError};
pub(crate) use prompt_marker::PromptMarker;

use super::{command, term::Term};

/// How known editors behave, by program name.
const EDITOR_BEHAVIORS: &[(&str, EditorBehavior)] = &[
    ("atom", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("bbedit", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("clion", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("code", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("code-insiders", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("codium", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("cursor", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("datagrip", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("emacsclient", EditorBehavior { is_console: true, wait_flag: None }),
    ("gedit", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("goland", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("gvim", EditorBehavior { is_console: true, wait_flag: Some("-f") }),
    ("idea", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("idea64", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("intellij-idea", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("kate", EditorBehavior { is_console: true, wait_flag: Some("--block") }),
    ("mate", EditorBehavior { is_console: true, wait_flag: Some("-w") }),
    ("mvim", EditorBehavior { is_console: true, wait_flag: Some("-f") }),
    ("notepad++", EditorBehavior { is_console: true, wait_flag: Some("-multiInst -nosession") }),
    ("open", EditorBehavior { is_console: true, wait_flag: Some("-W") }),
    ("phpstorm", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("pycharm", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("rider", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("rubymine", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("rustrover", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("studio", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("subl", EditorBehavior { is_console: true, wait_flag: Some("-w") }),
    ("webstorm", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("windsurf", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("xed", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
    ("zed", EditorBehavior { is_console: true, wait_flag: Some("--wait") }),
];

/// How a known editor behaves.
#[derive(Debug)]
struct EditorBehavior {
    /// It writes to the console while open.
    is_console: bool,
    /// The flag that makes it wait for the file to be closed, if it returns at once without it.
    wait_flag: Option<&'static str>,
}

/// The editor read from `$VISUAL` and `$EDITOR`.
#[derive(Debug, Clone)]
pub(crate) struct Editor {
    /// The raw command held in $TERM.
    pub(crate) command: OsString,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EditorError {
    /// $TERM is configured to be dumb.
    DumbTerminal,
    /// The editor is `:`, git's no-op editor, which isn't run.
    NoOp,
    /// $TERM could not find the terminal. The member contains the $TERM value.
    NotFound(OsString),
    /// $TERM had an unclosed quote.
    UnclosedQuote(OsString),
}

impl Editor {
    /// The behavior of a known editor.
    fn behavior(&self) -> Option<&'static EditorBehavior> {
        let program = self.program()?;
        EDITOR_BEHAVIORS.iter().find(|(name, _)| *name == program).map(|(_, behavior)| behavior)
    }

    /// The editor's process for `path`: its words, the wait flag a known editor needs, then the
    /// file. The program is looked up on
    /// `PATH` first, with `PATHEXT` on Windows.
    ///
    /// Certain editors need special wait flags attached to them, and this will add those if
    /// necessary.
    pub(crate) fn command(&self, path: &Path) -> Result<Command, EditorError> {
        // Git handles $TERM=":" as a no-op, so we'll do the same.
        if self.command == ":" {
            return Err(EditorError::NoOp);
        }

        let mut cmd_args = self.cmd_args();
        let program = cmd_args.next().transpose()?.unwrap_or_default();
        let found = which::which(&program)
            .map_err(|_not_found| EditorError::NotFound(self.command.clone()))?;

        let mut command = Command::new(found);
        #[cfg(unix)]
        std::os::unix::process::CommandExt::arg0(&mut command, &program);
        for word in cmd_args {
            command.arg(word?);
        }
        command.args(self.wait_flag().into_iter().flat_map(|flag| flag.split(' ')));
        command.arg(path);

        Ok(command)
    }

    /// Check whether an editor writes to the console or not.
    pub(crate) fn is_console(&self) -> bool {
        self.behavior().is_some_and(|behavior| behavior.is_console)
    }

    /// The program's name, lowercased and without directory or extension, like `code`.
    fn program(&self) -> Option<OsString> {
        let program = self.cmd_args().next()?.ok()?;
        Some(Path::new(&program).file_stem()?.to_ascii_lowercase())
    }

    /// Look the editor from `$VISUAL` unless the terminal is dumb, then `$EDITOR`, then `vi`
    /// (`notepad` on Windows).
    pub(crate) fn read() -> Result<Self, EditorError> {
        // This is git's logic. An unset TERM is dumb, except on Windows.
        let dumb_terminal = Term::read().map_or(cfg!(not(windows)), |term| term.is_dumb());

        let set = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());

        let visual = if dumb_terminal { None } else { set("VISUAL") };

        let command = match visual.or_else(|| set("EDITOR")) {
            Some(command) => command,
            None if dumb_terminal => return Err(EditorError::DumbTerminal),
            None => OsString::from(if cfg!(windows) { "notepad" } else { "vi" }),
        };
        Ok(Self { command })
    }

    /// Write the file `options` describe and open the editor on it, without waiting for it.
    pub(crate) fn spawn(&self, options: EditorSpawnOptions<'_>) -> Result<Opened, SpawnError> {
        Opened::start(self, options)
    }

    /// The flag a known editor needs to wait for the file to be closed, unless already given.
    fn wait_flag(&self) -> Option<&'static str> {
        let flag = self.behavior()?.wait_flag?;
        let first_flag = flag.split(' ').next().unwrap_or(flag);
        let given = self
            .cmd_args()
            .skip(1)
            .any(|word| word.is_ok_and(|word| word == first_flag || word == "--wait"));
        (!given).then_some(flag)
    }

    /// The editor string split into words.
    fn cmd_args(&self) -> impl Iterator<Item = Result<OsString, EditorError>> + '_ {
        command::split(&self.command).map(|word| {
            word.map(expand_tilde)
                .map_err(|_unclosed_quote| EditorError::UnclosedQuote(self.command.clone()))
        })
    }
}

/// How to open the editor, built with [`EditorSpawnOptions::builder`].
#[derive(Debug)]
pub(crate) struct EditorSpawnOptions<'options> {
    contents: &'options str,
    hint: Option<&'options str>,
    place: FilePlace,
    prompt: Option<&'options str>,
    prompt_marker: PromptMarker,
}

impl<'options> EditorSpawnOptions<'options> {
    /// Options for a file in a temporary directory, empty and without a prompt.
    pub(crate) fn builder() -> EditorSpawnOptionsBuilder<'options> {
        EditorSpawnOptionsBuilder(Self::default())
    }
}

impl Default for EditorSpawnOptions<'_> {
    fn default() -> Self {
        Self {
            contents: "",
            hint: None,
            place: FilePlace::TempFile(default_file_name()),
            prompt: None,
            prompt_marker: PromptMarker::default(),
        }
    }
}

/// Builds [`EditorSpawnOptions`].
#[derive(Debug)]
#[must_use]
pub(crate) struct EditorSpawnOptionsBuilder<'options>(EditorSpawnOptions<'options>);

impl<'options> EditorSpawnOptionsBuilder<'options> {
    pub(crate) fn build(self) -> EditorSpawnOptions<'options> {
        self.0
    }

    /// The text the file starts with. `.contents("Fixed the login bug.")` opens:
    ///
    /// ```text
    /// Fixed the login bug.
    /// ```
    pub(crate) const fn contents(mut self, contents: &'options str) -> Self {
        self.0.contents = contents;
        self
    }

    /// A line printed to the terminal while the editor is open, erased once it closes; empty
    /// prints nothing. `.hint("hint: Waiting for your editor to close the file...")` shows:
    ///
    /// ```text
    /// $ deploy
    /// hint: Waiting for your editor to close the file...
    /// ```
    pub(crate) const fn hint(mut self, hint: &'options str) -> Self {
        self.0.hint = Some(hint);
        self
    }

    /// Put the file at `path`, and leave it there.
    #[cfg_attr(
        not(all(test, unix)),
        expect(dead_code, reason = "prompts only edit temporary files")
    )]
    pub(crate) fn path(mut self, path: impl Into<PathBuf>) -> Self {
        self.0.place = FilePlace::Path(path.into());
        self
    }

    /// A question below the contents, removed from what the editor saves. With the contents
    /// above, `.prompt("Release notes\n\nLeave the file empty to cancel.")` opens:
    ///
    /// ```text
    /// Fixed the login bug.
    ///
    /// #| Release notes
    /// #|
    /// #| Leave the file empty to cancel.
    /// ```
    pub(crate) const fn prompt(mut self, prompt: &'options str) -> Self {
        self.0.prompt = Some(prompt);
        self
    }

    /// What each prompt line starts with, `#|` by default. With the contents and prompt above,
    /// `.prompt_marker("//")` opens:
    ///
    /// ```text
    /// Fixed the login bug.
    ///
    /// // Release notes
    /// //
    /// // Leave the file empty to cancel.
    /// ```
    #[cfg_attr(not(all(test, unix)), expect(dead_code, reason = "prompts keep the default marker"))]
    pub(crate) fn prompt_marker(mut self, prompt_marker: PromptMarker) -> Self {
        self.0.prompt_marker = prompt_marker;
        self
    }

    /// Put the file, named `name`, in a temporary directory removed with [`Opened`].
    pub(crate) fn tmpfile(mut self, name: impl Into<PathBuf>) -> Self {
        self.0.place = FilePlace::TempFile(name.into());
        self
    }
}

/// Where the editor's file goes.
#[derive(Debug)]
enum FilePlace {
    Path(PathBuf),
    TempFile(PathBuf),
}

/// The file name when none is given: the program's, like `git.txt`.
fn default_file_name() -> PathBuf {
    let mut name = crate::os::process_name().map_or_else(
        || OsString::from("edit"),
        |name| if let Cow::Owned(safe) = crate::fs::file_safe(&name) { safe } else { name },
    );
    name.push(".txt");
    name.into()
}

fn expand_tilde(word: OsString) -> OsString {
    // The split has already dropped any quotes, so a quoted `'~'` expands too.
    let Ok(rest) = Path::new(&word).strip_prefix("~") else { return word };
    let Some(home) = std::env::home_dir() else { return word };
    if rest.as_os_str().is_empty() { home } else { home.join(rest) }.into_os_string()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    pub(super) fn editor(command: &str) -> Editor {
        Editor { command: OsString::from(command) }
    }

    #[test]
    fn the_no_op_editor_has_no_command() {
        let command = editor(":").command(Path::new("notes.txt"));
        assert_eq!(command.err(), Some(EditorError::NoOp));
    }

    #[test]
    fn a_missing_program_is_not_found() {
        let command = editor("clap-couture-no-such-editor --flag").command(Path::new("notes.txt"));
        let expected = EditorError::NotFound(OsString::from("clap-couture-no-such-editor --flag"));
        assert_eq!(command.err(), Some(expected));
    }

    #[cfg(unix)]
    #[test]
    fn an_unclosed_quote_is_an_error() {
        let command = editor("vim 'oops").command(Path::new("notes.txt"));
        assert_eq!(command.err(), Some(EditorError::UnclosedQuote(OsString::from("vim 'oops"))));
    }

    #[rstest]
    #[case::code("code", Some("--wait"))]
    #[case::code_with_args("code -n", Some("--wait"))]
    #[case::code_cmd_path(r"C:\Tools\Code.cmd", None)]
    #[case::subl_path("/usr/local/bin/subl", Some("-w"))]
    #[case::already_waits("code --wait", None)]
    #[case::subl_long_flag("subl --wait", None)]
    #[case::terminal_editor("vim", None)]
    #[case::jetbrains("idea", Some("--wait"))]
    #[case::jetbrains_on_windows("idea64", Some("--wait"))]
    #[case::notepad_plus_plus("notepad++", Some("-multiInst -nosession"))]
    #[case::notepad_plus_plus_given("notepad++ -multiInst", None)]
    #[case::xcode("xed", Some("--wait"))]
    fn wait_flag_for_known_editors(#[case] command: &str, #[case] expected: Option<&str>) {
        assert_eq!(editor(command).wait_flag(), expected);
    }

    #[rstest]
    #[case::program("~/bin/ed --flag", &["~/bin/ed", "--flag"])]
    #[case::home_alone("vim ~", &["vim", "~"])]
    #[case::argument("vim -u ~/.vimrc", &["vim", "-u", "~/.vimrc"])]
    #[case::other_users_home("~root/ed", &[])]
    #[case::not_leading("a~b ./~", &[])]
    fn a_leading_tilde_is_the_home_directory(#[case] command: &str, #[case] expanded: &[&str]) {
        let home = std::env::home_dir().unwrap_or_default();
        let expected: Vec<OsString> = if expanded.is_empty() {
            command.split(' ').map(OsString::from).collect()
        } else {
            expanded
                .iter()
                .map(|word| match word.strip_prefix('~') {
                    Some("") => home.clone().into_os_string(),
                    Some(rest) => home.join(rest.trim_start_matches('/')).into_os_string(),
                    None => OsString::from(word),
                })
                .collect()
        };
        let words: Result<Vec<_>, _> = editor(command).cmd_args().collect();
        assert_eq!(words, Ok(expected));
    }

    #[rstest]
    #[case::gui_editor("code --wait", true)]
    #[case::emacsclient("/usr/bin/emacsclient -t", true)]
    #[case::terminal_editor("vim", false)]
    #[case::nano("nano -w", false)]
    fn is_console_for_editors_that_write_to_it(#[case] command: &str, #[case] expected: bool) {
        assert_eq!(editor(command).is_console(), expected);
    }
}
