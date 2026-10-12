# `orbit --help`, then `orbit deploy` asking for everything left out. See
# `scripts/record-examples.py`.

Set Width 1692
Set Height 1766

Hide
Type "unset VISUAL ORBIT_TOKEN && export EDITOR=vim && alias orbit=99-full-example && clear" Enter
Show

Type "orbit --help" Sleep 500ms Enter
Wait+Screen /Print help/
Sleep 4s
Type "clear" Enter

Type "orbit deploy" Sleep 400ms Enter
Wait+Screen /Application name/
Sleep 600ms
Type "web" Sleep 400ms Enter

Wait+Screen /These lines are removed/
Sleep 1.5s
# Writes under the initial heading; the instructions are dropped on save.
Type "ggo" Sleep 300ms Type "- Ships the new billing page." Sleep 600ms Escape
Sleep 600ms
Type ":wq" Sleep 400ms Enter

Wait+Screen /Which region\?/
Sleep 800ms
Down Sleep 500ms Enter

Wait+Screen /How many replicas\?/
Sleep 800ms
Type "abc" Sleep 400ms Enter
Wait+Screen /invalid value/
Sleep 1.5s
Type "3" Sleep 400ms Enter

Wait+Screen /Which sidecars\?/
Sleep 800ms
# <space> picks the option under the cursor, <enter> confirms the picks.
Space Sleep 400ms Down Sleep 400ms Down Sleep 400ms Space Sleep 600ms Enter

Wait+Screen /Deploy token/
Sleep 800ms
# Masked as it's typed: the recording shows only the mask.
Type "hunter2" Sleep 600ms Enter

Wait+Screen /Deploy web to eu\?/
Sleep 800ms
# cliclack's confirm takes `y` as the answer, without Enter.
Type "y"

Wait+Screen /deploying web/
Sleep 3s
