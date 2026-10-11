# `orbit-deploy` answering each kind of prompt. See `scripts/record-examples.py`.

Set Height 640

Hide
Type "alias orbit-deploy=03-interactive && clear" Enter
Show

Type "orbit-deploy" Sleep 400ms Enter
Wait+Screen /Application name/
Sleep 600ms
Type "web" Sleep 400ms Enter

Wait+Screen /Tail the logs/
Sleep 800ms
# cliclack's confirm takes `y` as the answer, without Enter.
Type "y"

Wait+Screen /Which region\?/
Sleep 800ms
Down Sleep 500ms Enter

Wait+Screen /How many replicas\?/
Sleep 800ms
Type "abc" Sleep 400ms Enter
Wait+Screen /invalid value/
Sleep 1.5s
Type "3" Sleep 400ms Enter

Wait+Screen /deploying web/
Sleep 3s
