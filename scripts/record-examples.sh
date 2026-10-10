#!/usr/bin/env bash
# Record interactive examples from `scripts/tapes/<example>.tape` with charmbracelet/vhs, to
# `assets/<example>-{dark,light}.gif`, plus `assets/<example>.txt`: the recording's screens as
# text.
#
#   scripts/record-examples.sh [example...]
#
# The `.txt` captures are the same on every run, unlike the GIFs, so CI diffs those to tell
# whether the GIFs are stale.
set -euo pipefail
cd "$(dirname "$0")/.."

themes=(
  "dark:Catppuccin Mocha"
  "light:Catppuccin Latte"
)

if (($# > 0)); then
  examples=("$@")
else
  examples=()
  for path in scripts/tapes/*.tape; do
    name=${path##*/}
    examples+=("${name%.tape}")
  done
fi

cargo build -q -p clap-couture --all-features "${examples[@]/#/--example=}"
# Tapes run the examples by name; each aliases its example to the CLI's own name.
export PATH="$PWD/target/debug/examples:$PATH"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

for example in "${examples[@]}"; do
  for pair in "${themes[@]}"; do
    variant=${pair%%:*}
    tape="$tmp/$example-$variant.tape"
    {
      echo "Output \"assets/$example-$variant.gif\""
      # The text capture ignores colors, so one theme's is enough.
      [[ $variant == dark ]] && echo "Output \"assets/$example.txt\""
      echo "Set Theme \"${pair#*:}\""
      cat "scripts/tapes/$example.tape"
    } >"$tape"
    vhs -q "$tape"
    echo "wrote assets/$example-$variant.gif"
  done
done
