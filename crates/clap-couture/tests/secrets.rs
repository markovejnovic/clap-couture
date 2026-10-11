//! `#[couture(prompt, secret)]` asks masked, keeps the answer out of clap, and refuses the command
//! line.
#![cfg(feature = "interactive")]

#[expect(dead_code, reason = "only tests/interactive.rs scripts confirm, select and text replies")]
mod common;

use clap::{Parser, Subcommand, error::ErrorKind};
use clap_couture::{
    Couture, CoutureParser as _, ParseError,
    interactive::{
        NonInteractive,
        secrecy::{ExposeSecret as _, SecretString},
    },
};
use common::{Asked, Kind, Reply, ScriptedBackend};
use rstest::rstest;

/// The secret is asked first, so the replicas' trial parse carries the secret's placeholder.
#[derive(Parser, Couture)]
struct Deploy {
    #[arg(long)]
    #[couture(prompt = "Which token?", secret)]
    api_token: SecretString,

    /// How many replicas to run.
    #[arg(long)]
    #[couture(prompt)]
    replicas: u8,
}

/// A root secret every subcommand takes, so the subcommand's parse carries the placeholder.
/// Optional, as clap refuses a required global arg.
#[derive(Parser, Couture)]
struct GlobalLogin {
    #[command(subcommand)]
    cmd: GlobalLoginCmd,

    #[arg(long, global = true)]
    #[couture(prompt = "Which token?", secret)]
    token: Option<SecretString>,
}

#[derive(Subcommand, Couture)]
enum GlobalLoginCmd {
    Push,
}

#[derive(Parser, Couture)]
struct Login {
    #[arg(long)]
    #[couture(prompt = "Which token?", secret)]
    token: SecretString,
}

#[derive(Parser, Couture)]
struct OptionalLogin {
    #[arg(long)]
    #[couture(prompt = "Which token?", secret)]
    token: Option<SecretString>,
}

#[derive(Parser, Couture)]
struct Registry {
    #[command(subcommand)]
    cmd: RegistryCmd,
}

#[derive(Subcommand, Couture)]
enum RegistryCmd {
    Login {
        #[arg(long)]
        #[couture(prompt = "Which token?", secret)]
        token: SecretString,
    },
}

#[derive(Parser, Couture)]
struct ScriptedLogin {
    #[arg(long)]
    #[couture(prompt = "Which token?", secret(allow_argv))]
    token: SecretString,
}

fn token_asked_masked() -> Asked {
    Asked {
        defaults: Vec::new(),
        error: None,
        hint: None,
        initial: None,
        kind: Kind::Secret,
        options: Vec::new(),
        question: "Which token?".to_owned(),
        selection_required: None,
    }
}

#[rstest]
fn the_derive_records_the_secret_and_its_argv_policy() {
    let allow_argv = |tree: &'static clap_couture::__private::PromptedArgTree| {
        tree.prompted_args()
            .first()
            .and_then(|arg| arg.attr.secret.as_ref())
            .map(clap_couture::__private::SecretAttr::allow_argv)
    };
    assert_eq!(allow_argv(&Login::PROMPTED_ARGS), Some(false));
    assert_eq!(allow_argv(&OptionalLogin::PROMPTED_ARGS), Some(false));
    assert_eq!(allow_argv(&ScriptedLogin::PROMPTED_ARGS), Some(true));
}

#[rstest]
fn the_command_line_is_refused_before_anything_is_asked() {
    let backend = ScriptedBackend::new([]);
    let login = Login::couture_try_parse_from_with(&backend, ["app", "--token", "x"]);
    assert_eq!(
        login.map(drop).map_err(|err| err.kind()).err(),
        Some(Some(ErrorKind::ValueValidation))
    );
    assert_eq!(backend.asked(), []);
}

#[rstest]
fn the_command_line_is_refused_without_a_terminal() {
    let login = Login::couture_try_parse_from_with(NonInteractive, ["app", "--token", "x"]);
    assert_eq!(
        login.map(drop).map_err(|err| err.kind()).err(),
        Some(Some(ErrorKind::ValueValidation))
    );
}

#[rstest]
fn an_optional_secret_is_refused_on_the_command_line() {
    let login = OptionalLogin::couture_try_parse_from_with(NonInteractive, ["app", "--token", "x"]);
    assert_eq!(
        login.map(drop).map_err(|err| err.kind()).err(),
        Some(Some(ErrorKind::ValueValidation))
    );
}

#[rstest]
fn the_refusal_names_the_arg() {
    let login = Login::couture_try_parse_from_with(NonInteractive, ["app", "--token", "x"]);
    let message = login.map(drop).map_err(|err| err.to_string()).err().unwrap_or_default();
    assert!(message.contains("--token <TOKEN> is a secret"), "{message}");
}

#[rstest]
fn couture_command_alone_refuses_the_command_line() {
    let matches = Login::couture_command().try_get_matches_from(["app", "--token", "x"]);
    assert_eq!(matches.map(drop).map_err(|err| err.kind()).err(), Some(ErrorKind::ValueValidation));
}

#[rstest]
fn allow_argv_takes_the_command_line() {
    let login = ScriptedLogin::couture_try_parse_from_with(NonInteractive, ["app", "--token", "x"]);
    let token = login.map(|login| login.token.expose_secret().to_owned());
    assert_eq!(token.ok().as_deref(), Some("x"));
}

#[rstest]
fn a_missing_secret_is_asked_masked() {
    let backend = ScriptedBackend::new([Reply::Secret(Some("hunter2"))]);
    let login = Login::couture_try_parse_from_with(&backend, ["app"]);
    let token = login.map(|login| login.token.expose_secret().to_owned());
    assert_eq!(token.ok().as_deref(), Some("hunter2"));
    assert_eq!(backend.asked(), [token_asked_masked()]);
}

#[rstest]
fn an_optional_secret_answered_with_nothing_is_none() {
    let backend = ScriptedBackend::new([Reply::Secret(None)]);
    let login = OptionalLogin::couture_try_parse_from_with(&backend, ["app"]);
    assert_eq!(login.map(|login| login.token.is_none()).ok(), Some(true));
}

#[rstest]
fn answers_after_a_secret_still_parse() {
    let backend = ScriptedBackend::new([Reply::Secret(Some("hunter2")), Reply::Text(Some("3"))]);
    let deploy = Deploy::couture_try_parse_from_with(&backend, ["app"]);
    let deploy =
        deploy.map(|deploy| (deploy.api_token.expose_secret().to_owned(), deploy.replicas));
    assert_eq!(deploy.ok(), Some(("hunter2".to_owned(), 3)));
}

#[rstest]
fn a_rejected_answer_after_a_secret_is_asked_again_and_reported_without_its_placeholder() {
    let backend = ScriptedBackend::new([
        Reply::Secret(Some("hunter2")),
        Reply::Text(Some("lots")),
        Reply::Text(Some("3")),
    ]);
    let deploy = Deploy::couture_try_parse_from_with(&backend, ["app"]);
    assert_eq!(deploy.map(|deploy| deploy.replicas).ok(), Some(3));
    let reported: Vec<String> =
        backend.asked().into_iter().filter_map(|asked| asked.error).collect();
    assert_eq!(reported.len(), 1, "{reported:?}");
    assert!(
        reported.iter().all(|error| !error.contains("couture-secret") && !error.contains('\0')),
        "{reported:?}"
    );
}

#[rstest]
fn a_cancelled_secret_is_an_io_error() {
    let backend = ScriptedBackend::new([Reply::Cancel]);
    let login = Login::couture_try_parse_from_with(&backend, ["app"]);
    assert!(matches!(login, Err(ParseError::Cancelled(_))));
}

#[rstest]
fn a_secret_on_a_subcommand_is_asked_masked() {
    let backend = ScriptedBackend::new([Reply::Secret(Some("hunter2"))]);
    let registry = Registry::couture_try_parse_from_with(&backend, ["app", "login"]);
    let token = registry.map(|registry| match registry.cmd {
        RegistryCmd::Login { token } => token.expose_secret().to_owned(),
    });
    assert_eq!(token.ok().as_deref(), Some("hunter2"));
    assert_eq!(backend.asked(), [token_asked_masked()]);
}

#[rstest]
fn a_secret_on_a_subcommand_is_refused_on_the_command_line() {
    let backend = ScriptedBackend::new([]);
    let registry =
        Registry::couture_try_parse_from_with(&backend, ["app", "login", "--token", "x"]);
    assert_eq!(
        registry.map(drop).map_err(|err| err.kind()).err(),
        Some(Some(ErrorKind::ValueValidation))
    );
    assert_eq!(backend.asked(), []);
}

#[rstest]
fn a_global_secret_is_asked_through_a_subcommand() {
    let backend = ScriptedBackend::new([Reply::Secret(Some("hunter2"))]);
    let login = GlobalLogin::couture_try_parse_from_with(&backend, ["app", "push"]);
    let token = login.map(|login| match login.cmd {
        GlobalLoginCmd::Push => login.token.map(|token| token.expose_secret().to_owned()),
    });
    assert_eq!(token.ok().flatten().as_deref(), Some("hunter2"));
    assert_eq!(backend.asked(), [token_asked_masked()]);
}
