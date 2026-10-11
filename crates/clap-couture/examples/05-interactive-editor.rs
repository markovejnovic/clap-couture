//! Prompted args: leave out `--region`, `--strategy`, `--follow` or `--notes` and couture asks,
//! the notes in your `$EDITOR`.
//!
//! ```sh
//! cargo run -p clap-couture --example 05-interactive-editor --features interactive-editor
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

    /// Release notes for this deploy.
    #[arg(long)]
    #[couture(prompt, editor(extension = "md"))]
    notes: Option<String>,

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
    if let Some(notes) = cli.notes {
        println!("notes:\n{notes}");
    }
}
