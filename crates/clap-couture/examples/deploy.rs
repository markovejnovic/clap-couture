//! Prompts for marked args: leave out `--region`, `--strategy` or `--follow` and couture asks.
//!
//! ```sh
//! cargo run -p clap-couture --example deploy --features interactive-cliclack
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::{Parser, ValueEnum};
use clap_couture::{Couture, CoutureParser as _};

#[derive(Parser, Couture)]
#[command(name = "deploy", about = "Ship the current project.")]
struct Cli {
    /// Tail the logs afterwards.
    #[arg(long)]
    #[couture(prompt)]
    follow: bool,

    /// Region to deploy to.
    #[arg(long)]
    #[couture(prompt)]
    region: String,

    /// How to roll the release out.
    #[arg(long, value_enum, default_value = "rolling")]
    #[couture(prompt)]
    strategy: Strategy,
}

#[derive(Clone, Copy, ValueEnum)]
enum Strategy {
    /// A slice of traffic first.
    Canary,
    /// One instance at a time.
    Rolling,
}

fn main() {
    let cli = Cli::couture_parse();
    let strategy = cli.strategy.to_possible_value().map(|value| value.get_name().to_owned());
    println!(
        "deploying to {} ({}), following logs: {}",
        cli.region,
        strategy.unwrap_or_default(),
        cli.follow
    );
}
