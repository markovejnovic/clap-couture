# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/markovejnovic/clap-couture/releases/tag/v0.1.0) - 2026-10-12

### Added

- Render markdown in prompt questions ([#58](https://github.com/markovejnovic/clap-couture/pull/58))
- Let users turn prompts off with NO_PROMPT or CI ([#48](https://github.com/markovejnovic/clap-couture/pull/48))
- Make markdown help opt-in ([#44](https://github.com/markovejnovic/clap-couture/pull/44))
- Give `-` a meaning with `prompt(dash)` and `stdin(dash)` ([#21](https://github.com/markovejnovic/clap-couture/pull/21))
- Read piped stdin into a #[couture(accept_stdin)] arg ([#18](https://github.com/markovejnovic/clap-couture/pull/18))
- [**breaking**] start editor prompts with initial text ([#16](https://github.com/markovejnovic/clap-couture/pull/16))
- Expose the prompter to users ([#13](https://github.com/markovejnovic/clap-couture/pull/13))
- masked secret prompts with #[couture(prompt, secret)] ([#8](https://github.com/markovejnovic/clap-couture/pull/8))
- ask prompted args in $EDITOR with #[couture(prompt, editor)] ([#10](https://github.com/markovejnovic/clap-couture/pull/10))
- Support confirmation flags ([#12](https://github.com/markovejnovic/clap-couture/pull/12))
- Support prompts for multi-value args ([#11](https://github.com/markovejnovic/clap-couture/pull/11))
- Support for clap-style i18n ([#7](https://github.com/markovejnovic/clap-couture/pull/7))
- Support #[command(flatten)] ([#6](https://github.com/markovejnovic/clap-couture/pull/6))

### Changed

- Render couture's help only when it's printed ([#53](https://github.com/markovejnovic/clap-couture/pull/53))

### Fixed

- Keep the indent clap leaves after an arg without help ([#65](https://github.com/markovejnovic/clap-couture/pull/65))
- List subcommands with the rows clap writes for them ([#69](https://github.com/markovejnovic/clap-couture/pull/69))
- Leave a flattened help to clap ([#68](https://github.com/markovejnovic/clap-couture/pull/68))
- List no `help` subcommand when clap lists no subcommands ([#66](https://github.com/markovejnovic/clap-couture/pull/66))
- Name a subcommand by its last `name`, as clap does ([#62](https://github.com/markovejnovic/clap-couture/pull/62))
- Style a subcommand's usage heading with the styles clap hands down ([#64](https://github.com/markovejnovic/clap-couture/pull/64))
- Run the app's SIGINT handler when a prompt is cancelled ([#50](https://github.com/markovejnovic/clap-couture/pull/50))
- Restore terminal echo when a dialoguer secret prompt is interrupted ([#52](https://github.com/markovejnovic/clap-couture/pull/52))
- Leave the app's SIGTERM handler alone while the editor opens ([#49](https://github.com/markovejnovic/clap-couture/pull/49))
- Better match clap's behavior, organize nested help ([#31](https://github.com/markovejnovic/clap-couture/pull/31))
- Name the binary, not the crate, in couture's errors ([#36](https://github.com/markovejnovic/clap-couture/pull/36))
- Only strip the editor header couture wrote ([#37](https://github.com/markovejnovic/clap-couture/pull/37))
- Don't accept editor text that couldn't be decoded ([#38](https://github.com/markovejnovic/clap-couture/pull/38))
- Hold prompted values to the arg's num_args ([#40](https://github.com/markovejnovic/clap-couture/pull/40))
- Keep placeholders, links, and paragraph breaks in markdown help ([#28](https://github.com/markovejnovic/clap-couture/pull/28))
- Reject flag actions and nested options the derive can't prompt for ([#29](https://github.com/markovejnovic/clap-couture/pull/29))
- Prompt through boxed and generic args ([#27](https://github.com/markovejnovic/clap-couture/pull/27))
- Read stdin on `-` under `stdin(piped)` too ([#26](https://github.com/markovejnovic/clap-couture/pull/26))
- Remove the editor's draft when terminated ([#42](https://github.com/markovejnovic/clap-couture/pull/42))
- Let cliclack prompts shorten or clear a list's defaults ([#43](https://github.com/markovejnovic/clap-couture/pull/43))
- Stabilize the API, and fix help, stdin and derive edge cases ([#23](https://github.com/markovejnovic/clap-couture/pull/23))
- unbreak Render examples workflow ([#14](https://github.com/markovejnovic/clap-couture/pull/14))

### Other

- Interactive CLI ([#3](https://github.com/markovejnovic/clap-couture/pull/3))
- better README + clippy
- Initial commit

### Security

- Keep secrets out of confirmations and usage ([#30](https://github.com/markovejnovic/clap-couture/pull/30))
- Keep the editor's temp file private ([#33](https://github.com/markovejnovic/clap-couture/pull/33))
