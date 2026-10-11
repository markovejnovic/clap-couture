# `orbit-release` asking for its title on `--title -`, then reading its notes from a pipe on
# `--notes -`. See `scripts/record-examples.py`.

Set Height 320

Hide
Type "alias orbit-release=11-interactive-dash && clear" Enter
Show

Type "orbit-release --title -" Sleep 400ms Enter
Wait+Screen /Release title/
Sleep 600ms
Type "v1.2" Sleep 400ms Enter

# Answered: cliclack marks the question done with ◇.
Wait+Screen /releasing v1.2/
Sleep 2s

# Cleared, so the captured run starts at the next command.
Hide
Type "clear" Enter
Show

Type 'echo "Fix the login page" | orbit-release --title v1.2 --notes -' Sleep 400ms Enter
Wait+Screen /notes: Fix the login page/
Sleep 3s
