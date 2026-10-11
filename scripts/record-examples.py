#!/usr/bin/env -S uv run
"""Record examples with vhs, from their package's `examples/tapes/<example>.tape`.

    scripts/record-examples.py [EXAMPLE | 'cargo run ... --example EXAMPLE']...

An example name records it with its default features: its GIFs and its text capture.
A `cargo run` command records the example built the way it says, e.g. with another
prompt backend; `example_recordings` says where each recording goes. Without
arguments, every example is recorded with its default features.

A tape holds only its `Set Height` and what to type; this script adds the outputs, the
theme and the settings every recording shares. The examples are on PATH, by name.

The `.txt` captures are the same on every run, unlike the GIFs, so CI diffs those to
tell whether the GIFs are stale.
"""

import os

import click

from devtools.rust.cargo import Cargo
from devtools.vhs import Tape, Vhs
from example_recordings import PACKAGE, ROOT, THEMES, ExampleRun

SETTINGS: dict[str, str | int] = {
    "Shell": "bash",
    "FontFamily": "JetBrains Mono",
    "FontSize": 18,
    "Width": 1000,
    "Padding": 30,
    "WindowBar": "Colorful",
}
# vhs rejects a quoted duration, and `Tape` quotes every string setting, so this one
# is a command.
TYPING_SPEED = "Set TypingSpeed 75ms"


@click.command()
@click.argument("arguments", nargs=-1, metavar="[EXAMPLE | 'cargo run ...']...")
def main(arguments: tuple[str, ...]) -> None:
    """Record the runs named on the command line, or every example."""
    with Cargo() as cargo:
        package = cargo.workspace(ROOT).package(PACKAGE)
        tapes = package.directory / "examples" / "tapes"
        runs = [ExampleRun.from_argument(argument) for argument in arguments] or [
            ExampleRun(example=name, package=PACKAGE)
            for name in sorted(package.examples)
        ]

        missing = [
            tape for tape in (run.tape(tapes) for run in runs) if not tape.exists()
        ]
        if missing:
            listed = (f"missing tape: {tape.relative_to(ROOT)}" for tape in missing)
            raise click.ClickException("\n".join(listed))

        executables = {
            run: package.examples[run.example].build(
                features=run.features, target_dir=run.target_dir
            )
            for run in runs
        }

    with Vhs() as vhs:
        for run, executable in executables.items():
            env = {
                **os.environ,
                "PATH": f"{executable.parent}{os.pathsep}{os.environ['PATH']}",
            }
            commands = f"{TYPING_SPEED}\n{run.tape(tapes).read_text()}"
            # A variant gets only its text, recorded in the dark theme.
            for theme, gif in (run.gifs or {"dark": None}).items():
                settings = {"Theme": THEMES[theme], **SETTINGS}
                outputs = () if gif is None else (gif,)
                recording = vhs.record(Tape(commands, outputs, settings), env=env)
                if gif is not None:
                    click.echo(f"wrote {gif.relative_to(ROOT)}")
                # The screens ignore colors, so one theme's are enough.
                if theme == "dark":
                    capture = run.write_capture(recording)
                    click.echo(f"wrote {capture.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
