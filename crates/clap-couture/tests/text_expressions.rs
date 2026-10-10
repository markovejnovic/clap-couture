//! Like clap's `about = expr`, a category's `title` and `description` take any expression,
//! evaluated each time the help is shown, so localized text follows a locale picked at runtime.

extern crate alloc;

use alloc::borrow::Cow;
use core::cell::Cell;

use clap::{Parser, Subcommand};
use clap_couture::{Couture, CoutureParser as _};
use rstest::rstest;

thread_local! {
    /// The locale each test picks; thread-local, as tests run in parallel.
    static LOCALE: Cell<Locale> = const { Cell::new(Locale::English) };
}

#[derive(Parser, Couture)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture)]
#[couture(categories = {
    "deploy" = { title = t("deploy.title"), description = t("deploy.description") },
    "account" = { title = format!("{}:", "Account"), description = Cow::Borrowed("Who you are") },
    "everyday" = { title = "Everyday:" },
})]
enum Cmd {
    /// Sign in.
    #[category("account")]
    Login,
    /// Tail live logs.
    #[category("everyday")]
    Logs,
    /// Ship the current project.
    #[category("deploy")]
    Ship,
}

#[derive(Clone, Copy)]
enum Locale {
    English,
    German,
}

fn help_in(locale: Locale) -> String {
    LOCALE.set(locale);
    Cli::couture_command().render_help().to_string()
}

/// A stand-in for an i18n crate's `t!`: the message for `key` in the current locale.
fn t(key: &'static str) -> &'static str {
    match (LOCALE.get(), key) {
        (Locale::English, "deploy.title") => "Deploy:",
        (Locale::German, "deploy.title") => "Bereitstellen:",
        (Locale::English, "deploy.description") => "Ship code to production",
        (Locale::German, "deploy.description") => "Code in Produktion bringen",
        _ => key,
    }
}

#[rstest]
#[case::english(Locale::English, "Deploy:", "Ship code to production")]
#[case::german(Locale::German, "Bereitstellen:", "Code in Produktion bringen")]
fn category_text_follows_the_runtime_locale(
    #[case] locale: Locale,
    #[case] title: &str,
    #[case] description: &str,
) {
    let help = help_in(locale);
    assert!(help.contains(title), "no `{title}` in:\n{help}");
    assert!(help.contains(description), "no `{description}` in:\n{help}");
}

#[rstest]
fn category_text_takes_strings_cows_and_literals() {
    let help = help_in(Locale::English);
    for text in ["Account:", "Who you are", "Everyday:"] {
        assert!(help.contains(text), "no `{text}` in:\n{help}");
    }
}
