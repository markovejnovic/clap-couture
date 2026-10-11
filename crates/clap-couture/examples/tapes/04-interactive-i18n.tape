# `04-interactive-i18n` asking in German. See `scripts/record-examples.py`.

Set Width 1246
Set Height 866
Env ORBIT_LANG "de"

Type "04-interactive-i18n" Sleep 400ms Enter
Wait+Screen /Welche App\?/
Sleep 600ms
Type "web" Sleep 400ms Enter

Wait+Screen /Logs verfolgen\?/
Sleep 800ms
# cliclack's confirm takes `y` as the answer, without Enter.
Type "y"

Wait+Screen /In welche Region\?/
Sleep 800ms
# The last choice leaves the optional region unset: "(keine)".
Down 3 Sleep 500ms Enter

Wait+Screen /deploying web/
Sleep 3s
