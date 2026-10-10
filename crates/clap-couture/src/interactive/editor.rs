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
    process::Command,
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
        let succeeded = self.command(path).status().is_ok_and(|status| status.success());
        if !succeeded {
            if hinted && !self.dumb_terminal {
                writeln!(io::stderr()).map_err(PromptError::Io)?;
            }
            return Err(PromptError::Io(io::Error::other(format!(
                "There was a problem with the editor '{}'.",
                self.command.to_string_lossy()
            ))));
        }
        if hinted && !self.dumb_terminal {
            write!(io::stderr(), "\r\x1b[K").map_err(PromptError::Io)?;
        }
        Ok(())
    }
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
        let file = tempfile::NamedTempFile::new();
        let path = file.as_ref().map(tempfile::NamedTempFile::path).ok();
        let ran = path.map(|path| {
            editor(command).run(path, "").and_then(|()| Ok(fs::read_to_string(path)?)).ok()
        });
        assert_eq!(ran.flatten().as_deref(), Some("from \"the editor\""));
    }

    #[cfg(unix)]
    #[test]
    fn run_reports_a_failing_editor() {
        let ran = editor("false").run(Path::new("/dev/null"), "");
        let message = ran.err().map(|err| err.to_string());
        assert_eq!(message.as_deref(), Some("There was a problem with the editor 'false'."));
    }
}
