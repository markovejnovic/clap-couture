# `deploy` asking for its release notes in `$EDITOR`. See `scripts/record-examples.py`.

Set Height 620

Hide
Type "unset VISUAL && export EDITOR=vim && alias deploy=06-interactive-editor && clear" Enter
Show

Type "deploy" Sleep 400ms Enter
Wait+Screen /Tail the logs/
Sleep 800ms
# cliclack's confirm takes `y` as the answer, without Enter.
Type "y"

Wait+Screen /These lines are removed/
Sleep 1.5s
# Writes above the instructions, which are dropped on save.
Type "ggO" Sleep 300ms Type "Ships the new billing page." Sleep 600ms Escape
Sleep 600ms
Type ":wq" Sleep 400ms Enter

Wait+Screen /Region to deploy to/
Sleep 800ms
Type "us-east" Sleep 400ms Enter

Wait+Screen /How to roll the release out/
Sleep 800ms
Enter

Wait+Screen /deploying to us-east/
Sleep 3s
