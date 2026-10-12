//! `#[couture(stdin(piped))]` takes piped stdin as the value of an arg the user left out or passed
//! `-`, and leaves any other stdin alone. Each case runs this binary again as a child, so its stdin
//! can be a pipe, a file or the null device.
#![cfg(feature = "interactive")]

#[expect(dead_code, reason = "only text replies are scripted here")]
mod common;

use core::{
    iter,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use std::{
    env, fs,
    io::{self, Write as _},
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    thread,
    time::Instant,
};

use clap::{Parser, Subcommand};
use clap_couture::{
    Couture, CoutureParser as _,
    interactive::{
        NonInteractive,
        secrecy::{ExposeSecret as _, SecretString},
    },
};
use common::{Kind, Reply, ScriptedBackend};
use rstest::rstest;

/// The argv [`child`] parses, its words split by `\x1f`.
const ARGV: &str = "CLAP_COUTURE_TEST_STDIN_ARGV";
/// The case [`child`] parses; unset outside a child.
const CASE: &str = "CLAP_COUTURE_TEST_STDIN_CASE";
/// What [`child`] prints before its result, to find it among the test harness's own output.
const RESULT: &str = "stdin-test-result: ";

#[derive(Parser, Couture, Debug)]
struct Post {
    #[arg(long)]
    #[couture(stdin(piped))]
    message: String,

    #[arg(long, conflicts_with = "message")]
    template: Option<String>,
}

#[derive(Parser, Couture, Debug)]
struct Codec {
    #[command(subcommand)]
    cmd: CodecCmd,
}

#[derive(Subcommand, Couture, Debug)]
enum CodecCmd {
    Decode {
        #[arg(long)]
        #[couture(stdin(piped))]
        input: String,
    },
    Encode {
        #[couture(stdin(piped))]
        input: String,
    },
    Version,
}

#[derive(Parser, Couture, Debug)]
struct Note {
    #[arg(long)]
    #[couture(stdin)]
    body: Option<String>,
}

#[derive(Parser, Couture, Debug)]
struct Draft {
    #[arg(long)]
    #[couture(prompt(dash), stdin)]
    message: Option<String>,
}

#[derive(Parser, Couture)]
struct DockerLogin {
    #[arg(long)]
    #[couture(prompt, secret, stdin)]
    password: SecretString,
}

#[derive(Parser, Couture)]
struct Login {
    /// Password.
    #[arg(long)]
    #[couture(prompt, secret, stdin(piped))]
    password: SecretString,

    #[arg(long)]
    #[couture(prompt)]
    user: String,
}

/// What the child's stdin is.
#[derive(Clone, Copy)]
enum Stdin {
    /// A file holding these bytes.
    File(&'static [u8]),
    /// The null device.
    Null,
    /// A pipe these bytes are written into, then closed.
    Piped(&'static [u8]),
    /// A pipe that stays open and empty until the child exits, so reading it would hang.
    PipedOpen,
}

/// Run by [`run`] in a child process: parse the case's CLI and print what came of it.
#[test]
#[ignore = "run in a child process by the other tests"]
fn child() {
    let Ok(case) = env::var(CASE) else { return };
    let words = env::var(ARGV).unwrap_or_default();
    let argv = iter::once("app").chain(words.split('\x1f').filter(|word| !word.is_empty()));
    let result = match case.as_str() {
        "post" => Post::couture_try_parse_from_with(NonInteractive, argv)
            .map(|post| format!("message={:?} template={:?}", post.message, post.template)),
        "codec" => {
            Codec::couture_try_parse_from_with(NonInteractive, argv).map(|codec| match codec.cmd {
                CodecCmd::Decode { input } => format!("decode {input:?}"),
                CodecCmd::Encode { input } => format!("encode {input:?}"),
                CodecCmd::Version => "version".to_owned(),
            })
        }
        "note" => Note::couture_try_parse_from_with(NonInteractive, argv)
            .map(|note| format!("body={:?}", note.body)),
        "draft" => {
            // Available, yet stdin is no terminal: a `-` reads it.
            let backend = ScriptedBackend::new([]);
            Draft::couture_try_parse_from_with(&backend, argv).map(|draft| {
                assert_eq!(backend.asked(), []);
                format!("message={:?}", draft.message)
            })
        }
        "docker-login" => DockerLogin::couture_try_parse_from_with(NonInteractive, argv)
            .map(|login| format!("password={}", login.password.expose_secret())),
        "login" => {
            // Available with stdin piped: the user is asked, the piped password is not.
            let backend = ScriptedBackend::new([Reply::Text(Some("ada"))]);
            Login::couture_try_parse_from_with(&backend, argv).map(|login| {
                let asked: Vec<_> = backend.asked().into_iter().map(|asked| asked.kind).collect();
                assert_eq!(asked, [Kind::Text]);
                format!("user={} password={}", login.user, login.password.expose_secret())
            })
        }
        _ => return,
    };
    let result = result.unwrap_or_else(|err| {
        let rendered = err.to_string();
        let kind = err.kind().map_or_else(|| "prompt".to_owned(), |kind| format!("{kind:?}"));
        format!("{kind}: {}", rendered.lines().next().unwrap_or_default())
    });
    writeln!(io::stdout(), "{RESULT}{result}").ok();
}

/// Run `case` in a child on `argv` with `stdin`, and return the result [`child`] printed.
fn run(case: &str, argv: &[&str], stdin: Stdin) -> Result<String, String> {
    let file = match stdin {
        Stdin::File(bytes) => Some(temp_file_with(bytes).map_err(|err| err.to_string())?),
        Stdin::Null | Stdin::Piped(_) | Stdin::PipedOpen => None,
    };
    let output = spawn(case, argv, stdin, file.as_ref()).and_then(|child| feed(child, stdin));
    if let Some(file) = file {
        fs::remove_file(file).ok();
    }
    let output = output.map_err(|err| err.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().find_map(|line| line.strip_prefix(RESULT)).map(str::to_owned).ok_or_else(|| {
        format!("no result in:\n{stdout}\n{}", String::from_utf8_lossy(&output.stderr))
    })
}

/// Write `stdin`'s bytes into `child`'s stdin pipe, then wait for it, killing it if it hangs.
fn feed(mut child: Child, stdin: Stdin) -> io::Result<Output> {
    let pipe = child.stdin.take();
    let held_open = match (stdin, pipe) {
        (Stdin::Piped(bytes), Some(mut pipe)) => {
            pipe.write_all(bytes)?;
            None
        }
        (_, pipe) => pipe,
    };
    let deadline = Instant::now().checked_add(Duration::from_secs(20));
    while child.try_wait()?.is_none() {
        if deadline.is_none_or(|deadline| Instant::now() > deadline) {
            child.kill().ok();
            return Err(io::Error::other("the child hung, reading a stdin it should leave alone"));
        }
        thread::sleep(Duration::from_millis(20));
    }
    drop(held_open);
    child.wait_with_output()
}

fn spawn(case: &str, argv: &[&str], stdin: Stdin, file: Option<&PathBuf>) -> io::Result<Child> {
    let child_stdin = match (stdin, file) {
        (Stdin::File(_), Some(file)) => Stdio::from(fs::File::open(file)?),
        (Stdin::Piped(_) | Stdin::PipedOpen, _) => Stdio::piped(),
        (Stdin::File(_) | Stdin::Null, _) => Stdio::null(),
    };
    Command::new(env::current_exe()?)
        .args(["child", "--exact", "--ignored", "--nocapture", "--quiet"])
        .env(CASE, case)
        .env(ARGV, argv.join("\x1f"))
        .stdin(child_stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// A temp file holding `bytes`, unique to this process and call.
fn temp_file_with(bytes: &[u8]) -> io::Result<PathBuf> {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let index = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = env::temp_dir().join(format!("clap-couture-stdin-{}-{index}", std::process::id()));
    fs::write(&path, bytes)?;
    Ok(path)
}

#[rstest]
#[case::with_its_line_ending_dropped(b"hello\n", "message=\"hello\" template=None")]
#[case::without_a_line_ending(b"hello", "message=\"hello\" template=None")]
#[case::with_a_crlf_dropped(b"hello\r\n", "message=\"hello\" template=None")]
#[case::keeping_inner_lines(b"a\n\nb\n\n", "message=\"a\\n\\nb\\n\" template=None")]
#[case::as_an_empty_line(b"\n", "message=\"\" template=None")]
fn a_left_out_arg_takes_piped_stdin(#[case] piped: &'static [u8], #[case] parsed: &str) {
    assert_eq!(run("post", &[], Stdin::Piped(piped)).as_deref(), Ok(parsed));
}

#[rstest]
#[case::nothing_piped(Stdin::Piped(b""))]
#[case::a_file(Stdin::File(b"hello\n"))]
#[case::the_null_device(Stdin::Null)]
fn stdin_that_is_not_piped_text_leaves_the_arg_out(#[case] stdin: Stdin) {
    let result = run("post", &[], stdin);
    assert!(
        result.as_ref().is_ok_and(|result| result.starts_with("MissingRequired")),
        "{result:?}"
    );
}

#[rstest]
#[case::the_arg_is_passed(&["--message", "typed"], "message=\"typed\" template=None")]
#[case::a_conflicting_arg_is_passed(&["--template", "t"], "MissingRequiredArgument")]
#[case::help_is_asked_for(&["--help"], "DisplayHelp")]
fn stdin_is_left_unread_when(#[case] argv: &[&str], #[case] parsed: &str) {
    let result = run("post", argv, Stdin::PipedOpen);
    assert!(result.as_ref().is_ok_and(|result| result.starts_with(parsed)), "{result:?}");
}

#[test]
fn stdin_that_is_not_utf8_is_an_error() {
    assert_eq!(
        run("post", &[], Stdin::Piped(b"\xff\xfe")).as_deref(),
        Ok("InvalidUtf8: error: --message <MESSAGE> got invalid UTF-8 from stdin")
    );
}

#[rstest]
#[case::by_flag(&["decode"], "decode \"abc\"")]
#[case::by_position(&["encode"], "encode \"abc\"")]
#[case::unless_passed(&["encode", "typed"], "encode \"typed\"")]
fn each_subcommand_takes_stdin_for_its_own_arg(#[case] argv: &[&str], #[case] parsed: &str) {
    assert_eq!(run("codec", argv, Stdin::Piped(b"abc\n")).as_deref(), Ok(parsed));
}

#[test]
fn a_subcommand_without_a_stdin_arg_leaves_stdin_alone() {
    assert_eq!(run("codec", &["version"], Stdin::PipedOpen).as_deref(), Ok("version"));
}

#[test]
fn a_piped_secret_is_not_asked_while_the_rest_is() {
    assert_eq!(
        run("login", &[], Stdin::Piped(b"hunter2\n")).as_deref(),
        Ok("user=ada password=hunter2")
    );
}

#[rstest]
#[case::piped(&["--body", "-"], Stdin::Piped(b"hello\n"), "body=Some(\"hello\")")]
#[case::piped_with_equals(&["--body=-"], Stdin::Piped(b"hello\n"), "body=Some(\"hello\")")]
#[case::from_a_file(&["--body", "-"], Stdin::File(b"hello\n"), "body=Some(\"hello\")")]
#[case::from_the_null_device(&["--body", "-"], Stdin::Null, "body=Some(\"\")")]
fn a_dash_reads_any_stdin(#[case] argv: &[&str], #[case] stdin: Stdin, #[case] parsed: &str) {
    assert_eq!(run("note", argv, stdin).as_deref(), Ok(parsed));
}

#[rstest]
#[case::left_out(&[], "body=None")]
#[case::passed(&["--body", "typed"], "body=Some(\"typed\")")]
fn without_a_dash_stdin_is_left_unread(#[case] argv: &[&str], #[case] parsed: &str) {
    assert_eq!(run("note", argv, Stdin::PipedOpen).as_deref(), Ok(parsed));
}

#[test]
fn a_dash_reads_a_secret_the_command_line_may_not_pass() {
    let result = run("docker-login", &["--password", "-"], Stdin::Piped(b"hunter2\n"));
    assert_eq!(result.as_deref(), Ok("password=hunter2"));
}

#[rstest]
#[case::piped(Stdin::Piped(b"hello\n"), "message=Some(\"hello\")")]
#[case::from_a_file(Stdin::File(b"hello\n"), "message=Some(\"hello\")")]
#[case::from_the_null_device(Stdin::Null, "message=Some(\"\")")]
fn a_dash_that_may_ask_reads_stdin_that_is_no_terminal(#[case] stdin: Stdin, #[case] parsed: &str) {
    assert_eq!(run("draft", &["--message", "-"], stdin).as_deref(), Ok(parsed));
}

#[rstest]
#[case::piped(&["--message", "-"], Stdin::Piped(b"hello\n"))]
#[case::piped_with_equals(&["--message=-"], Stdin::Piped(b"hello\n"))]
#[case::from_a_file(&["--message", "-"], Stdin::File(b"hello\n"))]
fn a_dash_reads_stdin_into_a_piped_arg_too(#[case] argv: &[&str], #[case] stdin: Stdin) {
    assert_eq!(run("post", argv, stdin).as_deref(), Ok("message=\"hello\" template=None"));
}

#[rstest]
fn stdin_past_the_limit_is_an_error() {
    let past_the_limit: &'static [u8] = Vec::leak(vec![b'a'; (16 << 20) + 1]);
    let result = run("note", &["--body", "-"], Stdin::File(past_the_limit));
    let error = "ValueValidation: error: --body <BODY> got more than 16 MiB from stdin";
    assert_eq!(result.as_deref(), Ok(error));
}
