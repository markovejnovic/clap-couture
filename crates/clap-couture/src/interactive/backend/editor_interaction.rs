//! Answer a prompted arg in the user's $EDITOR.

use std::{ffi::OsStr, io};

use super::Backend;
use crate::{
    fs,
    interactive::{ConfirmPrompt, EditorPrompt, PromptError},
    os::{
        editor::{
            Editor, EditorError, EditorSpawnOptions, Opened, PromptMarker, SpawnError, WaitError,
        },
        locale::{Locale, NotUnicode},
    },
};

pub struct EditorInteraction<'run, B: Backend + ?Sized> {
    backend: &'run B,
    /// The header the file was last opened with.
    header: String,
    marker: PromptMarker,
    opened: Option<Opened>,
    /// How many U+FFFD the answer may hold: those that were text, not marks for bytes that
    /// couldn't be read. `None` until the file couldn't be read.
    own_marks: Option<usize>,
    prompt: &'run EditorPrompt<'run>,
}

impl<'run, B: Backend + ?Sized> EditorInteraction<'run, B> {
    /// Ask `prompt` in the user's `$EDITOR` returning `None` for an empty answer. The answer ends
    /// in a newline, as the file does.
    ///
    /// # Errors
    /// - [`PromptError::Signalled`] when the editor dies of Ctrl-C or of the terminal closing;
    ///   [`PromptError::Cancelled`] on Windows.
    /// - [`PromptError::Cancelled`] when `confirm_done` is cancelled.
    /// - [`PromptError::Io`] when the temp file cannot be written or read, or the editor cannot be
    ///   found or started, or exits unsuccessfully.
    pub fn answer(mut self) -> Result<Option<String>, PromptError> {
        let Some(opened) = self.opened.as_mut() else {
            // `:` is git's no-op editor: the initial text is taken as written.
            let initial = self.prompt.initial.map(str::trim).filter(|initial| !initial.is_empty());
            return Ok(initial.map(|initial| format!("{initial}\n")));
        };

        loop {
            let saved = match opened.wait() {
                Err(WaitError::ReturnedEarly) => {
                    let done = self.backend.confirm(&ConfirmPrompt {
                        default: true,
                        question: self.prompt.returned_early,
                        styles: self.prompt.styles,
                    })?;

                    if !done {
                        opened.reopen()?;
                        continue;
                    }
                    // The user may have saved in the running instance meanwhile.
                    Ok(opened.read()?)
                }
                saved => saved,
            };

            // Reopen with what could be read, and how to fix the rest.
            let (answer, error) = match Locale::decode(&saved?) {
                Ok(text) => {
                    let answer = self.marker.strip(&text, Some(&self.header));
                    let marks = answer.as_deref().map_or(0, Self::count_marks);
                    if self.own_marks.is_none_or(|own_marks| marks <= own_marks) {
                        return Ok(answer);
                    }
                    (answer, self.prompt.unreadable)
                }
                Err(NotUnicode { marks, text }) => {
                    let answer = self.marker.strip(&text, Some(&self.header));
                    // Marks left from an earlier reopen read as text now: never count more.
                    let own_marks =
                        answer.as_deref().map_or(0, Self::count_marks).saturating_sub(marks);
                    self.own_marks =
                        Some(self.own_marks.map_or(own_marks, |own| own.min(own_marks)));
                    (answer, self.prompt.not_utf8)
                }
            };
            self.header =
                format!("{}\n\nerror: {error}\n{}", self.prompt.question, self.prompt.instructions);
            opened.write(answer.as_deref().unwrap_or_default(), Some(&self.header))?;
            opened.reopen()?;
        }
    }

    /// How many U+FFFD `text` holds.
    fn count_marks(text: &str) -> usize {
        text.matches(char::REPLACEMENT_CHARACTER).count()
    }

    /// # Errors
    /// - [`PromptError::Io`] when `$EDITOR` can't be used, the file can't be written, or the editor
    ///   can't be started.
    pub fn start(backend: &'run B, prompt: &'run EditorPrompt<'run>) -> Result<Self, PromptError> {
        let editor = Editor::read()?;
        let (question, instructions) = (prompt.question, prompt.instructions);
        let header = prompt.error.map_or_else(
            || format!("{question}\n\n{instructions}"),
            |error| format!("{question}\n\nerror: {error}\n{instructions}"),
        );

        let mut file_name = fs::file_safe(OsStr::new(prompt.file_stem)).into_owned();
        file_name.push(".");
        file_name.push(prompt.extension);

        let options = EditorSpawnOptions::builder()
            .contents(prompt.initial.unwrap_or_default())
            .prompt(&header)
            .tmpfile(file_name)
            .hint(prompt.waiting)
            .build();

        let opened = match editor.spawn(options) {
            Err(SpawnError::Editor(EditorError::NoOp)) => None,
            opened => Some(opened?),
        };
        Ok(Self {
            backend,
            header,
            marker: PromptMarker::default(),
            opened,
            own_marks: None,
            prompt,
        })
    }
}

impl From<EditorError> for PromptError {
    fn from(err: EditorError) -> Self {
        Self::Io(match err {
            EditorError::DumbTerminal => io::Error::other("Terminal is dumb, but EDITOR unset"),
            EditorError::NoOp => io::Error::other("The editor is `:`, which edits nothing."),
            EditorError::NotFound(command) => io::Error::new(
                io::ErrorKind::NotFound,
                format!("Could not find the editor '{}'.", command.to_string_lossy()),
            ),
            EditorError::UnclosedQuote(command) => io::Error::other(format!(
                "Could not split the editor '{}' into words: a quote is not closed.",
                command.to_string_lossy()
            )),
        })
    }
}

impl From<SpawnError> for PromptError {
    fn from(err: SpawnError) -> Self {
        match err {
            SpawnError::Editor(err) => err.into(),
            SpawnError::Io(err) => Self::Io(err),
        }
    }
}

impl From<WaitError> for PromptError {
    fn from(err: WaitError) -> Self {
        match err {
            WaitError::Failed(editor) => Self::Io(io::Error::other(format!(
                "There was a problem with the editor '{}'.",
                editor.to_string_lossy()
            ))),
            WaitError::Io(err) => Self::Io(err),
            // The user pressed Ctrl-C in the editor, or closed the terminal: back out like any
            // cancelled prompt, removing the temp file.
            WaitError::Killed(signal) => Self::Signalled(signal),
            WaitError::ReturnedEarly => {
                Self::Io(io::Error::other("the editor's file was not read"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use crate::os::signal::tests::lock_handling as lock_signal_handling;

    fn editor(command: &str) -> Editor {
        Editor { command: std::ffi::OsString::from(command) }
    }

    #[test]
    fn an_unclosed_quote_is_reported() {
        let error = PromptError::from(EditorError::UnclosedQuote("vim 'oops".into()));
        assert_eq!(
            error.to_string(),
            "Could not split the editor 'vim 'oops' into words: a quote is not closed."
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_editor_is_reported() {
        let _signal_handling = lock_signal_handling();
        let answer = editor("false")
            .spawn(EditorSpawnOptions::builder().build())
            .map_err(PromptError::from)
            .and_then(|mut opened| opened.wait().map_err(PromptError::from));
        let message = answer.err().map(|err| err.to_string());
        assert_eq!(message.as_deref(), Some("There was a problem with the editor 'false'."));
    }

    #[test]
    fn a_missing_editor_is_reported() {
        let opened =
            editor("clap-couture-no-such-editor").spawn(EditorSpawnOptions::builder().build());
        let message = opened.err().map(|err| PromptError::from(err).to_string());
        assert_eq!(
            message.as_deref(),
            Some("Could not find the editor 'clap-couture-no-such-editor'.")
        );
    }
}
