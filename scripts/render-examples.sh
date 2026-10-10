#!/usr/bin/env bash
# Capture examples' `--help` to `assets/<example>.ansi`, then render it to
# `assets/<example>-{dark,light}.svg` with charmbracelet/freeze.
#
#   scripts/render-examples.sh [--ansi-only] [example...]
#
# The `.ansi` captures are identical on every OS, unlike freeze's SVGs, so CI diffs those to tell
# whether the images are stale. `--ansi-only` skips freeze, for that check.
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

# The dark render uses Catppuccin Mocha's base; the light render swaps each fill for its Latte twin.
mocha_to_latte=(
  "1e1e2e:eff1f5" # base, the window background
  "c4c4c4:4c4f69" # freeze's default text -> Latte text
)
light_sed=()
for pair in "${mocha_to_latte[@]}"; do
  light_sed+=(-e "s/fill=\"#${pair%%:*}\"/fill=\"#${pair##*:}\"/g")
done

ansi_only=false
if [[ ${1-} == --ansi-only ]]; then
  ansi_only=true
  shift
fi

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
  if $ansi_only; then
    continue
  fi

  dark="assets/$example-dark.svg"
  light="assets/$example-light.svg"
  freeze "${freeze_opts[@]}" --output "$dark" <"$ansi"
  sed "${light_sed[@]}" "$dark" >"$light"
  echo "wrote $dark and $light"
done
