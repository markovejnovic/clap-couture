//! Every clap-couture feature in one CLI: categorized, markdown `--help` in your own clap styles,
//! and prompts for whatever the user leaves out, from a picker to a masked token to `$EDITOR`.
//!
//! ```sh
//! cargo run -p clap-couture --example 99-full-example --features markdown -- --help
//! cargo run -p clap-couture --example 99-full-example --features markdown -- deploy
//! ```

#![expect(clippy::print_stdout, reason = "a runnable demo prints its result")]
#![expect(clippy::use_debug, reason = "lists print shortest as their `Debug`")]

use clap::{
    Parser, Subcommand, ValueEnum,
    builder::{
        Styles,
        styling::{AnsiColor, Effects},
    },
};
use clap_couture::{Couture, CoutureParser as _, interactive::secrecy::SecretString};

// couture draws `--help` and its prompts in the command's clap styles.
const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Magenta.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Magenta.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Blue.on_default())
    .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
    .valid(AnsiColor::Green.on_default())
    .invalid(AnsiColor::Yellow.on_default());

/// Deploy and manage your apps from the **terminal**.
#[derive(Parser, Couture)]
#[command(name = "orbit", bin_name = "orbit", styles = STYLES)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture)]
#[couture(categories = {
    // Descriptions are markdown, with the `markdown` feature.
    "ship" = { title = "Ship:", description = "Get code to **production**" },
    "account" = { title = "Account:", description = "Who *you* are, and what you keep" },
})]
enum Cmd {
    /// Ship the current project.
    #[couture(category = "ship")]
    Deploy(Deploy),
    /// Sign in to your account.
    #[couture(category = "account")]
    Login(Login),
    // No category: listed first, under no heading.
    /// Tail live logs.
    Logs,
    /// Roll back to a previous release.
    #[couture(category = "ship")]
    Rollback(Rollback),
    // `orbit secrets --help` is categorized too, by the categories on `Secrets`.
    /// Manage environment secrets.
    #[couture(category = "account")]
    #[command(subcommand)]
    Secrets(Secrets),
    /// Show recent activity.
    Status,
}

#[derive(Parser, Couture)]
struct Deploy {
    /// Application name.
    #[arg(long)]
    // Asks the doc comment.
    #[couture(prompt)]
    app: String,

    /// Release notes.
    #[arg(long)]
    // Written in the user's `$EDITOR`, like `git commit`.
    #[couture(prompt, editor(extension = "md", initial = "## Changes\n"))]
    notes: Option<String>,

    /// Region to deploy to.
    #[arg(long, value_enum)]
    // A `ValueEnum` is picked from a list.
    #[couture(prompt = "Which region?")]
    region: Region,

    /// Replica count.
    #[arg(long, default_value_t = 2)]
    // Pre-filled with the default, and parsed by clap: `abc` gets clap's error, and the question
    // again.
    #[couture(prompt = "How many replicas?")]
    replicas: u16,

    /// Sidecars to run next to the app.
    #[arg(long, value_enum)]
    // A `Vec` of a `ValueEnum` is a multi-select.
    #[couture(prompt = "Which sidecars?")]
    sidecars: Vec<Sidecar>,

    /// Deploy token.
    #[arg(long, env = "ORBIT_TOKEN")]
    // Masked as it's typed, refused on the command line, and `--token -` reads it from stdin.
    #[couture(prompt, secret, stdin)]
    token: SecretString,

    /// Skip the confirmation.
    #[arg(long)]
    // Asked last, naming the answers above.
    #[couture(confirm = "Deploy {app} to {region}?")]
    yes: bool,
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

#[derive(Parser, Couture)]
struct Rollback {
    /// Release to roll back to; `-` asks for it.
    #[arg(long)]
    #[couture(prompt(dash, question = "Roll back to which release?"))]
    to: String,
}

#[derive(Parser, Couture)]
struct Login {
    /// Your email.
    #[arg(long)]
    #[couture(prompt)]
    email: String,

    /// Your password; `-` reads it from stdin, like `docker login --password-stdin`.
    #[arg(long)]
    #[couture(prompt, secret, stdin)]
    password: SecretString,
}

#[derive(Subcommand, Couture)]
#[couture(categories = { "read" = { title = "Read:" }, "write" = { title = "Write:" } })]
enum Secrets {
    /// List the secrets' names.
    #[couture(category = "read")]
    List,
    /// Remove a secret.
    #[couture(category = "write")]
    Remove {
        /// The secret's name.
        name: String,
    },
    /// Set a secret.
    #[couture(category = "write")]
    Set(SetSecret),
}

#[derive(Parser, Couture)]
struct SetSecret {
    /// The secret's name.
    #[arg(long)]
    #[couture(prompt)]
    name: String,

    /// The secret's value.
    #[arg(long)]
    // An optional secret: left empty, it stays `None`.
    #[couture(prompt, secret)]
    value: Option<SecretString>,
}

fn main() {
    let cli = Cli::couture_parse();

    match cli.cmd {
        Cmd::Status => println!("all systems go"),
        Cmd::Logs => println!("tailing logs"),
        Cmd::Deploy(deploy) => {
            // A real deploy hands the token on; the demo only says it arrived.
            let _token: SecretString = deploy.token;
            println!(
                "deploying {} to {:?} with {} replicas and {:?}",
                deploy.app, deploy.region, deploy.replicas, deploy.sidecars
            );
            if let Some(notes) = deploy.notes {
                println!("{notes}");
            }
        }
        Cmd::Rollback(rollback) => println!("rolling back to {}", rollback.to),
        Cmd::Login(login) => println!("signed in as {}", login.email),
        Cmd::Secrets(Secrets::List) => println!("no secrets yet"),
        Cmd::Secrets(Secrets::Set(secret)) => match secret.value {
            Some(_) => println!("set {}", secret.name),
            None => println!("left {} empty", secret.name),
        },
        Cmd::Secrets(Secrets::Remove { name }) => println!("removed {name}"),
    }
}
