//! `Backend::editor`'s default spawns `$EDITOR` directly, its words split like a shell would. The
//! test sets process-wide variables, so it lives alone in its own binary, in a single test.
#![cfg(all(feature = "interactive-editor", unix))]

use core::cell::RefCell;
use std::{
    fs, io,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
};

use clap::Parser;
use clap_couture::{
    Couture, CoutureParser as _,
    interactive::{Backend, ConfirmPrompt, PromptError, SelectPrompt, TextPrompt},
};

#[derive(Parser, Couture)]
struct Release {
    /// Release notes.
    #[arg(long, default_value = "Bug fixes.")]
    #[couture(prompt, editor)]
    notes: String,
}

/// A terminal that opens the editor through the default `Backend::editor`, and answers its
/// "done editing?" question from a script.
#[derive(Default)]
struct DefaultEditor {
    /// The confirm questions asked, oldest first.
    asked: RefCell<Vec<String>>,
    /// The confirm answers, next one first.
    replies: RefCell<Vec<bool>>,
}

impl DefaultEditor {
    fn replying(replies: &[bool]) -> Self {
        Self { asked: RefCell::default(), replies: RefCell::new(replies.to_vec()) }
    }
}

impl Backend for DefaultEditor {
    fn confirm(&self, prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        self.asked.borrow_mut().push(prompt.question.to_owned());
        let mut replies = self.replies.borrow_mut();
        if replies.is_empty() {
            return Err(PromptError::Io(io::Error::other("no confirm reply was scripted")));
        }
        Ok(replies.remove(0))
    }

    fn is_available(&self) -> bool {
        true
    }

    fn select(&self, _prompt: &SelectPrompt<'_>) -> Result<Option<String>, PromptError> {
        Err(not_editor())
    }

    fn text(&self, _prompt: &TextPrompt<'_>) -> Result<Option<String>, PromptError> {
        Err(not_editor())
    }
}

fn not_editor() -> PromptError {
    PromptError::Io(io::Error::other("only editor prompts should be asked"))
}

/// An executable `sh` script named `name` in `dir`, standing in for an editor.
fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).ok();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).ok();
    path
}

/// The notes parsed with `editor` as `$EDITOR` and `backend` asking, or clap's message.
fn notes_with(editor: &str, backend: &DefaultEditor) -> Result<String, String> {
    // SAFETY: this binary holds a single test, so no other thread reads the environment.
    unsafe {
        std::env::set_var("EDITOR", editor);
    }
    Release::couture_try_parse_from_with(backend, ["release"])
        .map(|release| release.notes)
        .map_err(|err| err.to_string())
}

#[test]
fn the_default_backend_spawns_the_editor_itself() {
    // SAFETY: as in `notes_with`.
    unsafe {
        std::env::set_var("TERM", "xterm");
    }
    // SAFETY: as in `notes_with`.
    unsafe {
        std::env::remove_var("VISUAL");
    }
    let editors = std::env::temp_dir().join(format!("clap-couture-editors-{}", std::process::id()));
    fs::create_dir_all(&editors).ok();
    let dir = editors.as_path();
    let quiet = DefaultEditor::default;
    let no_questions: [&str; 0] = [];

    // Words split like a shell: the quoted arg arrives whole, before the file. Fast, but the file
    // changed, so nobody is asked whether the editor is done.
    let overwrite = script(dir, "overwrite", r#"printf 'from "%s"\n' "$1" > "$2""#);
    let backend = quiet();
    let notes = notes_with(&format!("{} 'the editor'", overwrite.display()), &backend);
    assert_eq!(notes.as_deref(), Ok("from \"the editor\""));
    assert_eq!(backend.asked.take(), no_questions);

    // Appends after the header; the header comes back out, the default stays.
    let append = script(dir, "append", r#"printf 'More.\n' >> "$1""#);
    let notes = notes_with(&append.display().to_string(), &quiet());
    assert_eq!(notes.as_deref(), Ok("Bug fixes.\n\nMore."));

    // Nothing in `$EDITOR` is expanded: `$HOME` reaches the editor as is.
    let echo_arg = script(dir, "echo-arg", r#"printf '%s\n' "$1" > "$2""#);
    let notes = notes_with(&format!("{} '$HOME'", echo_arg.display()), &quiet());
    assert_eq!(notes.as_deref(), Ok("$HOME"));

    // `:` is git's no-op editor: the pre-filled default is taken, and nobody is asked.
    let backend = quiet();
    assert_eq!(notes_with(":", &backend).as_deref(), Ok("Bug fixes."));
    assert_eq!(backend.asked.take(), no_questions);

    // Returned at once, file untouched, like `code` without `--wait`: asked, with the flag that
    // makes it wait, and a yes takes the file as it is then.
    let hand_off = script(dir, "code", "exit 0");
    let backend = DefaultEditor::replying(&[true]);
    let notes = notes_with(&hand_off.display().to_string(), &backend);
    assert_eq!(notes.as_deref(), Ok("Bug fixes."));
    assert_eq!(backend.asked.take(), [format!(
        "Your editor returned right away. Done editing the file?\nTo make it wait, set \
         EDITOR=\"{} --wait\"",
        hand_off.display()
    )]);

    // A no reopens the editor; this one edits the file the second time.
    let marker = dir.join("opened-once");
    let second_time = script(
        dir,
        "second-time",
        &format!(
            "if [ -e '{0}' ]; then printf 'Second try.\\n' > \"$1\"; else touch '{0}'; fi",
            marker.display()
        ),
    );
    let backend = DefaultEditor::replying(&[false]);
    let notes = notes_with(&second_time.display().to_string(), &backend);
    assert_eq!(notes.as_deref(), Ok("Second try."));
    // An unknown editor gets no hint at a wait flag.
    assert_eq!(backend.asked.take(), ["Your editor returned right away. Done editing the file?"]);

    // Ctrl-C reaches the CLI too, which ignores it while the editor is open. Without that, this
    // test process would die here.
    let interrupt_parent =
        script(dir, "interrupt-parent", r#"kill -INT "$PPID"; printf 'Survived.\n' > "$1""#);
    let notes = notes_with(&interrupt_parent.display().to_string(), &quiet());
    assert_eq!(notes.as_deref(), Ok("Survived."));

    // The editor dying of Ctrl-C cancels the prompt, and the temp file goes with it.
    let seen = dir.join("seen");
    let die =
        script(dir, "die", &format!(r#"printf '%s' "$1" > '{}'; kill -INT $$"#, seen.display()));
    let cancelled = notes_with(&die.display().to_string(), &quiet());
    assert!(cancelled.as_ref().is_err_and(|err| err.contains("prompt cancelled")), "{cancelled:?}");
    let temp_file = PathBuf::from(fs::read_to_string(&seen).unwrap_or_default());
    assert!(
        temp_file.extension().is_some_and(|extension| extension == "txt") && !temp_file.exists(),
        "{}",
        temp_file.display()
    );

    // An editor exiting 130 on its own is a failure, not Ctrl-C: no `sh` reports it any more.
    let exit_130 = script(dir, "exit-130", "exit 130");
    let failed = notes_with(&exit_130.display().to_string(), &quiet()).err().unwrap_or_default();
    assert!(failed.contains("There was a problem with the editor"), "{failed}");

    let failed = notes_with("false", &quiet()).err().unwrap_or_default();
    assert!(failed.contains("There was a problem with the editor 'false'."), "{failed}");

    let missing = notes_with("clap-couture-no-such-editor", &quiet()).err().unwrap_or_default();
    assert!(
        missing.contains("Could not find the editor 'clap-couture-no-such-editor'."),
        "{missing}"
    );
    fs::remove_dir_all(&editors).ok();
}
