#!/usr/bin/env -S uv run
"""Record examples with vhs, from their package's `examples/tapes/<example>.tape`.

    scripts/record-examples.py [EXAMPLE | 'cargo run ... --example EXAMPLE']...

An example name records it with its default features: its GIFs and its text capture.
A `cargo run` command records the example built the way it says, e.g. with another
prompt backend; `example_recordings` says where each recording goes. Without
arguments, every example is recorded with its default features.

A tape holds only its `Set Width`, `Set Height` and what to type; this script adds the
outputs, the theme, the window and the settings every recording shares, so the GIFs and
the stills all look alike. The examples are on PATH, by name.

The `.txt` captures are the same on every run, unlike the GIFs, so CI diffs those to
tell whether the GIFs are stale.
"""

import json
import os
import re
import tempfile
from collections.abc import Mapping
from pathlib import Path

import click

from devtools.backdrop import Window
from devtools.rust.cargo import Cargo
from devtools.vhs import Setting, Tape, Vhs, VhsManager
from example_recordings import PACKAGE, ROOT, THEMES, ExampleRun

# Twice the size the docs show them at, so they're sharp on high-DPI screens.
WINDOW = Window(margin=64, radius=20)
SETTINGS: dict[str, Setting] = {
    "Shell": "bash",
    "FontFamily": "JetBrains Mono",
    "FontSize": 28,
    "LineHeight": 1.2,
    "Padding": 32,
    "Margin": WINDOW.margin,
    "BorderRadius": WINDOW.radius,
    "WindowBar": "Colorful",
    "WindowBarSize": 56,
}
# A tape's own `Set Width 1200`, which its backdrop is drawn to.
TAPE_SIZE = re.compile(
    r"^Set (?P<setting>Width|Height) (?P<pixels>\d+)$", re.MULTILINE
)
# vhs rejects a quoted duration, and `Tape` quotes every string setting, so this one
# is a command.
TYPING_SPEED = "Set TypingSpeed 75ms"
# The variables that turn couture's prompts off, which the tapes record; CI sets `CI`.
PROMPT_OPT_OUTS = frozenset({"CI", "NO_PROMPT"})


@click.command()
@click.argument("arguments", nargs=-1, metavar="[EXAMPLE | 'cargo run ...']...")
def main(arguments: tuple[str, ...]) -> None:
    """Record the runs named on the command line, or every example."""
    with Cargo() as cargo:
        package = cargo.workspace(ROOT).package(PACKAGE)
        tapes = package.directory / "examples" / "tapes"
        runs = [
            ExampleRun.from_argument(argument, package.examples)
            for argument in arguments
        ] or [
            ExampleRun(
                example=name,
                package=PACKAGE,
                beyond_defaults=example.beyond_defaults,
            )
            for name, example in sorted(package.examples.items())
        ]

        missing = [
            tape for tape in (run.tape(tapes) for run in runs) if not tape.exists()
        ]
        if missing:
            listed = (f"missing tape: {tape.relative_to(ROOT)}" for tape in missing)
            raise click.ClickException("\n".join(listed))

        executables = {
            run: package.examples[run.example].build(
                features=run.cargo_features, target_dir=run.target_dir
            )
            for run in runs
        }

    with Vhs() as vhs, tempfile.TemporaryDirectory(prefix="backdrops-") as backdrops:
        for run, executable in executables.items():
            env = {
                **{
                    name: value
                    for name, value in os.environ.items()
                    if name not in PROMPT_OPT_OUTS
                },
                "PATH": f"{executable.parent}{os.pathsep}{os.environ['PATH']}",
            }
            record(vhs, run, run.tape(tapes), env=env, backdrops=Path(backdrops))


def record(
    vhs: VhsManager,
    run: ExampleRun,
    tape: Path,
    *,
    env: Mapping[str, str],
    backdrops: Path,
) -> None:
    """Record `run` from `tape` in each theme, drawing its backdrops in `backdrops`.

    Raises:
        ClickException: The tape doesn't set its size, or vhs didn't take a still.
    """
    commands = tape.read_text()
    size = {m["setting"]: int(m["pixels"]) for m in TAPE_SIZE.finditer(commands)}
    if size.keys() != {"Width", "Height"}:
        msg = f"{tape.relative_to(ROOT)} must set its Width and Height"
        raise click.ClickException(msg)

    # A variant gets only its text, recorded in the dark theme.
    for theme, image in (run.images or {"dark": None}).items():
        backdrop = backdrops / f"{run.stem}-{theme}.png"
        THEMES[theme].backdrop.save(
            backdrop, size=(size["Width"], size["Height"]), window=WINDOW
        )
        settings = {
            "Theme": THEMES[theme].vhs,
            "MarginFill": str(backdrop),
            **SETTINGS,
        }
        source = f"{TYPING_SPEED}\n{commands}"
        outputs: tuple[Path, ...] = ()
        match image:
            case Path(suffix=".png"):
                image.unlink(missing_ok=True)
                # vhs saves a screenshot on its next frame, so it waits for it.
                source += f"\nScreenshot {json.dumps(str(image))}\nSleep 1s"
            case Path():
                outputs = (image,)
            case None:
                pass
        recording = vhs.record(Tape(source, outputs, settings), env=env)
        if image is not None:
            if not image.exists():
                msg = f"vhs didn't write {image.relative_to(ROOT)}"
                raise click.ClickException(msg)
            click.echo(f"wrote {image.relative_to(ROOT)}")
        # The screens ignore colors, so one theme's are enough.
        if theme == "dark":
            capture = run.write_capture(recording)
            click.echo(f"wrote {capture.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
