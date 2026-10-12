//! A subcommand menu wider than the terminal wraps as clap's options do: each wrapped line stays
//! under the column it started in.

use clap::{Command, CommandFactory as _, Parser, Subcommand};
use clap_couture::{CommandExt as _, Couture};
use rstest::rstest;

#[derive(Parser, Couture)]
#[command(name = "orbit")]
struct Orbit {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture)]
#[couture(categories = {
    "deploy" = { title = "Deploy:", description = "Ship code to production and back" }
})]
enum Cmd {
    /// Ship the current project to every region at once.
    #[couture(category = "deploy")]
    Deploy,
    /// Show what is running in every region.
    Status,
}

#[derive(Parser, Couture)]
#[command(name = "orbit")]
struct Styled {
    #[command(subcommand)]
    cmd: StyledCmd,
}

#[derive(Subcommand, Couture)]
enum StyledCmd {
    /// Ship **the current** project **to every** region at `once`.
    Deploy,
}

#[derive(Parser, Couture)]
#[command(name = "orbit")]
struct Nested {
    #[command(subcommand)]
    cmd: NestedCmd,
}

#[derive(Subcommand, Couture)]
enum NestedCmd {
    /// Manage the fleet.
    #[command(subcommand)]
    Fleet(Cmd),
}

/// `cmd`'s help with couture, as plain text.
fn help(cmd: Command) -> String {
    cmd.with_couture::<Orbit>().render_help().to_string()
}

/// The menu in `help`: what sits between the usage and the options.
fn menu(help: &str) -> &str {
    let menu = help.split_once("<COMMAND>\n\n").map_or("", |(_, rest)| rest);
    menu.split_once("\n\nOptions:").map_or("", |(menu, _)| menu)
}

#[rstest]
fn an_about_wraps_under_the_about_column() {
    let help = help(Orbit::command().term_width(40));
    assert!(help.contains("  status  Show what is running in every\n          region\n"), "{help}");
}

#[rstest]
fn a_description_wraps_under_the_description_column() {
    let help = help(Orbit::command().term_width(40));
    assert!(help.contains("Deploy:     Ship code to production and\n            back\n"), "{help}");
}

#[rstest]
fn a_narrow_menu_keeps_every_line_under_its_column() {
    let help = help(Orbit::command().term_width(30));
    let expected = "  status  Show what is running
          in every region

Deploy:     Ship code to
            production and
            back
  deploy  Ship the current
          project to every
          region at once";
    assert_eq!(menu(&help), expected, "{help}");
}

#[rstest]
fn the_menu_fits_the_width(#[values(24, 27, 30, 33, 36, 40, 44, 45, 50, 60)] width: usize) {
    let help = help(Orbit::command().term_width(width));
    assert!(help.lines().all(|line| line.chars().count() <= width), "{help}");
    assert_eq!(menu(&help).matches("\n\n").count(), 1, "{help}");
}

#[rstest]
fn a_nested_menu_wraps_to_the_roots_width() {
    let mut cmd = Nested::command().term_width(40).with_couture::<Nested>();
    let fleet = cmd.find_subcommand_mut("fleet").map(|fleet| fleet.render_help().to_string());
    let help = fleet.unwrap_or_default();
    assert!(help.contains("  status  Show what is running in every\n          region\n"), "{help}");
}

#[rstest]
fn a_max_term_width_is_respected() {
    let capped = help(Orbit::command().max_term_width(40));
    assert_eq!(capped, help(Orbit::command().term_width(40)));
}

#[rstest]
fn a_menu_that_fits_is_left_whole() {
    let help = help(Orbit::command().term_width(0));
    assert!(
        help.contains("  deploy  Ship the current project to every region at once\n"),
        "{help}"
    );
}

#[cfg(feature = "markdown")]
#[rstest]
fn markdown_wraps_by_the_text_it_shows() {
    let mut cmd = Styled::command().term_width(40).with_couture::<Styled>();
    let help = cmd.render_help();
    let plain = help.to_string();
    assert!(
        plain.contains("  deploy  Ship the current project to\n          every region at once"),
        "{plain}"
    );
    // The style ends before the line does, and picks up again past the indent.
    let ansi = help.ansi().to_string();
    assert!(ansi.contains("to\u{1b}[0m\n          \u{1b}[1mevery"), "{ansi:?}");
}
