# `03-interactive.tape` for the inquire backend, whose confirm needs Enter. See `scripts/record-examples.py`.

Set Height 960

Hide
Type "alias orbit-deploy=03-interactive && clear" Enter
Show

Type "orbit-deploy" Sleep 400ms Enter
Wait+Screen /Application name/
Sleep 600ms
Type "web" Sleep 400ms Enter

Wait+Screen /Tail the logs/
Sleep 800ms
# inquire's confirm is a text field: `y`, then Enter.
Type "y" Sleep 400ms Enter

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

Wait+Screen /Environment variables/
Sleep 800ms
Type "LOG=debug" Sleep 400ms Enter
Wait+Screen /to finish/
Sleep 800ms
Type "PORT=8080" Sleep 400ms Enter
Sleep 1s
# <enter> on an empty answer finishes the list.
Enter

Wait+Screen /deploying web/
Sleep 3s
