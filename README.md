<div align="center">

<h1>clap-couture</h1>
<strong>Haute couture for your command line.</strong><br>

<p>
  <a href="https://crates.io/crates/clap-couture"><img alt="crates.io" src="https://img.shields.io/crates/v/clap-couture?style=flat-square&color=DB2777"></a>
  <a href="https://docs.rs/clap-couture"><img alt="docs.rs" src="https://img.shields.io/docsrs/clap-couture?style=flat-square&color=DB2777"></a>
  <a href="https://github.com/markovejnovic/clap-couture/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/markovejnovic/clap-couture/ci.yml?branch=main&style=flat-square&label=CI"></a>
  <a href="#installation"><img alt="MSRV 1.85" src="https://img.shields.io/badge/MSRV-1.85-DB2777?style=flat-square"></a>
  <a href="https://crates.io/crates/clap"><img alt="clap 4.x" src="https://img.shields.io/badge/clap-4.x-DB2777?style=flat-square"></a>
  <a href="LICENSE"><img alt="MIT" src="https://img.shields.io/badge/license-MIT-DB2777?style=flat-square"></a>
</p>


</div>

**clap-couture** is a [`clap`](https://docs.rs/clap) extension that beautifies
your `clap` with only a few derive attributes.

## Features

- Organize your help with _categories_.
- Interactive input (WIP)
- Markdown for rich styling (WIP)

#### Categories

`clap-couture` allows you to provide a set of categories to better improve the
`--help` experience your users see. As your project gets larger, the list of
subcommands makes your CLI intimidating and we don't like that.

With `clap-couture`, you can create categories:

```rust
#[derive(Subcommand, Couture)]
#[couture(categories = {
    "everyday" = { title = "Everyday",
                   description = "The commands you'll reach for daily" },
    "deploy"   = { title = "Deploy",
                   description = "Ship code to production" },
    "account"  = { title = "Account & sync" },
})]
enum Cmd {
    #[category("deploy")] Deploy,
    #[category("account")] Login,
    #[category("everyday")] Logs,
    #[category("deploy")] Rollback,
    #[category("everyday")] Search,
    #[category("deploy")] Secrets,
    #[category("everyday")] Status,
    #[category("account")] Sync,
}
```

which will generate beautiful `--help`:

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

## Quick Start

Run

```sh
cargo add clap-couture
# markdown styling in descriptions:
cargo add clap-couture --features markdown
```

clap-couture targets **clap 4.x** and Rust **1.85+** (edition 2024).

## License

[MIT](LICENSE).
