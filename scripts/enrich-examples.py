#!/usr/bin/env -S uv run
"""Record every example, then fill its "Example Output" doc blocks from the recordings.

    //! `cargo run ... -- --help` outputs:
    //!
    //! Any prose in between is kept.
    //!
    //! ```terminal
    //! ```                      <- replaced with what the recording ends on

A fence's caption is the nearest paragraph above it shaped like the `cargo run` line.
The fence gets what that run printed, recorded by `record-examples.py` with the
features the caption picks, e.g. another prompt backend. The caption is rewritten to
the real command, with the args the tape typed; `cargo run ...` stands for the example
with its default features. The output is plain text, without color, so it reads well
in an editor. Afterwards `cargo +nightly fmt --all` runs, so the docs are wrapped as CI
checks.

    scripts/enrich-examples.py [EXAMPLE]...
"""

import re
import shlex
import subprocess
import sys
from dataclasses import replace
from pathlib import Path

import click

from devtools.rust.cargo import Cargo, Example
from devtools.rust.rsdoc import Fence, ModuleDocs, Paragraph, replaced
from devtools.vhs import Recording
from example_recordings import PACKAGE, ROOT, ExampleRun

RECORD = Path(__file__).resolve().parent / "record-examples.py"

# `<command>` outputs:
CAPTION = re.compile(r"^`(?P<command>cargo run [^`]*)` outputs:$")
# `cargo run ... -- --help`: the example as is, with the args the tape types.
PLACEHOLDER = "cargo run ..."
OUTPUT_INFO = "terminal"


class MismatchedExampleError(ValueError):
    """A caption runs another example than the one it documents."""


def captioned(docs: ModuleDocs, fence: Fence) -> tuple[Paragraph, str] | None:
    """The `cargo run` paragraph above `fence`, and its command, if there is one."""
    for block in docs.before(fence):
        if isinstance(block, Paragraph) and (match := CAPTION.match(block.markdown)):
            return block, match["command"]
    return None


def run_of(example: Example, command: str) -> ExampleRun:
    """The run a caption's command asks for.

    Raises:
        MismatchedExampleError: The command runs another example.
    """
    if command.startswith(PLACEHOLDER):
        return ExampleRun(
            example=example.name,
            package=PACKAGE,
            beyond_defaults=example.beyond_defaults,
        )

    run = replace(ExampleRun.parse(command), beyond_defaults=example.beyond_defaults)
    if run.example != example.name:
        msg = f"{example.source.path}: `{command}` runs another example"
        raise MismatchedExampleError(msg)
    return run


def runs(example: Example, docs: ModuleDocs) -> list[ExampleRun]:
    """The runs `docs`' output fences show, in order."""
    return [
        run_of(example, found[1])
        for fence in docs.fences(OUTPUT_INFO)
        if (found := captioned(docs, fence)) is not None
    ]


def enrich(example: Example, docs: ModuleDocs) -> ModuleDocs:
    """`docs`, with each output fence and its caption filled from its recording.

    Raises:
        ScrolledAwayError: A tape's `Set Height` is too small to show the whole run.
    """
    # Each edit reparses the docs, so the fences are looked up again by position.
    for index in range(len(docs.fences(OUTPUT_INFO))):
        fence = docs.fences(OUTPUT_INFO)[index]
        found = captioned(docs, fence)
        if found is None:
            continue

        paragraph, command = found
        run = run_of(example, command)
        session = Recording.parse(run.capture.read_text()).session()
        # The first word is the example's name, or an alias for it.
        filled = replace(run, args=tuple(shlex.split(session.command)[1:]))

        docs = replaced(docs, fence, fence.with_content(session.output))
        docs = replaced(docs, paragraph, f"`{filled}` outputs:")
    return docs


@click.command()
@click.argument("names", nargs=-1, metavar="[EXAMPLE]...")
def main(names: tuple[str, ...]) -> None:
    """Record the examples named on the command line, or all, and fill their docs."""
    with Cargo() as cargo:
        workspace = cargo.workspace(ROOT)
        package = workspace.package(PACKAGE)

    unknown = [name for name in names if name not in package.examples]
    if unknown:
        msg = f"no such example: {', '.join(unknown)}"
        raise click.BadParameter(msg, param_hint="EXAMPLE")

    examples = [package.examples[name] for name in names or sorted(package.examples)]
    documented = {
        example: docs
        for example in examples
        if (docs := example.source.source().module_docs().read()) is not None
    }

    # Every example as it is, and every variant a caption asks for.
    variants = [
        str(run)
        for example, docs in documented.items()
        for run in runs(example, docs)
        if run.variant is not None
    ]
    record = [*(example.name for example in examples), *dict.fromkeys(variants)]
    _ = subprocess.run([sys.executable, RECORD, *record], check=True)

    originals = {
        example: example.source.source()
        for example, docs in documented.items()
        if runs(example, docs)
    }
    for example, source in originals.items():
        example.source.write(
            source.module_docs().write(enrich(example, documented[example]))
        )

    # A filled caption is one line, however long; rustfmt wraps doc comments the way
    # CI checks they're wrapped. `rustfmt.toml` is unstable, so it takes a nightly.
    _ = workspace.run("+nightly", "fmt", "--all").check()

    for example, source in originals.items():
        if example.source.source() != source:
            click.echo(f"enriched {example.source.path.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
