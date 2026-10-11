# `orbit-create` asking which engine only after a yes to a database. See `scripts/record-examples.py`.

Set Height 440

Hide
Type "alias orbit-create=06-interactive-prompter && clear" Enter
Show

Type "orbit-create" Sleep 400ms Enter
Wait+Screen /Application name/
Sleep 600ms
Type "web" Sleep 400ms Enter

Wait+Screen /Add a database\?/
Sleep 1s
# cliclack's confirm takes `y` as the answer, without Enter.
Type "y"

Wait+Screen /Which engine\?/
Sleep 800ms
Down Sleep 500ms Enter

Wait+Screen /creating web with postgres/
Sleep 3s
