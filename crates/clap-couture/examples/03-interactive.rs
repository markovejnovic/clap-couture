//! Example demonstrating clap-couture's interactive mode.
//!
//! clap-couture allows you to ask the user for input if they don't provide a required flag.
//! You do so by decorating your flag with `#[couture(prompt)]`.
//! clap-couture will avoid asking for input:
//!
//!   - If the CLI `--flag` is passed. clap's behavior is respected.
//!   - If the environment value `FLAG=...` is passed. clap's behavior is respected.
//!   - If stderr or stdin are not available.
//!   - If stderr is not a PTY.
//!
//! # Example Output
//!
//! `cargo run -p clap-couture --example 03-interactive` outputs:
//!
//! ```terminal
//! ◇  Application name
//! │  web
//! │
//! ◇  Tail the logs afterwards?
//! │  Yes
//! │
//! ◇  Which region?
//! │  eu
//! │
//! ◇  How many replicas?
//! │  abc
//! │
//! ■  invalid value 'abc' for '--replicas <REPLICAS>': invalid digit found in string
//! │
//! ◇  How many replicas?
//! │  3
//! │
//! ◇  Which sidecars?
//! │  logging
//! │  proxy
//! │
//! ◇  Environment variables (KEY=VALUE)?
//! │  LOG=debug
//! │
//! ◇  Environment variables (KEY=VALUE)?
//! │  PORT=8080
//! │
//! ◇  Environment variables (KEY=VALUE)?
//! │  (<enter> to finish)
//! │
//! deploying web to eu with 3 replicas (follow: true)
//! sidecars: [Logging, Proxy], vars: ["LOG=debug", "PORT=8080"]
//! ```
//!
//! By default, this renders with [cliclack]. You can also choose different interactive input
//! backends, by passing one of `--feature ...`.
//!
//! `cargo run -p clap-couture --no-default-features --features interactive-inquire --example
//! 03-interactive` outputs:
//!
//! ```terminal
//! > Application name web
//! > Tail the logs afterwards? Yes
//! > Which region? eu
//! > How many replicas? abc
//! invalid value 'abc' for '--replicas <REPLICAS>': invalid digit found in string
//! > How many replicas? 3
//! > Which sidecars? logging, proxy
//! > Environment variables (KEY=VALUE)? LOG=debug
//! > Environment variables (KEY=VALUE)? PORT=8080
//! > Environment variables (KEY=VALUE)?
//! deploying web to eu with 3 replicas (follow: true)
//! sidecars: [Logging, Proxy], vars: ["LOG=debug", "PORT=8080"]
//! ```
//!
//! `interactive-dialoguer` is the last option.
//!
//! `cargo run -p clap-couture --no-default-features --features interactive-dialoguer --example
//! 03-interactive` outputs:
//!
//! ```terminal
//! ✔ Application name · web
//! ✔ Tail the logs afterwards? · yes
//! ✔ Which region? · apac
//! ✔ How many replicas? · abc
//! invalid value 'abc' for '--replicas <REPLICAS>': invalid digit found in string
//! ✔ How many replicas? · 3
//! ✔ Which sidecars? · logging, proxy
//! ✔ Environment variables (KEY=VALUE)? (<enter> to finish) · LOG=debug
//! ✔ Environment variables (KEY=VALUE)? (<enter> to finish) · PORT=8080
//! ✔ Environment variables (KEY=VALUE)? (<enter> to finish) ·
//! deploying web to apac with 3 replicas (follow: true)
//! sidecars: [Logging, Proxy], vars: ["LOG=debug", "PORT=8080"]
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]
#![expect(clippy::use_debug, reason = "lists print shortest as their `Debug`")]

use clap::{Parser, ValueEnum};
use clap_couture::{Couture, CoutureParser as _};

#[derive(Parser, Couture)]
struct Cli {
    /// Application name.
    #[arg(long)]
    // You can add a `#[couture(prompt)]` annotation to any clap field which results in a prompt
    // for the user. It respects rsdoc, and will use the rsdoc as the prompt, by default.
    #[couture(prompt)]
    app: String,

    /// Enable log following.
    #[arg(long)]
    // You can pass arbitrary prompts if you wish to customize the input prompt.
    #[couture(prompt = "Tail the logs afterwards?")]
    follow: bool,

    /// Application region.
    #[arg(long, value_enum)]
    // clap-couture also supports enums, showing a multiple-choice prompt.
    #[couture(prompt = "Which region?")]
    region: Region,

    /// Replica count.
    #[arg(long, default_value_t = 2)]
    // clap-couture will pre-fill defaults in the input field. This allows the user to quickly hit
    // <enter> and use the default.
    #[couture(prompt = "How many replicas?")]
    replicas: u16,

    /// Sidecars to run next to the app.
    #[arg(long, value_enum)]
    // A `Vec` of an enum shows a multi-select: <space> picks an option, <enter> confirms.
    #[couture(prompt = "Which sidecars?")]
    sidecars: Vec<Sidecar>,

    /// Environment variables to set.
    #[arg(long)]
    // A `Vec` of anything else asks one value at a time, until the user hits <enter> on an empty
    // answer.
    #[couture(prompt = "Environment variables (KEY=VALUE)?")]
    vars: Vec<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Region {
    Apac,
    Eu,
    Us,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Sidecar {
    Logging,
    Metrics,
    Proxy,
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
    println!("sidecars: {:?}, vars: {:?}", cli.sidecars, cli.vars);
}
