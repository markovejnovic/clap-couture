//! A minimal example of how clap-couture enables you to use markdown.
//!
//! clap-couture supports markdown too! You can use it either with categories, or with rsdoc for
//! your pretty output.
//!
//! # Example Output
//!
//! **Note**: This example output does not contain the markdown, since we cannot render markdown in
//! plaintext. clap-couture will not render terminal escape codes if it's not writing to a
//! terminal.
//!
//! `cargo run -p clap-couture --features markdown --example 02-markdown-simple -- --help` outputs:
//!
//! ```terminal
//! Usage:
//!   02-markdown-simple <COMMAND>
//!
//!   deploy  Ship the current project
//!   login   Sign in to your account
//!
//! Options:
//!   -h, --help  Print help
//! ```
#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::{Parser, Subcommand};
use clap_couture::{Couture, CoutureParser as _};

#[derive(Parser, Couture)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture)]
enum Cmd {
    /// Ship the *current project*.
    Deploy,
    /// Sign in to *your* account.
    Login,
}

fn main() {
    let cli = Cli::couture_parse();

    let ran = match cli.cmd {
        Cmd::Deploy => "deploy",
        Cmd::Login => "login",
    };
    println!("running: {ran}");
}
