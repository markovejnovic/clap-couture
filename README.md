<div align="center">

<h1>clap-couture</h1>

<p>
  <strong>Haute couture for your command line.</strong><br>
  The most beautiful <code>clap</code> extension you've heard of.
</p>

<p>
  <a href="https://crates.io/crates/clap-couture"><img alt="crates.io" src="https://img.shields.io/crates/v/clap-couture?style=flat-square&color=DB2777"></a>
  <a href="https://docs.rs/clap-couture"><img alt="docs.rs" src="https://img.shields.io/docsrs/clap-couture?style=flat-square&color=DB2777"></a>
  <a href="https://github.com/markovejnovic/clap-couture/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/markovejnovic/clap-couture/ci.yml?branch=main&style=flat-square&label=CI"></a>
  <a href="#installation"><img alt="MSRV 1.85" src="https://img.shields.io/badge/MSRV-1.85-DB2777?style=flat-square"></a>
  <a href="https://crates.io/crates/clap"><img alt="clap 4.x" src="https://img.shields.io/badge/clap-4.x-DB2777?style=flat-square"></a>
  <a href="LICENSE"><img alt="MIT" src="https://img.shields.io/badge/license-MIT-DB2777?style=flat-square"></a>
</p>

<table>
  <tr>
    <td align="center"><sub><b>PLAIN CLAP</b></sub></td>
    <td align="center"><sub><b>WITH CLAP-COUTURE</b></sub></td>
  </tr>
  <tr>
    <td valign="top"><img src="assets/before.svg" alt="clap's default --help: one long, flat list of commands" width="430"></td>
    <td valign="top"><img src="assets/after.svg" alt="the same CLI with clap-couture: commands grouped into titled categories with descriptions" width="430"></td>
  </tr>
</table>

</div>

**clap-couture** is a [`clap`](https://docs.rs/clap) extension that turns a
boring flat `--help` into a categorized, beautifully organized command
menu---grouped sections, per-category descriptions, and Markdown styling, all
from a few derive attributes.

## Why

Every clap CLI starts elegant and, around the eighth subcommand, turns into a
wall. `Commands:` becomes an undifferentiated list where `login` sits next to
`import` sits next to `search`, and the reader has to squint to find the three
commands they actually use every day.

`clap-couture` gives you a **category**. You can group commands into titled
sections with custom descriptions, in a couple of derive attributes, at no
runtime cost, all with markdown formatting!

## Quickstart

```toml
# Cargo.toml
[dependencies]
clap = { version = "4", features = ["derive"] }
clap-couture = "0.1"
```

Add `Couture` next to clap's own derives, name your categories, and pin each
command to one:

```rust
use clap::{Parser, Subcommand};
use clap_couture::Couture;

#[derive(Parser, Couture)]
#[command(name = "orbit", about = "Deploy and manage your apps from the terminal.")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Couture)]
#[couture(categories = {
    "everyday" = { title = "Everyday", description = "The commands you'll reach for daily" },
    "deploy"   = { title = "Deploy",   description = "Ship code to production" },
    "account"  = { title = "Account & sync" },
})]
enum Cmd {
    /// Search your deployment history
    #[category("everyday")]
    Search,
    /// Show recent activity
    #[category("everyday")]
    Status,
    /// Ship the current project
    #[category("deploy")]
    Deploy,
    /// Roll back to a previous release
    #[category("deploy")]
    Rollback,
    /// Sign in to your account
    #[category("account")]
    Login,
}

fn main() {
    let cli = Cli::couture_parse(); // drop-in for `Cli::parse()`
    // ...dispatch on cli.cmd exactly as before
}
```

`couture_parse()` is a drop-in for clap's `parse()`. Now `orbit --help` reads like a
menu instead of a phone book:

```text
Deploy and manage your apps from the terminal.

Usage:
  orbit <COMMAND>

Everyday:        The commands you'll reach for daily
  search    Search your deployment history
  status    Show recent activity

Deploy:          Ship code to production
  deploy    Ship the current project
  rollback  Roll back to a previous release

Account & sync:
  login     Sign in to your account

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## Installation

```sh
cargo add clap-couture
# markdown styling in descriptions:
cargo add clap-couture --features markdown
```

clap-couture targets **clap 4.x** and Rust **1.85+** (edition 2024).

## License

[MIT](LICENSE).
