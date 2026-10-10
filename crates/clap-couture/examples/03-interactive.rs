//! `orbit deploy` with prompted args: leave a flag out on a terminal and couture asks for it.
//! Each arg shows a different kind of prompt, picked from the arg's type.
//!
//! ```sh
//! cargo run -p clap-couture --features interactive-cliclack --example 03-interactive
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::{Parser, ValueEnum};
use clap_couture::{Couture, CoutureParser as _};

#[derive(Parser, Couture)]
#[command(name = "orbit-deploy", bin_name = "orbit-deploy", about = "Ship the current project.")]
struct Cli {
    /// A free-form value asks for text.
    #[arg(long)]
    #[couture(prompt = "Which app?")]
    app: String,

    /// A flag asks yes or no.
    #[arg(long)]
    #[couture(prompt = "Tail the logs afterwards?")]
    follow: bool,

    /// A value with possible values asks to pick one.
    #[arg(long, value_enum)]
    #[couture(prompt = "Which region?")]
    region: Region,

    /// A default is pre-filled, and an answer that fails to parse is asked again.
    #[arg(long, default_value_t = 2)]
    #[couture(prompt = "How many replicas?")]
    replicas: u16,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Region {
    Apac,
    Eu,
    Us,
}

fn main() {
    let cli = Cli::couture_parse();
    let region = match cli.region {
        Region::Apac => "apac",
        Region::Eu => "eu",
        Region::Us => "us",
    };
    println!(
        "deploying {} to {region} with {} replicas (follow: {})",
        cli.app, cli.replicas, cli.follow
    );
}
