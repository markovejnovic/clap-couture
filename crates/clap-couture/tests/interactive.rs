//! `#[couture(prompt)]` asks for marked args the user left out, then parses the answers like
//! typed ones.
#![cfg(feature = "interactive")]

mod common;

use core::sync::atomic::{AtomicUsize, Ordering};

use clap::{
    ArgGroup, Args, Parser, Subcommand, ValueEnum, builder::ArgPredicate, error::ErrorKind,
};
use clap_couture::{Couture, CoutureParser as _};
use common::{Asked, Kind, Reply, ScriptedBackend};
use rstest::rstest;

/// How often `Plain`'s value parser ran, to count parses of argv.
static PLAIN_PARSES: AtomicUsize = AtomicUsize::new(0);
/// How often `Piped`'s value parser ran, to count parses of argv.
static PIPED_PARSES: AtomicUsize = AtomicUsize::new(0);

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

#[derive(Parser, Couture, Debug)]
#[command(group(ArgGroup::new("target").args(["host", "socket"])))]
struct Connect {
    #[arg(long)]
    host: Option<String>,

    #[arg(long, conflicts_with = "region")]
    local: bool,

    #[arg(long)]
    #[couture(prompt = "Which region?")]
    region: Option<String>,

    #[arg(long)]
    #[couture(prompt = "Which socket?")]
    socket: Option<String>,

    #[arg(long, conflicts_with = "region")]
    #[couture(prompt = "Which zone?")]
    zone: Option<String>,
}

/// Stands in for an `Args` type from another crate, which cannot implement `Couture`.
#[derive(Args, Debug)]
struct Verbosity {
    #[arg(long)]
    verbose: bool,
}

#[derive(Parser, Couture, Debug)]
struct Tool {
    #[command(flatten)]
    extra: Option<Extra>,

    #[command(flatten)]
    log: Verbosity,

    #[arg(long)]
    #[couture(prompt = "Which name?")]
    name: String,
}

#[derive(Args, Couture, Debug)]
struct Extra {
    #[arg(long)]
    #[couture(prompt = "Which tag?")]
    tag: Option<String>,
}

#[derive(Parser, Couture, Debug)]
struct Log {
    #[arg(
        long,
        default_value = "info",
        default_value_if("verbose", ArgPredicate::IsPresent, Some("debug"))
    )]
    #[couture(prompt = "Which level?")]
    level: String,

    #[arg(long)]
    pinned: bool,

    #[arg(
        long,
        default_value = "latest",
        default_value_if("pinned", ArgPredicate::IsPresent, None)
    )]
    #[couture(prompt = "Which tag?")]
    tag: Option<String>,

    #[arg(long)]
    verbose: bool,
}

#[derive(Parser, Couture, Debug)]
struct Paint {
    #[arg(long, value_enum)]
    #[couture(prompt = "Which color?")]
    color: Option<Color>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Color {
    Blue,
    Red,
}

#[derive(Parser, Couture, Debug)]
struct Plain {
    #[arg(long, value_parser = count_plain)]
    input: String,
}

#[derive(Parser, Couture, Debug)]
struct Piped {
    #[arg(long, value_parser = count_piped)]
    input: String,

    #[arg(long)]
    #[couture(prompt)]
    region: String,
}

#[expect(clippy::unnecessary_wraps, reason = "clap's value_parser takes a fallible fn")]
fn count_plain(value: &str) -> Result<String, String> {
    PLAIN_PARSES.fetch_add(1, Ordering::Relaxed);
    Ok(value.to_owned())
}

#[expect(clippy::unnecessary_wraps, reason = "clap's value_parser takes a fallible fn")]
fn count_piped(value: &str) -> Result<String, String> {
    PIPED_PARSES.fetch_add(1, Ordering::Relaxed);
    Ok(value.to_owned())
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
    let backend = ScriptedBackend::new([Reply::Text(Some("us"))]);
    let deploy = Deploy::couture_try_parse_from_with(&backend, deploy_argv("region"));
    assert_eq!(deploy.map(|deploy| deploy.region).ok().as_deref(), Some("us"));
    assert_eq!(backend.asked(), [asked(Kind::Text, "Region to deploy to", None, &[])]);
}

#[rstest]
fn leaves_an_optional_arg_unset_on_an_empty_answer() {
    let backend = ScriptedBackend::new([Reply::Text(None)]);
    let deploy = Deploy::couture_try_parse_from_with(&backend, deploy_argv("port"));
    assert_eq!(deploy.map(|deploy| deploy.port).ok(), Some(None));
}

#[rstest]
fn prefills_the_default() {
    let backend = ScriptedBackend::new([Reply::Text(Some("prod"))]);
    let deploy = Deploy::couture_try_parse_from_with(&backend, deploy_argv("env"));
    assert_eq!(deploy.map(|deploy| deploy.env).ok().as_deref(), Some("prod"));
    assert_eq!(backend.asked(), [asked(Kind::Text, "env", Some("staging"), &[])]);
}

#[rstest]
fn asks_yes_or_no_for_a_flag() {
    let backend = ScriptedBackend::new([Reply::Confirm(true)]);
    let deploy = Deploy::couture_try_parse_from_with(&backend, deploy_argv("follow"));
    assert_eq!(deploy.map(|deploy| deploy.follow).ok(), Some(true));
    assert_eq!(backend.asked(), [asked(
        Kind::Confirm,
        "Tail the logs afterwards",
        Some("false"),
        &[]
    )]);
}

#[rstest]
fn offers_the_possible_values_of_an_enum() {
    let backend = ScriptedBackend::new([Reply::Select(Some("canary"))]);
    let deploy = Deploy::couture_try_parse_from_with(&backend, deploy_argv("strategy"));
    assert_eq!(deploy.map(|deploy| deploy.strategy).ok(), Some(Strategy::Canary));
    assert_eq!(backend.asked(), [asked(Kind::Select, "Which strategy?", None, &[
        "canary", "rolling"
    ])]);
}

#[rstest]
fn never_asks_for_passed_args() {
    let backend = ScriptedBackend::new([]);
    let deploy = Deploy::couture_try_parse_from_with(&backend, deploy_argv(""));
    assert!(deploy.is_ok(), "{deploy:?}");
    assert_eq!(backend.asked(), []);
}

#[rstest]
fn asks_again_with_claps_message_when_an_answer_is_invalid() {
    let backend = ScriptedBackend::new([Reply::Text(Some("abc")), Reply::Text(Some("8080"))]);
    let deploy = Deploy::couture_try_parse_from_with(&backend, deploy_argv("port"));
    assert_eq!(deploy.map(|deploy| deploy.port).ok(), Some(Some(8080)));
    let errors: Vec<_> = backend.asked().into_iter().map(|asked| asked.error).collect();
    assert_eq!(errors, [
        None,
        Some("invalid value 'abc' for '--port <PORT>': invalid digit found in string".to_owned())
    ]);
}

#[rstest]
fn a_cancelled_prompt_is_an_io_error() {
    let backend = ScriptedBackend::new([Reply::Cancel]);
    let deploy = Deploy::couture_try_parse_from_with(&backend, deploy_argv("region"));
    assert_eq!(deploy.map_err(|err| err.kind()).err(), Some(ErrorKind::Io));
}

#[rstest]
fn without_a_terminal_a_missing_required_arg_is_claps_error() {
    let backend = ScriptedBackend::unavailable();
    let deploy = Deploy::couture_try_parse_from_with(&backend, deploy_argv("region"));
    assert_eq!(deploy.map_err(|err| err.kind()).err(), Some(ErrorKind::MissingRequiredArgument));
}

#[rstest]
#[case::struct_variant(&["orbit", "logs"], "Which app?")]
#[case::flattened_args(&["orbit", "login"], "Which user?")]
fn only_asks_on_the_subcommand_path_taken(#[case] argv: &[&str], #[case] question: &str) {
    let backend = ScriptedBackend::new([Reply::Text(Some("x"))]);
    let orbit = Orbit::couture_try_parse_from_with(&backend, argv);
    assert!(orbit.is_ok(), "{orbit:?}");
    assert_eq!(backend.asked(), [asked(Kind::Text, question, None, &[])]);
}

#[rstest]
fn asks_every_missing_mark_in_declaration_order() {
    let backend = ScriptedBackend::new([Reply::Text(Some("8080")), Reply::Text(Some("us"))]);
    let argv = ["deploy", "--env", "prod", "--follow", "--strategy", "rolling"];
    let deploy = Deploy::couture_try_parse_from_with(&backend, argv);
    assert_eq!(
        deploy.map(|deploy| (deploy.port, deploy.region)).ok(),
        Some((Some(8080), "us".to_owned()))
    );
    let questions: Vec<_> = backend.asked().into_iter().map(|asked| asked.question).collect();
    assert_eq!(questions, ["Which port?", "Region to deploy to"]);
}

#[rstest]
fn asks_for_a_missing_positional() {
    let backend = ScriptedBackend::new([Reply::Text(Some("main"))]);
    let argv = ["repo", "--profile", "p", "push", "--offset", "1"];
    let target = Repo::couture_try_parse_from_with(&backend, argv).map(|repo| {
        let RepoCmd::Push { target, .. } = repo.cmd;
        target
    });
    assert_eq!(target.ok().as_deref(), Some("main"));
}

#[rstest]
fn never_asks_for_a_global_arg_passed_after_the_subcommand() {
    let backend = ScriptedBackend::new([]);
    let argv = ["repo", "push", "x", "--offset", "1", "--profile", "p"];
    let repo = Repo::couture_try_parse_from_with(&backend, argv);
    assert_eq!(repo.map(|repo| repo.profile).ok().flatten().as_deref(), Some("p"));
    assert_eq!(backend.asked(), []);
}

#[rstest]
fn help_exits_before_asking() {
    let backend = ScriptedBackend::new([]);
    let repo = Repo::couture_try_parse_from_with(&backend, ["repo", "push", "--help"]);
    assert_eq!(repo.map_err(|err| err.kind()).err(), Some(ErrorKind::DisplayHelp));
    assert_eq!(backend.asked(), []);
}

#[rstest]
fn an_answer_may_start_with_a_dash() {
    let backend = ScriptedBackend::new([Reply::Text(Some("-5"))]);
    let argv = ["repo", "--profile", "p", "push", "x"];
    let offset = Repo::couture_try_parse_from_with(&backend, argv).map(|repo| {
        let RepoCmd::Push { offset, .. } = repo.cmd;
        offset
    });
    assert_eq!(offset.ok(), Some(Some(-5i32)));
}

#[rstest]
fn help_shows_the_marked_args_in_usage() {
    let backend = ScriptedBackend::new([]);
    let help = Deploy::couture_try_parse_from_with(&backend, ["deploy", "--help"])
        .map(drop)
        .map_err(|err| err.render().to_string());
    let help = help.err().unwrap_or_default();
    let usage = help.lines().skip_while(|line| !line.starts_with("Usage:")).nth(1);
    assert!(
        usage.is_some_and(|usage| usage.contains("--region <REGION>")),
        "usage lost the required mark:\n{help}"
    );
}

#[rstest]
fn an_unmarked_cli_parses_argv_once() {
    let backend = ScriptedBackend::new([]);
    let plain = Plain::couture_try_parse_from_with(&backend, ["plain", "--input", "-"]);
    assert!(plain.is_ok(), "{plain:?}");
    assert_eq!(PLAIN_PARSES.load(Ordering::Relaxed), 1);
}

#[rstest]
fn without_a_terminal_argv_is_parsed_once() {
    let backend = ScriptedBackend::unavailable();
    let argv = ["piped", "--input", "-", "--region", "eu"];
    let piped = Piped::couture_try_parse_from_with(&backend, argv);
    assert!(piped.is_ok(), "{piped:?}");
    assert_eq!(PIPED_PARSES.load(Ordering::Relaxed), 1);
}

#[rstest]
#[case::declared_by_the_passed_arg(&["connect", "--local", "--socket", "s", "--zone", "z"])]
#[case::declared_by_the_mark_and_an_exclusive_group(&["connect", "--region", "eu", "--host", "h"])]
fn never_asks_for_a_mark_that_conflicts_with_a_passed_arg(#[case] argv: &[&str]) {
    let backend = ScriptedBackend::new([]);
    let connect = Connect::couture_try_parse_from_with(&backend, argv);
    assert!(connect.is_ok(), "{connect:?}");
    assert_eq!(backend.asked(), []);
}

#[rstest]
fn never_asks_for_a_mark_that_conflicts_with_an_earlier_answer() {
    let backend = ScriptedBackend::new([Reply::Text(Some("eu"))]);
    let connect = Connect::couture_try_parse_from_with(&backend, ["connect", "--host", "h"]);
    assert_eq!(
        connect.map(|connect| (connect.region, connect.zone)).ok(),
        Some((Some("eu".to_owned()), None))
    );
    assert_eq!(backend.asked(), [asked(Kind::Text, "Which region?", None, &[])]);
}

#[rstest]
#[case::set_by_the_condition(&["log", "--verbose", "--tag", "v1"], "debug", Some("v1"))]
#[case::cleared_by_the_condition(&["log", "--pinned", "--level", "warn"], "warn", None)]
fn never_asks_for_a_mark_whose_conditional_default_fired(
    #[case] argv: &[&str],
    #[case] level: &str,
    #[case] tag: Option<&str>,
) {
    let backend = ScriptedBackend::new([]);
    let log = Log::couture_try_parse_from_with(&backend, argv);
    assert_eq!(
        log.map(|log| (log.level, log.tag)).ok(),
        Some((level.to_owned(), tag.map(str::to_owned)))
    );
    assert_eq!(backend.asked(), []);
}

#[rstest]
fn asks_through_optional_and_foreign_flattened_args() {
    let backend = ScriptedBackend::new([Reply::Text(Some("x")), Reply::Text(None)]);
    let tool = Tool::couture_try_parse_from_with(&backend, ["tool", "--verbose"]);
    assert!(tool.is_ok(), "{tool:?}");
    let questions: Vec<_> = backend.asked().into_iter().map(|asked| asked.question).collect();
    assert_eq!(questions, ["Which name?", "Which tag?"]);
}

#[rstest]
fn leaves_an_optional_enum_unset_when_none_is_picked() {
    let backend = ScriptedBackend::new([Reply::Select(None)]);
    let paint = Paint::couture_try_parse_from_with(&backend, ["paint"]);
    assert_eq!(paint.map(|paint| paint.color).ok(), Some(None));
}
