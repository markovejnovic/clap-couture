//! Example demonstrating clap-couture's `-` as an arg's value.
//!
//! clap-couture can give `-` a meaning, and leave the arg alone otherwise. You do so by adding
//! `dash` to `#[couture(prompt)]` or `#[couture(stdin(piped))]`.
//!
//!   - `orbit-release --title -` asks for the title; left out, it is not asked.
//!   - `orbit-release --notes -` reads the notes from stdin, be it a pipe, a file or the terminal;
//!     left out, stdin is not read.
//!   - One trailing newline is dropped, so `echo` reads as typed.
//!
//! # Example Output
//!
//! `cargo run -p clap-couture --example 11-interactive-dash -- 'Fix login' '|' orbit-release
//! --title v1.2 --notes -` outputs:
//!
//! ```terminal
//! releasing v1.2
//! notes: Fix login
//! ```
//!
//! `echo "Fix login" | cargo run -p clap-couture --example 11-interactive-dash -- --title
//! v1.2 --notes -` outputs:
//!
//! ```terminal
//! releasing v1.2
//! notes: Fix login
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::Parser;
use clap_couture::{Couture, CoutureParser as _};

#[derive(Parser, Couture)]
struct Cli {
    /// Release notes; `-` reads them from stdin.
    #[arg(long)]
    #[couture(stdin)]
    notes: Option<String>,

    /// Release title; `-` asks for it.
    #[arg(long)]
    #[couture(prompt(dash, question = "Release title"))]
    title: String,
}

fn main() {
    let cli = Cli::couture_parse();
    println!("releasing {}", cli.title);
    if let Some(notes) = cli.notes {
        println!("notes: {notes}");
    }
}
