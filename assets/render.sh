#!/usr/bin/env bash
# Regenerate the README hero images with charmbracelet/freeze.
#
#   brew install charmbracelet/tap/freeze   # https://github.com/charmbracelet/freeze
#   ./assets/render.sh
set -euo pipefail
cd "$(dirname "$0")/.."

freeze_opts=(
  --language ansi
  --window
  --background "#1e1e2e"
  --padding 30 --margin 64
  --border.radius 10
  --shadow.blur 24 --shadow.x 0 --shadow.y 16
  --font.size 14 --line-height 1.4
)

bin=./target/debug/examples/orbit
cargo build -q -p clap-couture --example orbit --features markdown

CLICOLOR_FORCE=1 "$bin" --help | freeze "${freeze_opts[@]}" --output assets/after.svg
ORBIT_PLAIN=1 CLICOLOR_FORCE=1 "$bin" | freeze "${freeze_opts[@]}" --output assets/before.svg

echo "wrote assets/before.svg and assets/after.svg"
