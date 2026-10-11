//! Demo of localizing what clap-couture asks and prints.
//!
//! Any text couture shows can be an expression instead of a string literal, evaluated each
//! time it's shown: a lookup in your translations, for example. This example fakes one with
//! a `t!` macro, which answers in German when `ORBIT_LANG=de` is set.
//!
//! # Example Output
//!
//! With `ORBIT_LANG=de` set and no flags passed, orbit asks in German.
//!
//! `cargo run -p clap-couture --example 04-interactive-i18n` outputs:
//!
//! ```terminal
//! ◇  Welche App?
//! │  web
//! │
//! ◇  Danach die Logs verfolgen?
//! │  Yes
//! │
//! ◇  In welche Region?
//! │  (keine)
//! │
//! deploying web to every region (follow: true)
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::{Parser, ValueEnum};
use clap_couture::{Couture, CoutureParser as _};

/// Look `key` up in orbit's translations, in the language `ORBIT_LANG` picks.
///
/// A real CLI would use a localization crate; couture only needs an expression.
macro_rules! t {
    ($key:literal) => {
        translate($key)
    };
}

// clap's own texts take expressions too, so the whole interface speaks one language.
#[derive(Parser, Couture)]
#[command(about = t!("about"))]
// The words couture prints itself: the headings of `--help`, and the choice that leaves an
// optional pick unset.
#[couture(messages = { none = t!("none"), options = t!("options"), usage = t!("usage") })]
struct Cli {
    #[arg(long, help = t!("app.help"))]
    // A prompt is any expression, too. You can use this to localize, similar to how
    // you would with clap.
    #[couture(prompt = t!("app.prompt"))]
    app: String,

    #[arg(long, help = t!("follow.help"))]
    #[couture(prompt = t!("follow.prompt"))]
    follow: bool,

    // Optional, so the pick ends with the `none` choice: "(keine)" in German.
    #[arg(long, value_enum, help = t!("region.help"))]
    #[couture(prompt = t!("region.prompt"))]
    region: Option<Region>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Region {
    Apac,
    Eu,
    Us,
}

/// `key`'s text in the language `ORBIT_LANG` picks: German for `de`, else English.
fn translate(key: &str) -> &'static str {
    let german = std::env::var("ORBIT_LANG").is_ok_and(|lang| lang == "de");
    match (key, german) {
        ("about", false) => "Deploy the current project.",
        ("about", true) => "Das aktuelle Projekt ausrollen.",
        ("app.help", false) => "The app to deploy",
        ("app.help", true) => "Die App, die ausgerollt wird",
        ("app.prompt", false) => "Which app?",
        ("app.prompt", true) => "Welche App?",
        ("follow.help", false) => "Tail the logs afterwards",
        ("follow.help", true) => "Danach die Logs verfolgen",
        ("follow.prompt", false) => "Tail the logs afterwards?",
        ("follow.prompt", true) => "Danach die Logs verfolgen?",
        ("none", false) => "(none)",
        ("none", true) => "(keine)",
        ("options", false) => "Options:",
        ("options", true) => "Optionen:",
        ("region.help", false) => "The region to deploy to",
        ("region.help", true) => "Die Region, in die ausgerollt wird",
        ("region.prompt", false) => "Which region?",
        ("region.prompt", true) => "In welche Region?",
        ("usage", false) => "Usage:",
        ("usage", true) => "Verwendung:",
        _ => "?",
    }
}

fn main() {
    let cli = Cli::couture_parse();
    let region = match cli.region {
        Some(Region::Apac) => "apac",
        Some(Region::Eu) => "eu",
        Some(Region::Us) => "us",
        None => "every region",
    };
    println!("deploying {} to {region} (follow: {})", cli.app, cli.follow);
}
