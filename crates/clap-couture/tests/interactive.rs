//! `#[couture(prompt)]` asks for prompted args the user left out, then parses the answers like
//! typed ones.
#![cfg(feature = "interactive")]

#[expect(
    dead_code,
    reason = "secret replies script tests/secrets.rs; nothing here asks for a secret"
)]
mod common;

use core::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};

use clap::{
    ArgGroup, Args, Parser, Subcommand, ValueEnum, builder::ArgPredicate, error::ErrorKind,
};
use clap_couture::{Couture, CoutureParser as _, ParseError};
use common::{Asked, Kind, Reply, ScriptedBackend};
use rstest::rstest;

/// How often `Plain`'s value parser ran, to count parses of argv.
static PLAIN_PARSES: AtomicUsize = AtomicUsize::new(0);
/// How often `Piped`'s value parser ran, to count parses of argv.
static PIPED_PARSES: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// What `Localized` asks; thread-local, as tests run in parallel.
    static REGION_QUESTION: Cell<&'static str> = const { Cell::new("Which region?") };
}

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

#[derive(Parser, Couture, Debug)]
#[couture(messages = { none = "(keine)" })]
struct NoneOption {
    #[arg(long, value_enum)]
    #[couture(prompt = "Which color?")]
    color: Option<Color>,
}

#[derive(Parser, Couture, Debug)]
struct Localized {
    #[arg(long)]
    #[couture(prompt = REGION_QUESTION.get())]
    region: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Env {
    Dev,
    Prod,
    Stage,
}

#[derive(Parser, Couture, Debug)]
struct Release {
    #[arg(long, value_enum)]
    #[couture(prompt = "Which envs?")]
    envs: Vec<Env>,
}

#[derive(Parser, Couture, Debug)]
struct Pinned {
    #[arg(long, value_enum, default_values = ["dev", "stage"])]
    #[couture(prompt = "Which envs?")]
    envs: Vec<Env>,
}

#[derive(Parser, Couture, Debug)]
struct Narrowed {
    #[arg(long, value_enum, required = true)]
    #[couture(prompt = "Which envs?")]
    envs: Vec<Env>,
}

#[derive(Parser, Couture, Debug)]
#[couture(messages = { selection_required = "Mindestens eine Auswahl" })]
struct NarrowedGerman {
    #[arg(long, value_enum, required = true)]
    #[couture(prompt = "Which envs?")]
    envs: Vec<Env>,
}

#[derive(Parser, Couture, Debug)]
struct Maybe {
    #[arg(long, value_enum)]
    #[couture(prompt = "Which envs?")]
    envs: Option<Vec<Env>>,

    #[arg(long)]
    #[couture(prompt = "Which tags?")]
    tags: Option<Vec<String>>,
}

#[derive(Parser, Couture, Debug)]
struct Serve {
    #[arg(long, required = true)]
    #[couture(prompt = "Which ports?")]
    ports: Vec<u16>,
}

#[derive(Parser, Couture, Debug)]
struct Labels {
    #[arg(long, default_values = ["a", "b"])]
    #[couture(prompt = "Which labels?")]
    labels: Vec<String>,
}

#[derive(Parser, Couture, Debug)]
struct Split {
    #[arg(long, value_delimiter = ',', default_value = "a,b")]
    #[couture(prompt = "Which labels?")]
    labels: Vec<String>,
}

#[derive(Parser, Couture, Debug)]
struct Delete {
    #[arg(long, conflicts_with = "force")]
    dry_run: bool,

    #[arg(long)]
    #[couture(confirm = "Delete {name} from {regions}?")]
    force: bool,

    #[arg(long)]
    #[couture(prompt = "Which app?")]
    name: String,

    #[arg(long)]
    regions: Vec<String>,
}

/// `Delete` minus its conflicts and prompts: the plainest command a confirmation guards.
#[derive(Parser, Couture, Debug)]
struct Wipe {
    #[arg(long)]
    #[couture(confirm = "Wipe {name}?")]
    force: bool,

    #[arg(long)]
    name: String,
}

#[derive(Parser, Couture, Debug)]
struct Braces {
    #[arg(long)]
    name: Option<String>,

    #[arg(long)]
    #[couture(confirm = "{{name}} {name} {nope} {")]
    yes: bool,
}

#[derive(Parser, Couture, Debug)]
struct Store {
    #[command(subcommand)]
    cmd: StoreCmd,
}

#[derive(Subcommand, Couture, Debug)]
enum StoreCmd {
    Drop {
        bucket: String,

        /// Drop {bucket} for good?
        #[arg(long)]
        #[couture(confirm)]
        yes: bool,
    },
    List,
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
        defaults: default.map(str::to_owned).into_iter().collect(),
        error: None,
        hint: None,
        initial: None,
        kind,
        options: options.iter().map(|option| (*option).to_owned()).collect(),
        question: question.to_owned(),
        selection_required: None,
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
    assert!(matches!(deploy, Err(ParseError::Cancelled(_))));
}

#[rstest]
fn without_a_terminal_a_missing_required_arg_is_claps_error() {
    let backend = ScriptedBackend::unavailable();
    let deploy = Deploy::couture_try_parse_from_with(&backend, deploy_argv("region"));
    assert_eq!(
        deploy.map_err(|err| err.kind()).err(),
        Some(Some(ErrorKind::MissingRequiredArgument))
    );
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
fn asks_every_missing_prompted_arg_in_declaration_order() {
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
        let RepoCmd::Push { offset: _, target } = repo.cmd;
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
    assert_eq!(repo.map_err(|err| err.kind()).err(), Some(Some(ErrorKind::DisplayHelp)));
    assert_eq!(backend.asked(), []);
}

#[rstest]
fn an_answer_may_start_with_a_dash() {
    let backend = ScriptedBackend::new([Reply::Text(Some("-5"))]);
    let argv = ["repo", "--profile", "p", "push", "x"];
    let offset = Repo::couture_try_parse_from_with(&backend, argv).map(|repo| {
        let RepoCmd::Push { offset, target: _ } = repo.cmd;
        offset
    });
    assert_eq!(offset.ok(), Some(Some(-5i32)));
}

#[rstest]
fn help_shows_the_prompted_args_in_usage() {
    let backend = ScriptedBackend::new([]);
    let help = Deploy::couture_try_parse_from_with(&backend, ["deploy", "--help"])
        .map(drop)
        .map_err(|err| err.to_string());
    let help = help.err().unwrap_or_default();
    let usage = help.lines().skip_while(|line| !line.starts_with("Usage:")).nth(1);
    assert!(
        usage.is_some_and(|usage| usage.contains("--region <REGION>")),
        "usage lost the required prompted arg:\n{help}"
    );
}

#[rstest]
fn a_cli_without_prompts_parses_argv_once() {
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
#[case::declared_by_the_prompted_arg_and_an_exclusive_group(
    &["connect", "--region", "eu", "--host", "h"]
)]
fn never_asks_for_a_prompted_arg_that_conflicts_with_a_passed_arg(#[case] argv: &[&str]) {
    let backend = ScriptedBackend::new([]);
    let connect = Connect::couture_try_parse_from_with(&backend, argv);
    assert!(connect.is_ok(), "{connect:?}");
    assert_eq!(backend.asked(), []);
}

#[rstest]
fn never_asks_for_a_prompted_arg_that_conflicts_with_an_earlier_answer() {
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
fn never_asks_for_a_prompted_arg_whose_conditional_default_fired(
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

#[rstest]
fn a_prompt_expression_is_evaluated_when_asked() {
    REGION_QUESTION.set("Welche Region?");
    let backend = ScriptedBackend::new([Reply::Text(Some("eu"))]);
    let localized = Localized::couture_try_parse_from_with(&backend, ["deploy"]);
    assert_eq!(localized.map(|localized| localized.region).ok().as_deref(), Some("eu"));
    assert_eq!(backend.asked(), [asked(Kind::Text, "Welche Region?", None, &[])]);
}

#[rstest]
fn an_optional_select_lists_the_none_message() {
    let backend = ScriptedBackend::new([Reply::Select(None)]);
    let cli = NoneOption::couture_try_parse_from_with(&backend, ["paint"]);
    assert_eq!(cli.map(|cli| cli.color).ok(), Some(None));
    assert_eq!(backend.asked(), [asked(Kind::Select, "Which color?", None, &[
        "blue", "red", "(keine)"
    ])]);
}

fn multi_select(defaults: &[&str]) -> Asked {
    Asked {
        defaults: defaults.iter().map(|default| (*default).to_owned()).collect(),
        error: None,
        hint: None,
        initial: None,
        kind: Kind::MultiSelect,
        options: ["dev", "prod", "stage"].map(str::to_owned).into(),
        question: "Which envs?".to_owned(),
        selection_required: None,
    }
}

fn text(question: &str, initial: Option<&str>, hint: bool) -> Asked {
    Asked {
        defaults: Vec::new(),
        error: None,
        hint: hint.then(|| "(<enter> to finish)".to_owned()),
        initial: initial.map(str::to_owned),
        kind: Kind::Text,
        options: Vec::new(),
        question: question.to_owned(),
        selection_required: None,
    }
}

#[rstest]
fn offers_a_multi_select_for_possible_values() {
    let backend = ScriptedBackend::new([Reply::MultiSelect(&["dev", "stage"])]);
    let release = Release::couture_try_parse_from_with(&backend, ["release"]);
    assert_eq!(release.map(|release| release.envs).ok(), Some(Vec::from([Env::Dev, Env::Stage])));
    assert_eq!(backend.asked(), [multi_select(&[])]);
}

#[rstest]
fn pre_picks_every_default() {
    let backend = ScriptedBackend::new([Reply::MultiSelect(&["prod"])]);
    let pinned = Pinned::couture_try_parse_from_with(&backend, ["pinned"]);
    assert_eq!(pinned.map(|pinned| pinned.envs).ok(), Some(Vec::from([Env::Prod])));
    assert_eq!(backend.asked(), [multi_select(&["dev", "stage"])]);
}

#[rstest]
fn picking_nothing_drops_the_defaults() {
    let backend = ScriptedBackend::new([Reply::MultiSelect(&[])]);
    let pinned = Pinned::couture_try_parse_from_with(&backend, ["pinned"]);
    assert_eq!(pinned.map(|pinned| pinned.envs).ok(), Some(Vec::new()));
}

#[rstest]
fn a_required_multi_select_is_not_optional() {
    let backend = ScriptedBackend::new([Reply::MultiSelect(&["prod"])]);
    let narrowed = Narrowed::couture_try_parse_from_with(&backend, ["narrowed"]);
    assert_eq!(narrowed.map(|narrowed| narrowed.envs).ok(), Some(Vec::from([Env::Prod])));
}

#[rstest]
fn a_required_multi_select_says_a_selection_is_required() {
    let backend = ScriptedBackend::new([Reply::MultiSelect(&["prod"])]);
    let narrowed = Narrowed::couture_try_parse_from_with(&backend, ["narrowed"]);
    assert!(narrowed.is_ok(), "{narrowed:?}");
    let messages: Vec<_> =
        backend.asked().into_iter().map(|asked| asked.selection_required).collect();
    assert_eq!(messages, [Some("Select at least one option".to_owned())]);
}

#[rstest]
fn the_selection_required_message_can_be_set() {
    let backend = ScriptedBackend::new([Reply::MultiSelect(&["prod"])]);
    let narrowed = NarrowedGerman::couture_try_parse_from_with(&backend, ["narrowed"]);
    assert!(narrowed.is_ok(), "{narrowed:?}");
    let messages: Vec<_> =
        backend.asked().into_iter().map(|asked| asked.selection_required).collect();
    assert_eq!(messages, [Some("Mindestens eine Auswahl".to_owned())]);
}

#[rstest]
fn never_asks_for_passed_values() {
    let backend = ScriptedBackend::new([]);
    let release = Release::couture_try_parse_from_with(&backend, ["release", "--envs", "prod"]);
    assert_eq!(release.map(|release| release.envs).ok(), Some(Vec::from([Env::Prod])));
    assert_eq!(backend.asked(), []);
}

#[rstest]
fn leaves_an_optional_list_unset_on_no_answer() {
    let backend = ScriptedBackend::new([Reply::MultiSelect(&[]), Reply::Text(None)]);
    let maybe = Maybe::couture_try_parse_from_with(&backend, ["maybe"]);
    assert_eq!(maybe.map(|maybe| (maybe.envs, maybe.tags)).ok(), Some((None, None)));
    assert_eq!(backend.asked(), [multi_select(&[]), text("Which tags?", None, true)]);
}

#[rstest]
fn asks_for_text_values_until_an_empty_answer() {
    let backend = ScriptedBackend::new([
        Reply::Text(Some("80")),
        Reply::Text(Some("443")),
        Reply::Text(None),
    ]);
    let serve = Serve::couture_try_parse_from_with(&backend, ["serve"]);
    assert_eq!(serve.map(|serve| serve.ports).ok(), Some(Vec::from([80, 443])));
    assert_eq!(backend.asked(), [
        text("Which ports?", None, false),
        text("Which ports?", None, true),
        text("Which ports?", None, true),
    ]);
}

#[rstest]
fn asks_an_invalid_value_again_and_keeps_the_rest() {
    let backend = ScriptedBackend::new([
        Reply::Text(Some("80")),
        Reply::Text(Some("http")),
        Reply::Text(Some("443")),
        Reply::Text(None),
    ]);
    let serve = Serve::couture_try_parse_from_with(&backend, ["serve"]);
    assert_eq!(serve.map(|serve| serve.ports).ok(), Some(Vec::from([80, 443])));
    let errors: Vec<_> = backend.asked().into_iter().map(|asked| asked.error).collect();
    assert_eq!(errors, [
        None,
        None,
        Some(
            "invalid value 'http' for '--ports <PORTS>': invalid digit found in string".to_owned()
        ),
        None,
    ]);
}

#[rstest]
fn starts_each_value_as_the_default_in_turn() {
    let backend = ScriptedBackend::new([
        Reply::Text(Some("a")),
        Reply::Text(Some("c")),
        Reply::Text(Some("d")),
        Reply::Text(None),
    ]);
    let labels = Labels::couture_try_parse_from_with(&backend, ["labels"]);
    assert_eq!(
        labels.map(|labels| labels.labels).ok(),
        Some(["a", "c", "d"].map(str::to_owned).into())
    );
    assert_eq!(backend.asked(), [
        text("Which labels?", Some("a"), true),
        text("Which labels?", Some("b"), true),
        text("Which labels?", None, true),
        text("Which labels?", None, true),
    ]);
}

#[rstest]
fn splits_a_delimited_default_into_its_values() {
    let backend = ScriptedBackend::new([Reply::Text(Some("a")), Reply::Text(None)]);
    let split = Split::couture_try_parse_from_with(&backend, ["split"]);
    assert_eq!(split.map(|split| split.labels).ok(), Some(Vec::from(["a".to_owned()])));
    assert_eq!(backend.asked(), [
        text("Which labels?", Some("a"), true),
        text("Which labels?", Some("b"), true),
    ]);
}

#[rstest]
fn confirms_after_every_prompt_with_the_answers_filled_in() {
    let backend = ScriptedBackend::new([Reply::Text(Some("web")), Reply::Confirm(true)]);
    let argv = ["delete", "--regions", "eu", "--regions", "us"];
    let delete = Delete::couture_try_parse_from_with(&backend, argv);
    assert_eq!(delete.map(|delete| delete.force).ok(), Some(true));
    assert_eq!(backend.asked(), [
        asked(Kind::Text, "Which app?", None, &[]),
        asked(Kind::Confirm, "Delete web from eu, us?", Some("false"), &[]),
    ]);
}

#[rstest]
fn a_declined_confirmation_is_an_io_error() {
    let backend = ScriptedBackend::new([Reply::Confirm(false)]);
    let delete = Delete::couture_try_parse_from_with(&backend, ["delete", "--name", "web"]);
    assert!(matches!(delete, Err(ParseError::Declined)));
}

#[rstest]
#[case::flag_passed(&["delete", "--name", "web", "--force"], true)]
#[case::conflicting_arg_passed(&["delete", "--name", "web", "--dry-run"], false)]
fn never_confirms_a_passed_or_conflicting_flag(#[case] argv: &[&str], #[case] force: bool) {
    let backend = ScriptedBackend::new([]);
    let delete = Delete::couture_try_parse_from_with(&backend, argv);
    assert_eq!(delete.map(|delete| delete.force).ok(), Some(force));
    assert_eq!(backend.asked(), []);
}

#[rstest]
#[case::flag_passed(&["delete", "--name", "web", "--force"], true)]
#[case::conflicting_arg_passed(&["delete", "--name", "web", "--dry-run"], false)]
fn without_a_terminal_a_passed_or_conflicting_flag_parses(
    #[case] argv: &[&str],
    #[case] force: bool,
) {
    let backend = ScriptedBackend::unavailable();
    let delete = Delete::couture_try_parse_from_with(&backend, argv);
    assert_eq!(delete.map(|delete| delete.force).ok(), Some(force));
}

#[rstest]
fn without_a_terminal_an_unconfirmed_flag_is_a_missing_required_arg() {
    let backend = ScriptedBackend::unavailable();
    let wipe = Wipe::couture_try_parse_from_with(&backend, ["wipe", "--name", "web"]);
    let err = wipe.map_err(|err| (err.kind(), err.to_string())).err();
    assert!(
        err.as_ref().is_some_and(|(kind, rendered)| {
            *kind == Some(ErrorKind::MissingRequiredArgument) && rendered.contains("--force")
        }),
        "{err:?}"
    );
}

#[rstest]
fn without_a_terminal_help_leaves_the_confirmed_flag_optional() {
    let backend = ScriptedBackend::unavailable();
    let help = Delete::couture_try_parse_from_with(&backend, ["delete", "--help"])
        .map(drop)
        .map_err(|err| err.to_string());
    let help = help.err().unwrap_or_default();
    let usage = help.lines().skip_while(|line| !line.starts_with("Usage:")).nth(1);
    assert!(usage.is_some_and(|usage| !usage.contains("--force")), "{help}");
}

#[rstest]
fn a_confirmation_keeps_escaped_braces_and_unknown_placeholders() {
    let backend = ScriptedBackend::new([Reply::Confirm(true)]);
    let braces = Braces::couture_try_parse_from_with(&backend, ["braces", "--name", "x"]);
    assert_eq!(braces.map(|braces| braces.yes).ok(), Some(true));
    let questions: Vec<_> = backend.asked().into_iter().map(|asked| asked.question).collect();
    assert_eq!(questions, ["{name} x {nope} {"]);
}

#[rstest]
#[case::taken(&["store", "drop", "b1"], &["Drop b1 for good?"])]
#[case::not_taken(&["store", "list"], &[])]
fn confirms_only_on_the_subcommand_path_taken(#[case] argv: &[&str], #[case] questions: &[&str]) {
    let backend = ScriptedBackend::new([Reply::Confirm(true)]);
    let store = Store::couture_try_parse_from_with(&backend, argv);
    assert!(store.is_ok(), "{store:?}");
    let asked: Vec<_> = backend.asked().into_iter().map(|asked| asked.question).collect();
    assert_eq!(asked, questions);
}
