//! `SIGTERM` while the editor is open ends the editor too, removes the draft, and still kills the
//! CLI. The CLI runs in a child process of this binary, so that the signal spares the runner.
#![cfg(all(feature = "interactive-editor", unix))]

#[expect(dead_code, reason = "no reply is scripted: the editor never returns")]
mod common;

use core::time::Duration;
use std::{
    fs,
    os::unix::{fs::PermissionsExt as _, process::ExitStatusExt as _},
    path::PathBuf,
    process::Command,
    time::Instant,
};

use clap::Parser;
use clap_couture::{Couture, CoutureParser as _};
use common::ScriptedBackend;

/// Where the editor writes the path of the file it was given, for [`cli_in_child`] to run.
const SEEN: &str = "CLAP_COUTURE_TEST_SEEN";

/// The signal `kill -TERM` sends.
const SIGTERM: i32 = 15;

/// Longer than the editor ever runs once `SIGTERM` reaches it.
const EDITOR_SLEEP: Duration = Duration::from_secs(30);

#[derive(Parser, Couture)]
struct Release {
    /// Release notes.
    #[arg(long)]
    #[couture(prompt, editor)]
    notes: String,
}

/// Run by [`terminating_the_cli_removes_the_draft`] in a child process of this binary: prompts
/// in an editor that sends the CLI `SIGTERM`.
#[test]
#[ignore = "run in a child process by `terminating_the_cli_removes_the_draft`"]
fn cli_in_child() {
    let Some(seen) = std::env::var_os(SEEN) else { return };
    let editor = PathBuf::from(&seen).with_extension("sh");
    let body = format!(
        "#!/bin/sh\nprintf '%s' \"$1\" > '{}'\nkill -TERM \"$PPID\"\nexec sleep {}\n",
        PathBuf::from(&seen).display(),
        EDITOR_SLEEP.as_secs()
    );
    fs::write(&editor, body).ok();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o755)).ok();
    // SAFETY: the only test this process runs, so no other thread reads the environment.
    unsafe {
        std::env::set_var("EDITOR", &editor);
    }
    // SAFETY: as above.
    unsafe {
        std::env::remove_var("VISUAL");
    }
    drop(Release::couture_try_parse_from_with(ScriptedBackend::new([]), ["release"]));
}

#[test]
fn terminating_the_cli_removes_the_draft() {
    let dir = std::env::temp_dir().join(format!("clap-couture-terminated-{}", std::process::id()));
    fs::create_dir_all(&dir).ok();
    let seen = dir.join("seen");
    let started = Instant::now();
    let status = std::env::current_exe().and_then(|test_binary| {
        Command::new(test_binary)
            .args(["cli_in_child", "--exact", "--ignored", "--quiet"])
            .env(SEEN, &seen)
            .output()
    });
    let elapsed = started.elapsed();
    let signal = status.ok().and_then(|output| output.status.signal());
    let draft = PathBuf::from(fs::read_to_string(&seen).unwrap_or_default());
    let draft_dir = draft.parent().map(PathBuf::from).unwrap_or_default();
    let left_behind = draft_dir.exists();
    fs::remove_dir_all(&draft_dir).ok();
    fs::remove_dir_all(&dir).ok();

    assert_eq!(signal, Some(SIGTERM), "the CLI still dies of the signal");
    assert!(draft.is_absolute(), "the editor ran: {}", draft.display());
    assert!(!left_behind, "the draft was left in {}", draft_dir.display());
    assert!(elapsed < EDITOR_SLEEP / 2, "the editor outlived the signal: {elapsed:?}");
}
