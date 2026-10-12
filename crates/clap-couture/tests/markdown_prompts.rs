//! With `markdown`, a prompt's question is markdown too: styled when color is on, plain when off.
#![cfg(all(feature = "markdown", feature = "interactive"))]

#[expect(dead_code, reason = "only text and confirm replies are scripted here")]
mod common;

use clap::{ColorChoice, Parser};
use clap_couture::{Couture, CoutureParser as _};
use common::{Reply, ScriptedBackend};
use rstest::rstest;

#[derive(Parser, Couture)]
#[command(color = ColorChoice::Always)]
struct Colored {
    /// Release notes, in **markdown**.
    #[arg(long)]
    #[couture(prompt)]
    notes: String,
}

#[derive(Parser, Couture)]
#[command(color = ColorChoice::Never)]
struct Plain {
    /// Release notes, in **markdown**.
    #[arg(long)]
    #[couture(prompt)]
    notes: String,

    #[arg(long)]
    #[couture(confirm = "Ship `{notes}`?")]
    yes: bool,
}

fn questions(backend: &ScriptedBackend) -> Vec<String> {
    backend.asked().into_iter().map(|asked| asked.question).collect()
}

#[rstest]
fn a_question_is_styled_when_color_is_on() {
    let backend = ScriptedBackend::new([Reply::Text(Some("hi"))]);
    let notes = Colored::couture_try_parse_from_with(&backend, ["app"]).map(|cli| cli.notes);
    assert_eq!(notes.ok().as_deref(), Some("hi"));
    assert_eq!(questions(&backend), ["Release notes, in \u{1b}[1mmarkdown\u{1b}[0m"]);
}

#[rstest]
fn a_question_is_plain_when_color_is_off() {
    let backend = ScriptedBackend::new([Reply::Text(Some("hi"))]);
    let notes = Plain::couture_try_parse_from_with(&backend, ["app", "--yes"]).map(|cli| cli.notes);
    assert_eq!(notes.ok().as_deref(), Some("hi"));
    assert_eq!(questions(&backend), ["Release notes, in markdown"]);
}

#[rstest]
fn an_answer_in_a_confirmation_is_not_markdown() {
    let backend = ScriptedBackend::new([Reply::Text(Some("**bold**")), Reply::Confirm(true)]);
    let yes = Plain::couture_try_parse_from_with(&backend, ["app"]).map(|cli| cli.yes);
    assert_eq!(yes.ok(), Some(true));
    assert_eq!(questions(&backend), ["Release notes, in markdown", "Ship **bold**?"]);
}
