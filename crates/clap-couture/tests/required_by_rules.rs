//! A prompted arg that clap requires through a rule other than `required` is asked for, or
//! left to clap's error where clap can't lift the rule.
#![cfg(feature = "interactive")]

#[expect(dead_code, reason = "only text replies are scripted here")]
mod common;

use clap::{Parser, Subcommand, error::ErrorKind};
use clap_couture::{Couture, CoutureParser as _};
use common::{Asked, Kind, Reply, ScriptedBackend};
use rstest::rstest;

/// `--key` is required by each of the other args in its own way.
#[derive(Parser, Couture, Debug)]
struct Release {
    #[arg(long, requires_if("stable", "key"))]
    channel: Option<String>,

    #[arg(long)]
    #[couture(prompt = "Which key?")]
    key: Option<String>,

    /// Requires `--registry`, which is never asked, beside `--key`.
    #[arg(long, requires_all(["key", "registry"]))]
    mirror: bool,

    #[arg(long)]
    registry: Option<String>,

    #[arg(long, requires = "key")]
    sign: bool,

    #[arg(long)]
    #[couture(prompt = "Which title?")]
    title: Option<String>,

    #[arg(long, requires_ifs([("lts", "key")]))]
    track: Option<String>,
}

/// `push --signed` requires the subcommand's `--key`.
#[derive(Parser, Couture, Debug)]
struct Registry {
    #[command(subcommand)]
    cmd: RegistryCmd,
}

#[derive(Subcommand, Couture, Debug)]
enum RegistryCmd {
    Push {
        #[arg(long)]
        #[couture(prompt = "Which key?")]
        key: Option<String>,

        #[arg(long, requires = "key")]
        signed: bool,
    },
}

/// A prompted token under each rule clap can't lift from it.
#[derive(Parser, Couture, Debug)]
struct Gate {
    #[arg(long, required_if_eq_all([("mode", "audit"), ("zone", "eu")]))]
    #[couture(prompt = "Which audit token?")]
    audit_token: Option<String>,

    #[arg(long, required_if_eq_any([("mode", "locked")]))]
    #[couture(prompt = "Which lock token?")]
    lock_token: Option<String>,

    #[arg(long)]
    mode: Option<String>,

    #[arg(long, required_if_eq("mode", "strict"))]
    #[couture(prompt = "Which token?")]
    token: Option<String>,

    #[arg(long)]
    zone: Option<String>,

    #[arg(long, required_unless_present_all(["mode", "zone"]))]
    #[couture(prompt = "Which zone token?")]
    zone_token: Option<String>,
}

fn asked_for_key() -> Asked {
    Asked {
        defaults: Vec::new(),
        error: None,
        hint: None,
        initial: None,
        kind: Kind::Text,
        options: Vec::new(),
        question: "Which key?".to_owned(),
        selection_required: None,
    }
}

#[rstest]
#[case::requires(&["release", "--title", "t", "--sign"])]
#[case::requires_if(&["release", "--title", "t", "--channel", "stable"])]
#[case::requires_ifs(&["release", "--title", "t", "--track", "lts"])]
#[case::requires_beside_a_prompted_arg_passed_last(&["release", "--sign", "--title", "t"])]
fn asks_for_an_arg_a_passed_arg_requires(#[case] argv: &[&str]) {
    let backend = ScriptedBackend::new([Reply::Text(Some("k1"))]);
    let release = Release::couture_try_parse_from_with(&backend, argv);
    assert_eq!(release.map(|release| release.key).ok(), Some(Some("k1".to_owned())));
    assert_eq!(backend.asked(), [asked_for_key()]);
}

#[rstest]
fn asks_for_an_arg_a_passed_arg_of_a_subcommand_requires() {
    let backend = ScriptedBackend::new([Reply::Text(Some("k1"))]);
    let registry =
        Registry::couture_try_parse_from_with(&backend, ["registry", "push", "--signed"]);
    let key = registry.map(|registry| {
        let RegistryCmd::Push { key, signed: _ } = registry.cmd;
        key
    });
    assert_eq!(key.ok(), Some(Some("k1".to_owned())));
    assert_eq!(backend.asked(), [asked_for_key()]);
}

#[rstest]
#[case::requires(&["release", "--title", "t", "--sign"])]
#[case::requires_if(&["release", "--title", "t", "--channel", "stable"])]
#[case::requires_ifs(&["release", "--title", "t", "--track", "lts"])]
fn leaves_an_unanswered_arg_a_passed_arg_requires_to_clap(#[case] argv: &[&str]) {
    let backend = ScriptedBackend::new([Reply::Text(None)]);
    let release = Release::couture_try_parse_from_with(&backend, argv);
    assert_eq!(
        release.map_err(|err| err.kind()).err(),
        Some(Some(ErrorKind::MissingRequiredArgument))
    );
    assert_eq!(backend.asked(), [asked_for_key()]);
}

#[rstest]
#[case::requires_if(&["release", "--title", "t", "--channel", "beta"])]
#[case::requires_ifs(&["release", "--title", "t", "--track", "edge"])]
fn an_arg_a_passed_arg_does_not_require_may_stay_unset(#[case] argv: &[&str]) {
    let backend = ScriptedBackend::new([Reply::Text(None)]);
    let release = Release::couture_try_parse_from_with(&backend, argv);
    assert_eq!(release.map(|release| release.key).ok(), Some(None));
}

#[rstest]
fn a_required_arg_that_is_not_prompted_is_still_clap_s_error() {
    let backend = ScriptedBackend::new([]);
    let release = Release::couture_try_parse_from_with(&backend, ["release", "--mirror"]);
    assert_eq!(
        release.map_err(|err| err.kind()).err(),
        Some(Some(ErrorKind::MissingRequiredArgument))
    );
    assert_eq!(backend.asked(), []);
}

#[rstest]
fn without_a_terminal_an_arg_a_passed_arg_requires_is_clap_s_error() {
    let backend = ScriptedBackend::unavailable();
    let release = Release::couture_try_parse_from_with(&backend, ["release", "--sign"]);
    assert_eq!(
        release.map_err(|err| err.kind()).err(),
        Some(Some(ErrorKind::MissingRequiredArgument))
    );
}

#[rstest]
#[case::required_if_eq(&["gate", "--mode", "strict", "--zone", "us"], "--token")]
#[case::required_if_eq_any(&["gate", "--mode", "locked", "--zone", "us"], "--lock-token")]
#[case::required_if_eq_all(&["gate", "--mode", "audit", "--zone", "eu"], "--audit-token")]
#[case::required_unless_present_all(&["gate", "--zone", "us"], "--zone-token")]
fn a_rule_clap_can_t_lift_fails_with_clap_s_error_before_asking(
    #[case] argv: &[&str],
    #[case] missing: &str,
) {
    let backend = ScriptedBackend::new([]);
    let gate = Gate::couture_try_parse_from_with(&backend, argv);
    let err = gate.map_err(|err| (err.kind(), err.to_string())).err();
    assert!(
        err.as_ref().is_some_and(|(kind, message)| {
            *kind == Some(ErrorKind::MissingRequiredArgument) && message.contains(missing)
        }),
        "{err:?}"
    );
    assert_eq!(backend.asked(), []);
}
