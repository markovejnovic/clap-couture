#!/usr/bin/env -S uv run
"""Convert an ANSI capture to a rustdoc `<pre>` block, colored by rustdoc's own theme.

    scripts/ansi-to-html.py < CAPTURE.ansi > BLOCK.html

Styles become inline `style` attributes and no colors are set, so the text takes
rustdoc's colors in every theme.
"""

import sys

from ansi2html import Ansi2HTMLConverter

if __name__ == "__main__":
    body = Ansi2HTMLConverter(inline=True).convert(sys.stdin.read(), full=False)
    _ = sys.stdout.write(f"<pre><code>{body}</code></pre>\n")
