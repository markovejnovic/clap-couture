//! Example demonstrating clap-couture's piped input.
//!
//! clap-couture can take an arg's value from a pipe. You do so by decorating a field with
//! `#[couture(stdin)]`.
//!
//!   - `echo "Fix login" | orbit-release` works like `orbit-release --notes "Fix login"`.
//!   - Stdin is read only when it is a pipe and `--notes` was left out.
//!   - Beside `prompt`, the notes are asked for when nothing is piped. Prompts read stdin, so with
//!     stdin piped nothing is asked: every other arg comes from argv or env.
//!   - One trailing newline is dropped, so `echo` reads as typed.
//!
//! # Example Output
//!
//! `echo "Fix the login page" | cargo run -p clap-couture --example 10-interactive-stdin` outputs:
//!
//! ```terminal
//! releasing with notes: Fix the login page
//! ```
//!
//! With nothing piped, the notes are asked for.
//!
//! `cargo run -p clap-couture --example 10-interactive-stdin` outputs:
//!
//! ```terminal
//! ◇  Release notes
//! │  Fix the login page
//! │
//! releasing with notes: Fix the login page
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::Parser;
use clap_couture::{Couture, CoutureParser as _};

#[derive(Parser, Couture)]
struct Cli {
    /// Release notes.
    #[arg(long)]
    #[couture(prompt, stdin)]
    notes: String,
}

fn main() {
    let cli = Cli::couture_parse();
    println!("releasing with notes: {}", cli.notes);
}
