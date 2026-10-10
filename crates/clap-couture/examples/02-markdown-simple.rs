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
