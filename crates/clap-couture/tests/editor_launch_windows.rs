//! On Windows, `Backend::editor`'s default finds `$EDITOR` through `PATH` and `PATHEXT`, so a
//! `.cmd` editor like VS Code's `code` runs, and needs no `TERM`. The test sets process-wide
//! variables, so it lives alone in its own binary, in a single test.
#![cfg(all(feature = "interactive-editor", windows))]

use std::{env, fs, io};

use clap::Parser;
use clap_couture::{
    Couture, CoutureParser as _,
    interactive::{
        Backend, ConfirmPrompt, MultiSelectPrompt, PromptError, SecretPrompt, SelectPrompt,
        TextPrompt, secrecy::SecretString,
    },
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

    fn multi_select(&self, _prompt: &MultiSelectPrompt<'_>) -> Result<Vec<String>, PromptError> {
        Err(not_editor())
    }

    fn secret(&self, _prompt: &SecretPrompt<'_>) -> Result<Option<SecretString>, PromptError> {
        Err(not_editor())
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

/// The notes parsed with `editor` as `%EDITOR%`, or clap's message.
fn notes_with(editor: &str) -> Result<String, String> {
    // SAFETY: this binary holds a single test, so no other thread reads the environment.
    unsafe {
        env::set_var("EDITOR", editor);
    }
    Release::couture_try_parse_from_with(&DefaultEditor, ["release"])
        .map(|release| release.notes)
        .map_err(|err| err.to_string())
}

#[test]
fn the_default_backend_runs_a_cmd_editor_from_path() {
    let bin = env::temp_dir().join(format!("clap couture editor {}", std::process::id()));
    fs::create_dir_all(&bin).ok();
    // Overwrites the file it's given, header and all.
    fs::write(bin.join("fake-editor.cmd"), "@echo off\r\necho from %1> %2\r\n").ok();
    let path = env::var_os("PATH").unwrap_or_default();
    let path = env::join_paths(env::split_paths(&path).chain([bin.clone()])).unwrap_or_default();
    // SAFETY: as in `notes_with`.
    unsafe {
        env::set_var("PATH", path);
    }
    // SAFETY: as in `notes_with`.
    unsafe {
        env::remove_var("TERM");
    }
    // SAFETY: as in `notes_with`.
    unsafe {
        env::remove_var("VISUAL");
    }

    // Found through PATHEXT, with no TERM set. The editor's arg comes before the file.
    let from_path = notes_with("fake-editor the-editor");

    // A quoted full path with spaces, backslashes kept.
    let quoted = notes_with(&format!("\"{}\" quoted", bin.join("fake-editor.cmd").display()));

    fs::remove_dir_all(&bin).ok();
    assert_eq!(from_path.as_deref(), Ok("from the-editor"));
    assert_eq!(quoted.as_deref(), Ok("from quoted"));
}
