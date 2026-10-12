//! A cancelled prompt raises `SIGINT` under the app's own handling when it handles or ignores
//! it, and dies of it otherwise. The CLI runs in a child process of this binary, so that the
//! signal spares the runner.
#![cfg(all(feature = "interactive", unix))]

#[expect(dead_code, reason = "only a cancel is scripted")]
mod common;

use std::{os::unix::process::ExitStatusExt as _, process::Command};

use clap::Parser;
use clap_couture::{Couture, CoutureParser as _};
use common::{Reply, ScriptedBackend};
use rstest::rstest;

/// How [`cli_in_child`] handles `SIGINT` before its prompt is cancelled: `handle`, `ignore`, or
/// anything else for the default.
const HANDLING: &str = "CLAP_COUTURE_TEST_SIGINT";

/// What the app's handler writes to stderr.
const HANDLED: &str = "app handled SIGINT";

/// The status a shell reports for a death by `SIGINT`.
const INTERRUPTED: i32 = 130;

#[derive(Parser, Couture)]
struct Deploy {
    #[arg(long)]
    #[couture(prompt)]
    region: String,
}

/// The app's `SIGINT` handler: writes [`HANDLED`], async-signal-safely.
extern "C" fn handle(_signal: libc::c_int) {
    // SAFETY: `write` is async-signal-safe, and the buffer is live for the call.
    unsafe {
        libc::write(libc::STDERR_FILENO, HANDLED.as_ptr().cast(), HANDLED.len());
    }
}

/// Run by [`a_cancelled_prompt_respects_the_apps_handling`] in a child process of this binary:
/// sets up the app's `SIGINT` handling, then cancels the prompt and exits as `couture_parse`
/// would.
#[test]
#[ignore = "run in a child process by `a_cancelled_prompt_respects_the_apps_handling`"]
fn cli_in_child() {
    let Ok(handling) = std::env::var(HANDLING) else { return };
    let action = match handling.as_str() {
        #[expect(clippy::as_conversions, reason = "`signal` takes a handler as an address")]
        "handle" => (handle as extern "C" fn(libc::c_int) as *const ()).addr(),
        "ignore" => libc::SIG_IGN,
        _ => libc::SIG_DFL,
    };
    // SAFETY: `action` is a default, ignore, or an async-signal-safe handler.
    unsafe {
        libc::signal(libc::SIGINT, action);
    }
    if let Err(err) =
        Deploy::couture_try_parse_from_with(ScriptedBackend::new([Reply::Cancel]), ["deploy"])
    {
        err.exit();
    }
}

#[rstest]
#[case::handled("handle", None, Some(INTERRUPTED), true)]
#[case::ignored("ignore", None, Some(INTERRUPTED), false)]
#[case::by_default("default", Some(libc::SIGINT), None, false)]
fn a_cancelled_prompt_respects_the_apps_handling(
    #[case] handling: &str,
    #[case] died_of: Option<i32>,
    #[case] exited_with: Option<i32>,
    #[case] handled: bool,
) {
    let output = std::env::current_exe().and_then(|test_binary| {
        Command::new(test_binary)
            .args(["cli_in_child", "--exact", "--ignored", "--quiet"])
            .env(HANDLING, handling)
            .output()
    });
    let status = output.as_ref().ok().map(|output| output.status);
    let stderr = output
        .as_ref()
        .map(|output| String::from_utf8_lossy(&output.stderr).into_owned())
        .unwrap_or_default();

    assert_eq!(status.and_then(|status| status.signal()), died_of, "{stderr}");
    assert_eq!(status.and_then(|status| status.code()), exited_with, "{stderr}");
    assert_eq!(stderr.contains(HANDLED), handled, "{stderr}");
}
