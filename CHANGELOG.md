# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/markovejnovic/clap-couture/releases/tag/v0.1.0) - 2026-10-11

### Added

- Expose the prompter to users ([#13](https://github.com/markovejnovic/clap-couture/pull/13))
- masked secret prompts with #[couture(prompt, secret)] ([#8](https://github.com/markovejnovic/clap-couture/pull/8))
- ask prompted args in $EDITOR with #[couture(prompt, editor)] ([#10](https://github.com/markovejnovic/clap-couture/pull/10))
- Support confirmation flags ([#12](https://github.com/markovejnovic/clap-couture/pull/12))
- Support prompts for multi-value args ([#11](https://github.com/markovejnovic/clap-couture/pull/11))
- Support for clap-style i18n ([#7](https://github.com/markovejnovic/clap-couture/pull/7))
- Support #[command(flatten)] ([#6](https://github.com/markovejnovic/clap-couture/pull/6))

### Fixed

- unbreak Render examples workflow ([#14](https://github.com/markovejnovic/clap-couture/pull/14))

### Other

- Interactive CLI ([#3](https://github.com/markovejnovic/clap-couture/pull/3))
- better README + clippy
- Initial commit
