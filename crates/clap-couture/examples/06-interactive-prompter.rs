//! Example demonstrating clap-couture's prompter.
//!
//! Sometimes you can't ask the user for input in the CLI arguments. When that's the case, you can
//! use [`CoutureParser::prompter`] which will give you the exact same styling and behavior as
//! couture used, making your input consistent with couture.
//!
//! # Example Output
//!
//! `cargo run -p clap-couture --example 06-interactive-prompter` outputs:
//!
//! ```terminal
//! ◇  Application name
//! │  web
//! │
//! ◇  Add a database?
//! │  Yes
//! │
//! ◇  Which engine?
//! │  postgres
//! │
//! creating web with postgres
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]

use clap::{Parser, ValueEnum};
use clap_couture::{Couture, CoutureParser as _, interactive::PromptError};

#[derive(Parser, Couture)]
struct Cli {
    /// Application name.
    #[arg(long)]
    #[couture(prompt)]
    app: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Engine {
    Mysql,
    Postgres,
    Sqlite,
}

fn main() -> Result<(), PromptError> {
    let cli = Cli::couture_parse();

    let engine = if let Some(prompter) = Cli::prompter()
        && prompter.confirm("Add a database?", true)?
    {
        Some(prompter.select::<Engine>("Which engine?")?)
    } else {
        None
    };

    match engine {
        Some(Engine::Mysql) => println!("creating {} with mysql", cli.app),
        Some(Engine::Postgres) => println!("creating {} with postgres", cli.app),
        Some(Engine::Sqlite) => println!("creating {} with sqlite", cli.app),
        None => println!("creating {}", cli.app),
    }
    Ok(())
}
