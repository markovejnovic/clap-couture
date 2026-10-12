#!/usr/bin/env bash
# Capture examples' `--help` to `assets/<example>.ansi`, and convert it to the rustdoc block
# `crates/clap-couture/docs/<example>.html`. The README's pictures of it are recorded by
# `scripts/record-examples.py`, like the GIFs, so they look alike.
#
#   scripts/render-examples.sh [example...]
#
# The `.ansi` captures are identical on every OS, so CI diffs those to tell whether the blocks are
# stale.
set -euo pipefail
cd "$(dirname "$0")/.."

if (($# > 0)); then
  examples=("$@")
else
  examples=()
  for path in crates/clap-couture/examples/*.rs; do
    name=${path##*/}
    name=${name%.rs}
    examples+=("$name")
  done
fi

cargo build -q -p clap-couture --all-features "${examples[@]/#/--example=}"

for example in "${examples[@]}"; do
  ansi="assets/$example.ansi"
  # Run it as its CLI, e.g. `orbit` for `01-orbit-organized`, since clap's usage line shows argv[0].
  display=${example#*-}
  display=${display%%-*}
  CLICOLOR_FORCE=1 bash -c 'exec -a "$0" "$1" --help' "$display" "./target/debug/examples/$example" >"$ansi"
  echo "wrote $ansi"
  block="crates/clap-couture/docs/$example.html"
  ./scripts/ansi-to-html.py <"$ansi" >"$block"
  echo "wrote $block"
done
