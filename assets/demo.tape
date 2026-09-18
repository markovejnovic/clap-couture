# Regenerate the animated demo:
#   brew install vhs        # https://github.com/charmbracelet/vhs
#   vhs assets/demo.tape    # writes assets/demo.gif
#
# Prefer the static hero? assets/demo.svg is committed and used by the README.

Output assets/demo.gif

Require cargo

Set Shell bash
Set FontSize 22
Set Width 1080
Set Height 820
Set Theme "Catppuccin Mocha"
Set Padding 26
Set WindowBar Colorful
Set BorderRadius 12
Set TypingSpeed 55ms

# Build the demo binary off-camera and alias it to `orbit`.
Hide
Type "cargo build -q -p clap-couture --example orbit --features markdown"
Enter
Wait
Type "alias orbit='./target/debug/examples/orbit'"
Enter
Type "clear"
Enter
Show

Sleep 700ms
Type "orbit --help"
Sleep 800ms
Enter
Sleep 5s
