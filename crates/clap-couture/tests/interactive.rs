//! `#[couture(prompt)]` asks for marked args the user left out, then parses the answers like
//! typed ones.
#![cfg(feature = "interactive")]

mod common;

use clap::{Args, Parser, Subcommand, ValueEnum, error::ErrorKind};
use clap_couture::{Couture, CoutureParser as _};
use common::{Asked, Kind, Reply, ScriptedPrompter};
use rstest::rstest;

#[derive(Parser, Couture, Debug)]
struct Deploy {
    #[arg(long, default_value = "staging")]
    #[couture(prompt)]
    env: String,

    /// Tail the logs afterwards.
    #[arg(long)]
    #[couture(prompt)]
    follow: bool,

    #[arg(long)]
    #[couture(prompt = "Which port?")]
    port: Option<u16>,

    /// Region to deploy to.
    #[arg(long)]
    #[couture(prompt)]
    region: String,

    #[arg(long, value_enum)]
    #[couture(prompt = "Which strategy?")]
    strategy: Strategy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Strategy {
    Canary,
    Rolling,
}

#[derive(Parser, Couture, Debug)]
struct Orbit {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture, Debug)]
enum Cmd {
    Login(Login),
    Logs {
        #[arg(long)]
        #[couture(prompt = "Which app?")]
        app: String,
    },
}

#[derive(Args, Couture, Debug)]
struct Login {
    #[command(flatten)]
    account: Account,
}

#[derive(Args, Couture, Debug)]
struct Account {
    #[arg(long)]
    #[couture(prompt = "Which user?")]
    user: String,
}

#[derive(Parser, Couture, Debug)]
struct Repo {
    #[command(subcommand)]
    cmd: RepoCmd,

    /// Profile to use.
    #[arg(long, global = true)]
    #[couture(prompt)]
    profile: Option<String>,
}

#[derive(Subcommand, Couture, Debug)]
enum RepoCmd {
    Push {
        #[arg(long, allow_negative_numbers = true)]
        #[couture(prompt = "Which offset?")]
        offset: Option<i32>,

        /// What to push.
        #[couture(prompt)]
        target: String,
    },
}

/// A full `Deploy` command line minus the flags that set `skip`.
fn deploy_argv(skip: &str) -> Vec<&'static str> {
    let flags: [(&str, &[&'static str]); 5] = [
        ("env", &["--env", "prod"]),
        ("follow", &["--follow"]),
        ("port", &["--port", "80"]),
        ("region", &["--region", "eu"]),
        ("strategy", &["--strategy", "rolling"]),
    ];
    let args = flags.iter().filter(|(name, _)| *name != skip).flat_map(|(_, args)| args.iter());
    core::iter::once("deploy").chain(args.copied()).collect()
}

fn asked(kind: Kind, question: &str, default: Option<&str>, options: &[&str]) -> Asked {
    Asked {
        default: default.map(str::to_owned),
        error: None,
        kind,
        options: options.iter().map(|option| (*option).to_owned()).collect(),
        question: question.to_owned(),
    }
}

#[rstest]
fn asks_for_a_missing_required_arg() {
    let prompter = ScriptedPrompter::new([Reply::Text(Some("us"))]);
    let deploy = Deploy::couture_try_parse_from_with(&prompter, deploy_argv("region"));
    assert_eq!(deploy.map(|deploy| deploy.region).ok().as_deref(), Some("us"));
    assert_eq!(prompter.asked(), [asked(Kind::Text, "Region to deploy to", None, &[])]);
}

#[rstest]
fn leaves_an_optional_arg_unset_on_an_empty_answer() {
    let prompter = ScriptedPrompter::new([Reply::Text(None)]);
    let deploy = Deploy::couture_try_parse_from_with(&prompter, deploy_argv("port"));
    assert_eq!(deploy.map(|deploy| deploy.port).ok(), Some(None));
}

#[rstest]
fn prefills_the_default() {
    let prompter = ScriptedPrompter::new([Reply::Text(Some("prod"))]);
    let deploy = Deploy::couture_try_parse_from_with(&prompter, deploy_argv("env"));
    assert_eq!(deploy.map(|deploy| deploy.env).ok().as_deref(), Some("prod"));
    assert_eq!(prompter.asked(), [asked(Kind::Text, "env", Some("staging"), &[])]);
}

#[rstest]
fn asks_yes_or_no_for_a_flag() {
    let prompter = ScriptedPrompter::new([Reply::Confirm(true)]);
    let deploy = Deploy::couture_try_parse_from_with(&prompter, deploy_argv("follow"));
    assert_eq!(deploy.map(|deploy| deploy.follow).ok(), Some(true));
    assert_eq!(prompter.asked(), [asked(
        Kind::Confirm,
        "Tail the logs afterwards",
        Some("false"),
        &[]
    )]);
}

#[rstest]
fn offers_the_possible_values_of_an_enum() {
    let prompter = ScriptedPrompter::new([Reply::Select("canary")]);
    let deploy = Deploy::couture_try_parse_from_with(&prompter, deploy_argv("strategy"));
    assert_eq!(deploy.map(|deploy| deploy.strategy).ok(), Some(Strategy::Canary));
    assert_eq!(prompter.asked(), [asked(Kind::Select, "Which strategy?", None, &[
        "canary", "rolling"
    ])]);
}

#[rstest]
fn never_asks_for_passed_args() {
    let prompter = ScriptedPrompter::new([]);
    let deploy = Deploy::couture_try_parse_from_with(&prompter, deploy_argv(""));
    assert!(deploy.is_ok(), "{deploy:?}");
    assert_eq!(prompter.asked(), []);
}

#[rstest]
fn asks_again_with_claps_message_when_an_answer_is_invalid() {
    let prompter = ScriptedPrompter::new([Reply::Text(Some("abc")), Reply::Text(Some("8080"))]);
    let deploy = Deploy::couture_try_parse_from_with(&prompter, deploy_argv("port"));
    assert_eq!(deploy.map(|deploy| deploy.port).ok(), Some(Some(8080)));
    let errors: Vec<_> = prompter.asked().into_iter().map(|asked| asked.error).collect();
    assert_eq!(errors, [
        None,
        Some("invalid value 'abc' for '--port <PORT>': invalid digit found in string".to_owned())
    ]);
}

#[rstest]
fn a_cancelled_prompt_is_an_io_error() {
    let prompter = ScriptedPrompter::new([Reply::Cancel]);
    let deploy = Deploy::couture_try_parse_from_with(&prompter, deploy_argv("region"));
    assert_eq!(deploy.map_err(|err| err.kind()).err(), Some(ErrorKind::Io));
}

#[rstest]
fn without_a_terminal_a_missing_required_arg_is_claps_error() {
    let prompter = ScriptedPrompter::unavailable();
    let deploy = Deploy::couture_try_parse_from_with(&prompter, deploy_argv("region"));
    assert_eq!(deploy.map_err(|err| err.kind()).err(), Some(ErrorKind::MissingRequiredArgument));
}

#[rstest]
#[case::struct_variant(&["orbit", "logs"], "Which app?")]
#[case::flattened_args(&["orbit", "login"], "Which user?")]
fn only_asks_on_the_subcommand_path_taken(#[case] argv: &[&str], #[case] question: &str) {
    let prompter = ScriptedPrompter::new([Reply::Text(Some("x"))]);
    let orbit = Orbit::couture_try_parse_from_with(&prompter, argv);
    assert!(orbit.is_ok(), "{orbit:?}");
    assert_eq!(prompter.asked(), [asked(Kind::Text, question, None, &[])]);
}

#[rstest]
fn asks_every_missing_mark_in_declaration_order() {
    let prompter = ScriptedPrompter::new([Reply::Text(Some("8080")), Reply::Text(Some("us"))]);
    let argv = ["deploy", "--env", "prod", "--follow", "--strategy", "rolling"];
    let deploy = Deploy::couture_try_parse_from_with(&prompter, argv);
    assert_eq!(
        deploy.map(|deploy| (deploy.port, deploy.region)).ok(),
        Some((Some(8080), "us".to_owned()))
    );
    let questions: Vec<_> = prompter.asked().into_iter().map(|asked| asked.question).collect();
    assert_eq!(questions, ["Which port?", "Region to deploy to"]);
}

#[rstest]
fn asks_for_a_missing_positional() {
    let prompter = ScriptedPrompter::new([Reply::Text(Some("main"))]);
    let argv = ["repo", "--profile", "p", "push", "--offset", "1"];
    let target = Repo::couture_try_parse_from_with(&prompter, argv).map(|repo| {
        let RepoCmd::Push { target, .. } = repo.cmd;
        target
    });
    assert_eq!(target.ok().as_deref(), Some("main"));
}

#[rstest]
fn never_asks_for_a_global_arg_passed_after_the_subcommand() {
    let prompter = ScriptedPrompter::new([]);
    let argv = ["repo", "push", "x", "--offset", "1", "--profile", "p"];
    let repo = Repo::couture_try_parse_from_with(&prompter, argv);
    assert_eq!(repo.map(|repo| repo.profile).ok().flatten().as_deref(), Some("p"));
    assert_eq!(prompter.asked(), []);
}

#[rstest]
fn help_exits_before_asking() {
    let prompter = ScriptedPrompter::new([]);
    let repo = Repo::couture_try_parse_from_with(&prompter, ["repo", "push", "--help"]);
    assert_eq!(repo.map_err(|err| err.kind()).err(), Some(ErrorKind::DisplayHelp));
    assert_eq!(prompter.asked(), []);
}

#[rstest]
fn an_answer_may_start_with_a_dash() {
    let prompter = ScriptedPrompter::new([Reply::Text(Some("-5"))]);
    let argv = ["repo", "--profile", "p", "push", "x"];
    let offset = Repo::couture_try_parse_from_with(&prompter, argv).map(|repo| {
        let RepoCmd::Push { offset, .. } = repo.cmd;
        offset
    });
    assert_eq!(offset.ok(), Some(Some(-5i32)));
}
