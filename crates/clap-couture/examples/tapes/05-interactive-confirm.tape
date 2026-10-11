# `orbit-destroy` confirming before it goes ahead. See `scripts/record-examples.py`.

Set Height 620

Hide
Type "alias orbit-destroy=05-interactive-confirm && clear" Enter
Show

Type "orbit-destroy" Sleep 400ms Enter
Wait+Screen /Application name/
Sleep 600ms
Type "web" Sleep 400ms Enter

Wait+Screen /Which region\?/
Sleep 800ms
Down Sleep 500ms Enter

Wait+Screen /Destroy web in eu\?/
Sleep 1.5s
# cliclack's confirm takes `y` as the answer, without Enter.
Type "y"

Wait+Screen /destroying web/
Sleep 3s
