//! `#[command(flatten)]` merges another type's commands into a level, and their categories must
//! come along: from a flattened subcommand enum's variants, and from the subcommand of a flattened
//! `Args` struct.

use clap::{Args, Parser, Subcommand};
use clap_couture::{CommandCategoryMap, Couture, CoutureParser as _};
use rstest::rstest;

#[derive(Parser, Couture)]
struct Direct {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Parser, Couture)]
struct ThroughArgs {
    #[command(flatten)]
    wrap: Wrap,
}

#[derive(Parser, Couture)]
struct ThroughOptionalArgs {
    #[command(flatten)]
    wrap: Option<Wrap>,
}

#[derive(Parser, Couture)]
struct BesideForeignArgs {
    #[command(subcommand)]
    cmd: Cmd,

    #[command(flatten)]
    verbosity: Verbosity,
}

#[derive(Args, Couture)]
struct Wrap {
    #[command(subcommand)]
    cmd: Cmd,
}

/// Stands in for an `Args` type from another crate, which cannot implement `Couture`.
#[derive(Args)]
struct Verbosity {
    #[arg(long)]
    verbose: bool,
}

#[derive(Subcommand, Couture)]
#[couture(categories = { "a" = { title = "Alpha" }, "b" = { title = "Beta" } })]
enum Cmd {
    #[command(flatten)]
    Account(Account),
    /// Ship it.
    #[category("a")]
    Deploy,
}

#[derive(Subcommand, Couture)]
#[couture(categories = { "b" = { title = "Child title" } })]
enum Account {
    /// Log in.
    #[category("b")]
    Login,
    /// Log out.
    Logout,
    #[command(flatten)]
    Nested(Nested),
}

#[derive(Subcommand, Couture)]
enum Nested {
    /// Sync settings.
    #[category("b")]
    Sync,
}

fn label_of(categories: CommandCategoryMap, command: &str) -> Option<&'static str> {
    categories.iter().find(|(name, _)| name.as_str() == command).map(|(_, category)| category.label)
}

#[rstest]
#[case::flattened_variant(Cmd::CATEGORIES, "login", Some("b"))]
#[case::flatten_in_flatten(Cmd::CATEGORIES, "sync", Some("b"))]
#[case::uncategorized_in_flattened(Cmd::CATEGORIES, "logout", None)]
#[case::flatten_variant_is_no_command(Cmd::CATEGORIES, "account", None)]
#[case::subcommand_field(Direct::CATEGORIES, "login", Some("b"))]
#[case::through_args(ThroughArgs::CATEGORIES, "deploy", Some("a"))]
#[case::through_optional_args(ThroughOptionalArgs::CATEGORIES, "login", Some("b"))]
#[case::beside_foreign_args(BesideForeignArgs::CATEGORIES, "sync", Some("b"))]
fn flattened_commands_keep_their_category(
    #[case] categories: CommandCategoryMap,
    #[case] command: &str,
    #[case] expected_label: Option<&str>,
) {
    assert_eq!(label_of(categories, command), expected_label);
    assert_eq!(categories.find(command).map(|category| category.label), expected_label);
}

#[rstest]
#[case::direct(Direct::couture_command())]
#[case::through_args(ThroughArgs::couture_command())]
fn help_files_flattened_commands_under_the_parents_heading(#[case] mut command: clap::Command) {
    let help = command.render_help().to_string();
    let beta_section = help.split_once("Beta:").map(|(_, after_heading)| after_heading);
    assert!(
        beta_section.is_some_and(|section| section.contains("login") && section.contains("sync")),
        "login and sync not under Beta in:\n{help}"
    );
    assert!(!help.contains("Child title"), "the child's title beat the parent's in:\n{help}");
}
