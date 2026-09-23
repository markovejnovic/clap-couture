//! `CoutureParser` is the parse entry point for every `clap::Parser` that derives `Couture`.

use clap::{Parser, Subcommand};
use clap_couture::{Couture, CoutureParser as _};
use rstest::rstest;

#[derive(Parser, Couture)]
struct Flat {
    #[arg(long)]
    name: String,
}

#[derive(Parser, Couture)]
struct Nested {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture)]
#[couture(categories = { "everyday" = { title = "Everyday" } })]
enum Cmd {
    /// Tail live logs.
    #[category("everyday")]
    Logs,
}

#[rstest]
fn a_cli_without_subcommands_parses() {
    let name = Flat::couture_try_parse_from(["app", "--name", "orbit"]).map(|cli| cli.name);
    assert_eq!(name.ok().as_deref(), Some("orbit"));
}

#[rstest]
fn the_parser_takes_its_categories_from_the_subcommand_field() {
    let help = Nested::couture_command().render_help().to_string();
    assert!(help.contains("Everyday:"), "no category heading in:\n{help}");
}
