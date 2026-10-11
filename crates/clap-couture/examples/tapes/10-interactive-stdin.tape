# `orbit-release` taking its notes from a pipe, then asking for them when nothing is piped. See
# `scripts/record-examples.py`.

Set Width 1246
Set Height 566

Hide
Type "alias orbit-release=10-interactive-stdin && clear" Enter
Show

Type 'echo "Fix the login page" | orbit-release' Sleep 400ms Enter
Wait+Screen /releasing with notes/
Sleep 2s

# Cleared, so the captured run starts at the next command.
Hide
Type "clear" Enter
Show

Type "orbit-release" Sleep 400ms Enter
Wait+Screen /Release notes/
Sleep 600ms
Type "Fix the login page" Sleep 400ms Enter

# Answered: cliclack marks the question done with ◇.
Wait+Screen /◇  Release notes/
Sleep 3s
