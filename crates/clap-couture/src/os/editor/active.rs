//! An editing session: the file written, the editor open on it, and the file read back.

use core::time::Duration;
use std::{
    ffi::OsString,
    fs,
    io::{self, IsTerminal as _, Write as _},
    path::PathBuf,
    time::Instant,
};

use tempfile::TempDir;

use super::{Editor, EditorError, EditorSpawnOptions, FilePlace, PromptMarker, default_file_name};
use crate::os::{
    command::{CommandExt as _, Exit, InteractiveChild},
    signal::Signal,
    term::{self, Term},
};

/// An editor that exits sooner than this and leaves the file as it was most likely handed the
/// file to a running instance, like `code` without `--wait`, rather than being done with it.
const RETURNED_EARLY: Duration = Duration::from_millis(500);

/// The waiting hint, while the editor is open.
///
/// Some editors they kind of suck in that they do not wait until the file passed is open. `VsCode`
/// et al. Consequently, we have to tell the user to "finish" their command.
///
/// When they're done, we need to erase the hint.
#[derive(Debug)]
struct Hint {
    /// Erased once the editor exits cleanly.
    erased: bool,
    text: String,
}

/// An editor open on its file. Dropping it waits for the editor, then removes a temporary file.
#[must_use = "dropping it waits for the editor to exit"]
pub(crate) struct Opened {
    /// Removed on drop, after the editor exits.
    _dir: Option<TempDir>,
    child: Option<InteractiveChild>,
    editor: Editor,
    hint: Option<Hint>,
    path: PathBuf,
    prompt_marker: PromptMarker,
    started: Instant,
    written: Vec<u8>,
}

impl Opened {
    /// Show the hint and start the editor.
    fn launch(&mut self) -> Result<(), SpawnError> {
        // Built before the hint shows, so a missing editor reports without one.
        let mut command = self.editor.command(&self.path)?;
        if let Some(hint) = &self.hint {
            let end = if hint.erased { ' ' } else { '\n' };
            write!(io::stderr(), "{}{end}", hint.text)?;
        }

        self.started = Instant::now();
        self.child = Some(command.spawn_interactive().map_err(|err| {
            if err.kind() == io::ErrorKind::NotFound {
                SpawnError::Editor(EditorError::NotFound(self.editor.command.clone()))
            } else {
                SpawnError::Io(err)
            }
        })?);
        Ok(())
    }

    /// Read the file as it is now.
    pub(crate) fn read(&self) -> io::Result<Vec<u8>> {
        fs::read(&self.path)
    }

    /// Open the editor on the file again.
    pub(crate) fn reopen(&mut self) -> Result<(), SpawnError> {
        drop(self.child.take());
        self.launch()
    }

    pub(super) fn start(
        editor: &Editor,
        options: EditorSpawnOptions<'_>,
    ) -> Result<Self, SpawnError> {
        let (dir, path) = match options.place {
            FilePlace::Path(path) => (None, path),
            FilePlace::TempFile(name) => {
                let prefix = crate::os::process_name().map(|mut prefix| {
                    prefix.push("-");
                    prefix
                });
                let mut builder = tempfile::Builder::new();
                if let Some(prefix) = &prefix {
                    builder.prefix(prefix);
                }
                // Private from creation: the draft, and the editor's swap files beside it, would
                // otherwise be readable by anyone sharing the temporary directory.
                #[cfg(unix)]
                builder.permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700));
                let dir = builder.tempdir()?;
                let name = name.file_name().map_or_else(default_file_name, Into::into);
                let path = dir.path().join(name);
                let mut new_file = fs::OpenOptions::new();
                new_file.write(true).create_new(true);
                #[cfg(unix)]
                std::os::unix::fs::OpenOptionsExt::mode(&mut new_file, 0o600);
                new_file.open(&path)?;
                (Some(dir), path)
            }
        };

        let mut written = Vec::new();
        options.prompt_marker.write(&mut written, options.contents, options.prompt)?;
        fs::write(&path, &written)?;

        let hint = options.hint.filter(|hint| !hint.is_empty() && io::stderr().is_terminal());

        let dumb_terminal = Term::read().map_or(cfg!(not(windows)), |term| term.is_dumb());
        let hint = hint.map(|text| Hint {
            erased: !dumb_terminal && !editor.is_console(),
            text: text.to_owned(),
        });

        let mut opened = Self {
            child: None,
            _dir: dir,
            editor: editor.clone(),
            hint,
            path,
            prompt_marker: options.prompt_marker,
            started: Instant::now(),
            written,
        };
        opened.launch()?;
        Ok(opened)
    }

    /// Wait for the editor to exit, then read the file.
    pub(crate) fn wait(&mut self) -> Result<Vec<u8>, WaitError> {
        let Some(child) = self.child.take() else { return Ok(self.read()?) };
        let exit = child.wait();

        if let Some(hint) = self.hint.as_ref().filter(|hint| hint.erased) {
            // Erase the hint and its trailing space after a clean exit; otherwise keep it,
            // ending its line.
            let mut stderr = io::stderr();
            let erasing = if matches!(exit, Ok(Exit::Success)) {
                let columns = term::visible_width(&hint.text).saturating_add(1);
                let vt = term::has_vt(&stderr);
                term::erase_line(&mut stderr, columns, vt)
            } else {
                writeln!(stderr)
            };

            drop(erasing);
        }

        match exit? {
            Exit::Success => {}
            Exit::Failed => return Err(WaitError::Failed(self.editor.command.clone())),
            Exit::Killed(signal) => return Err(WaitError::Killed(signal)),
        }

        let saved = fs::read(&self.path)?;
        if self.started.elapsed() < RETURNED_EARLY && saved == self.written {
            return Err(WaitError::ReturnedEarly);
        }

        Ok(saved)
    }

    /// Write `contents` and `prompt` to the file, for the next [`Opened::reopen`].
    pub(crate) fn write(&mut self, contents: &str, prompt: Option<&str>) -> io::Result<()> {
        self.written.clear();
        self.prompt_marker.write(&mut self.written, contents, prompt)?;
        fs::write(&self.path, &self.written)?;
        Ok(())
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        // The editor may still have the file open: wait for it before the directory goes.
        drop(self.child.take());
    }
}

/// Why the editor couldn't be opened.
#[derive(Debug)]
pub(crate) enum SpawnError {
    Editor(EditorError),
    Io(io::Error),
}

impl From<EditorError> for SpawnError {
    fn from(err: EditorError) -> Self {
        Self::Editor(err)
    }
}

impl From<io::Error> for SpawnError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// Why the editor's file couldn't be read back.
#[derive(Debug)]
pub(crate) enum WaitError {
    /// The editor exited unsuccessfully. Holds its command.
    Failed(OsString),
    Io(io::Error),
    /// The editor died of the terminal's signal.
    Killed(Signal),
    /// The editor returned at once and left the file as it was.
    ReturnedEarly,
}

impl From<io::Error> for WaitError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::editor::{EditorSpawnOptions, tests::editor};
    #[cfg(unix)]
    use crate::os::locale::Locale;

    #[cfg(unix)]
    const PROMPT: &str = "Release notes\n\nLeave the file empty to cancel.";

    #[test]
    fn the_no_op_editor_isnt_spawned() {
        let opened = editor(":").spawn(EditorSpawnOptions::builder().build());
        assert!(matches!(opened, Err(SpawnError::Editor(EditorError::NoOp))));
    }

    #[cfg(unix)]
    #[test]
    fn wait_reads_back_what_the_editor_saved_without_the_prompt() {
        let _handling = crate::os::signal::tests::lock_handling();
        let opened = editor(r#"sh -c 'printf "Saved.\n" > "$0"'"#)
            .spawn(EditorSpawnOptions::builder().prompt(PROMPT).build());
        let saved = opened.ok().and_then(|mut opened| opened.wait().ok());
        let answer = saved.and_then(|saved| Locale::decode(&saved).ok());
        let answer = answer.map(|text| PromptMarker::default().strip(&text));
        assert_eq!(answer, Some(Some("Saved.".to_owned())));
    }

    #[cfg(unix)]
    #[test]
    fn wait_returns_the_bytes_as_saved() {
        let _handling = crate::os::signal::tests::lock_handling();
        let opened = editor(r#"sh -c 'printf "caf\\351" > "$0"'"#)
            .spawn(EditorSpawnOptions::builder().prompt(PROMPT).build());
        let saved = opened.ok().and_then(|mut opened| opened.wait().ok());
        assert_eq!(saved.as_deref(), Some(b"caf\xE9".as_slice()));
    }

    #[cfg(unix)]
    #[test]
    fn wait_strips_a_custom_marker() {
        let _handling = crate::os::signal::tests::lock_handling();
        let options = EditorSpawnOptions::builder()
            .prompt(PROMPT)
            .prompt_marker(PromptMarker::new("//"))
            .build();
        let opened = editor(r#"sh -c 'printf "Saved.\n// edited\n" > "$0"'"#).spawn(options);
        let saved = opened.ok().and_then(|mut opened| opened.wait().ok());
        let answer = saved.and_then(|saved| Locale::decode(&saved).ok());
        let answer = answer.map(|text| PromptMarker::new("//").strip(&text));
        assert_eq!(answer, Some(Some("Saved.".to_owned())));
    }

    #[cfg(unix)]
    #[test]
    fn an_editor_that_returns_at_once_is_reported() {
        let _handling = crate::os::signal::tests::lock_handling();
        let opened = editor("true").spawn(EditorSpawnOptions::builder().contents("x").build());
        let answer = opened.map(|mut opened| opened.wait());
        assert!(matches!(answer, Ok(Err(WaitError::ReturnedEarly))));
    }

    #[cfg(unix)]
    #[test]
    fn a_temporary_file_stays_in_its_directory() {
        let _handling = crate::os::signal::tests::lock_handling();
        let opened =
            editor("true").spawn(EditorSpawnOptions::builder().tmpfile("../escaped.txt").build());
        let path = opened.map(|opened| opened.path.clone());
        assert!(path.is_ok_and(|path| path.file_name() == Some("escaped.txt".as_ref())
            && path.parent().is_some_and(|dir| dir.starts_with(std::env::temp_dir()))));
    }

    #[cfg(unix)]
    #[test]
    fn the_temporary_file_is_private_while_the_editor_is_open() {
        let _handling = crate::os::signal::tests::lock_handling();
        let modes = r#"sh -c '{ ls -ld "$(dirname "$0")"; ls -l "$0"; } | cut -c1-10 > "$0"'"#;
        let opened = editor(modes).spawn(EditorSpawnOptions::builder().contents("x").build());
        let saved = opened.ok().and_then(|mut opened| opened.wait().ok());
        assert_eq!(saved.as_deref(), Some(b"drwx------\n-rw-------\n".as_slice()));
    }

    #[cfg(unix)]
    #[test]
    fn a_file_at_a_path_stays() {
        let _handling = crate::os::signal::tests::lock_handling();
        let dir = tempfile::tempdir();
        let path = dir.as_ref().map(|dir| dir.path().join("notes.txt"));
        let kept = path.as_ref().is_ok_and(|path| {
            let opened = editor("true")
                .spawn(EditorSpawnOptions::builder().contents("x").path(path).build());
            drop(opened);
            path.exists()
        });
        assert!(kept);
    }

    #[cfg(unix)]
    #[test]
    fn dropping_removes_the_temporary_file() {
        let _handling = crate::os::signal::tests::lock_handling();
        let path = editor("true")
            .spawn(EditorSpawnOptions::builder().tmpfile("notes.txt").build())
            .map(|opened| opened.path.clone());
        assert!(path.is_ok_and(|path| path.ends_with("notes.txt") && !path.exists()));
    }
}
