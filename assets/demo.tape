# Regenerate the animated demo:
#   brew install vhs ffmpeg   # https://github.com/charmbracelet/vhs
#   vhs assets/demo.tape      # writes assets/demo.gif
#
# The README hero uses the static freeze renders (assets/before.svg, after.svg),
# regenerated with assets/render.sh — that's the primary, reliable path.
#
# NOTE: VHS 0.12 currently no-ops on this machine — it prints "Creating ...gif",
# exits 0, and writes nothing (Chrome/ttyd/ffmpeg are all present). This tape is
# kept for environments where VHS works (CI, a fixed install) or the animated cut.

Output assets/demo.gif

Require cargo

Set Shell bash
Set FontSize 22
Set Width 1000
Set Height 800
Set Theme "Catppuccin Mocha"
Set Padding 26
Set WindowBar Colorful
Set BorderRadius 12
Set TypingSpeed 55ms

# Build + alias off-camera in one shot, then clear. A single Sleep covers the
# build (cached builds are near-instant); avoid `Wait`, which aborts the render
# when a quiet build prints nothing to match against.
Hide
Type "cargo build -q -p clap-couture --example orbit --features markdown && alias orbit='./target/debug/examples/orbit' && clear"
Enter
Sleep 10s
Show

Sleep 600ms
Type "orbit --help"
Sleep 800ms
Enter
Sleep 5s
