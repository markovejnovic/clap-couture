//! Example demonstrating clap-couture's secret prompts.
//!
//! clap-couture can ask for a password or a token without echoing it. You do so by decorating a
//! `SecretString` field with `#[couture(prompt, secret)]`.
//!
//!   - The answer is masked as it's typed, and never reaches clap as plaintext.
//!   - `--token` is refused on the command line, where `ps` and shell history would see it;
//!     `secret(allow_argv)` allows it.
//!   - A token in `ORBIT_TOKEN` is used without asking, and `--help` doesn't show its value.
//!   - An `Option<SecretString>` field makes the secret optional: left empty, it stays `None`.
//!
//! # Example Output
//!
//! `cargo run -p clap-couture --example 07-interactive-secret` outputs:
//!
//! ```terminal
//! ◇  Application name
//! │  web
//! │
//! ◇  Deploy token
//! │  ▪▪▪▪▪▪▪
//! │
//! deploying web with the token
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::Parser;
use clap_couture::{Couture, CoutureParser as _, interactive::secrecy::SecretString};

#[derive(Parser, Couture)]
struct Cli {
    /// Application name.
    #[arg(long)]
    #[couture(prompt)]
    app: String,

    /// Deploy token.
    #[arg(long, env = "ORBIT_TOKEN")]
    #[couture(prompt, secret)]
    token: SecretString,
}

fn main() {
    let cli = Cli::couture_parse();

    // A real deploy hands the token on; the demo only says it arrived.
    let _token: SecretString = cli.token;
    println!("deploying {} with the token", cli.app);
}
