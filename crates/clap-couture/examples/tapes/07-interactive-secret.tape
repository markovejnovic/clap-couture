# `orbit-deploy` asking for its token without echoing it. See `scripts/record-examples.py`.

Set Height 520

Hide
Type "alias orbit-deploy=07-interactive-secret && clear" Enter
Show

Type "orbit-deploy" Sleep 400ms Enter
Wait+Screen /Application name/
Sleep 600ms
Type "web" Sleep 400ms Enter

Wait+Screen /Deploy token/
Sleep 800ms
# Masked as it's typed: the recording shows only the mask.
Type "hunter2" Sleep 600ms Enter

Wait+Screen /deploying web/
Sleep 3s
