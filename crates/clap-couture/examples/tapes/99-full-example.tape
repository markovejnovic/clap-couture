# One scene per feature: categorized markdown `--help`, prompts, and secrets masked, from the
# environment, and piped. See `scripts/record-examples.py`.

Set Width 1692
Set Height 1766

Hide
# A function, not an alias, so `ORBIT_TOKEN=... orbit` works.
Type `unset VISUAL ORBIT_TOKEN && export EDITOR=vim && orbit() { 99-full-example "$@"; } && clear` Enter
Show

Type "orbit --help" Sleep 500ms Enter
Wait+Screen /Print help/
Sleep 5s
Hide
Type "clear" Enter
Show

Type "orbit deploy --help" Sleep 500ms Enter
Wait+Screen /ORBIT_TOKEN/
Sleep 4s
Hide
Type "clear" Enter
Show

# `--app` is passed and the token comes from the environment, so neither is asked.
Type "ORBIT_TOKEN=s3cr3t orbit deploy --app web" Sleep 500ms Enter
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
Sleep 1s
# <enter> takes the pre-filled default.
Enter

Wait+Screen /Which sidecars\?/
Sleep 800ms
# <space> picks the option under the cursor, <enter> confirms the picks.
Space Sleep 400ms Down Sleep 400ms Down Sleep 400ms Space Sleep 600ms Enter

Wait+Screen /Deploy web to eu\?/
Sleep 800ms
# cliclack's confirm takes `y` as the answer, without Enter.
Type "y"

Wait+Screen /deploying web/
Sleep 3s
Hide
Type "clear" Enter
Show

Type "orbit login" Sleep 500ms Enter
Wait+Screen /Your email/
Sleep 600ms
Type "me@orbit.dev" Sleep 400ms Enter

Wait+Screen /Your password/
Sleep 800ms
# Masked as it's typed: the recording shows only the mask.
Type "hunter2" Sleep 1.5s Enter

Wait+Screen /signed in as/
Sleep 2s

# The same login, with the password piped in: nothing is asked.
Type "echo hunter2 | orbit login --email me@orbit.dev --password -" Sleep 500ms Enter
Wait+Screen /signed in as me@orbit.dev\n(.|\n)*signed in as/
Sleep 3s
