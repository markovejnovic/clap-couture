//! `Backend::editor`'s default opens `$EDITOR` through `sh`, as git does. The test sets
//! process-wide variables, so it lives alone in its own binary, in a single test.
#![cfg(all(feature = "interactive-editor", unix))]

use std::io;

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

/// A terminal that only knows how to open the editor, through the default `Backend::editor`.
struct DefaultEditor;

impl Backend for DefaultEditor {
    fn confirm(&self, _prompt: &ConfirmPrompt<'_>) -> Result<bool, PromptError> {
        Err(not_editor())
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

/// The notes parsed with `editor` as `$EDITOR`, or clap's message.
fn notes_with(editor: &str) -> Result<String, String> {
    // SAFETY: this binary holds a single test, so no other thread reads the environment.
    unsafe {
        std::env::set_var("EDITOR", editor);
    }
    Release::couture_try_parse_from_with(&DefaultEditor, ["release"])
        .map(|release| release.notes)
        .map_err(|err| err.to_string())
}

#[test]
fn the_default_backend_runs_editor_like_git() {
    // SAFETY: as in `notes_with`.
    unsafe {
        std::env::set_var("TERM", "xterm");
    }
    // SAFETY: as in `notes_with`.
    unsafe {
        std::env::remove_var("VISUAL");
    }

    // Overwrites the file, header and all, through `sh`'s redirection and quoting.
    assert_eq!(
        notes_with("printf 'from \"%s\"\\n' 'the editor' >").as_deref(),
        Ok("from \"the editor\"")
    );

    // Appends after the header; the header comes back out, the default stays.
    assert_eq!(notes_with("printf 'More.\\n' >>").as_deref(), Ok("Bug fixes.\n\nMore."));

    // `:` is git's no-op editor: the pre-filled default is taken.
    assert_eq!(notes_with(":").as_deref(), Ok("Bug fixes."));

    // Ctrl-C reaches the CLI too, which ignores it while the editor is open.
    assert_eq!(notes_with("kill -INT $PPID; printf 'Survived.\\n' >").as_deref(), Ok("Survived."));

    // The editor dying of Ctrl-C cancels the prompt, and the temp file goes with it.
    let seen = std::env::temp_dir().join(format!("clap-couture-editor-{}", std::process::id()));
    let cancelled =
        notes_with(&format!("printf '%s' \"$1\" > '{}'; kill -INT $$; :", seen.display()));
    assert!(cancelled.as_ref().is_err_and(|err| err.contains("prompt cancelled")), "{cancelled:?}");
    let temp_file = std::path::PathBuf::from(std::fs::read_to_string(&seen).unwrap_or_default());
    std::fs::remove_file(&seen).ok();
    assert!(
        temp_file.extension().is_some_and(|extension| extension == "txt") && !temp_file.exists(),
        "{}",
        temp_file.display()
    );

    let failed = notes_with("false").err().unwrap_or_default();
    assert!(failed.contains("There was a problem with the editor 'false'."), "{failed}");
}
