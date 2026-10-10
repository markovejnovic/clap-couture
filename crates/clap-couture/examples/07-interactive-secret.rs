//! Prompted args: leave out `--region`, `--strategy` or `--follow` and couture asks; the token
//! comes from `DEPLOY_TOKEN` or is asked for, masked.
//!
//! ```sh
//! cargo run -p clap-couture --example 07-interactive-secret
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::{Parser, ValueEnum};
use clap_couture::{Couture, CoutureParser as _, interactive::secrecy::SecretString};

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

    /// Token to deploy with.
    #[arg(long, env = "DEPLOY_TOKEN")]
    #[couture(prompt, secret)]
    token: SecretString,
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
    // A real deploy hands the token on; the demo only says it arrived.
    let _token: SecretString = cli.token;
    println!(
        "deploying to {} ({}), following logs: {}, token: received",
        cli.region,
        strategy.unwrap_or_default(),
        cli.follow
    );
}
