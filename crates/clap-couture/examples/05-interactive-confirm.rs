//! Example demonstrating clap-couture's confirmations.
//!
//! clap-couture can ask the user to confirm before a destructive command goes ahead. You do so by
//! decorating a `bool` flag, like `--force`, with `#[couture(confirm = "...")]`.
//!
//!   - The question is asked after every `#[couture(prompt)]`, so it can name the answers.
//!   - `{arg}` in the question stands for that arg's value.
//!   - A yes sets the flag; a no exits with status 1.
//!   - Passing `--force` skips the question.
//!   - Without a terminal, `--force` must be passed: clap reports it as a missing required arg.
//!
//! # Example Output
//!
//! `cargo run -p clap-couture --example 05-interactive-confirm` outputs:
//!
//! ```terminal
//! ◇  Application name
//! │  web
//! │
//! ◇  Which region?
//! │  eu
//! │
//! ◇  Destroy web in eu? This cannot be undone.
//! │  Yes
//! │
//! destroying web in eu
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::{Parser, ValueEnum};
use clap_couture::{Couture, CoutureParser as _};

#[derive(Parser, Couture)]
struct Cli {
    /// Application name.
    #[arg(long)]
    #[couture(prompt)]
    app: String,

    /// Skip the confirmation.
    #[arg(long)]
    // The question names the app and region, whether they were passed or answered above.
    #[couture(confirm = "Destroy {app} in {region}? This cannot be undone.")]
    force: bool,

    /// Application region.
    #[arg(long, value_enum)]
    #[couture(prompt = "Which region?")]
    region: Region,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Region {
    Apac,
    Eu,
    Us,
}

fn main() {
    // Past this point, `cli.force` is always true: the user said yes, or passed `--force`.
    let cli = Cli::couture_parse();

    let region = match cli.region {
        Region::Apac => "apac",
        Region::Eu => "eu",
        Region::Us => "us",
    };
    println!("destroying {} in {region}", cli.app);
}
