#!/usr/bin/env -S uv run
"""Read and raise the clap versions the workspace supports, for CI's clap bump.

The root `Cargo.toml` pins clap with `clap = ">=MIN, <=MAX"`, since couture copies
clap internals that any release may change. Each command prints `key=value` lines,
ready for `$GITHUB_OUTPUT`.

    scripts/clap-bounds.py bounds          # min=4.6.7, max=4.6.7
    scripts/clap-bounds.py latest          # latest=4.6.8, newer=true: above max?
    scripts/clap-bounds.py set-max 4.6.8   # raises MAX in Cargo.toml
"""

from pathlib import Path

import click
import httpx
from semver import Version

from devtools.rust.crates_io import NoReleaseError, SparseIndex
from devtools.rust.manifest import (
    InclusiveRange,
    UnexpectedRequirementError,
    WorkspaceManifest,
)

ROOT = Path(__file__).resolve().parent.parent
CRATE = "clap"

manifest_option = click.option(
    "--manifest",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default=ROOT / "Cargo.toml",
    show_default=True,
    help="The workspace's root Cargo.toml.",
)


def supported(manifest: Path) -> InclusiveRange:
    """The clap range `manifest` pins.

    Raises:
        ClickException: It isn't pinned with `clap = ">=MIN, <=MAX"`.
    """
    try:
        return WorkspaceManifest(manifest.read_text()).dependency_range(CRATE)
    except UnexpectedRequirementError as error:
        msg = f"{manifest}: {error}"
        raise click.ClickException(msg) from error


def echo_range(bounds: InclusiveRange) -> None:
    """Print `bounds` as `min=` and `max=` lines."""
    click.echo(f"min={bounds.min}\nmax={bounds.max}")


@click.group()
def main() -> None:
    """Read and raise the clap versions Cargo.toml allows."""


@main.command()
@manifest_option
def bounds(manifest: Path) -> None:
    """Print the oldest and newest clap allowed."""
    echo_range(supported(manifest))


@main.command()
@manifest_option
def latest(manifest: Path) -> None:
    """Print clap's newest release of the allowed major, and whether it's above max."""
    allowed = supported(manifest)
    try:
        with httpx.Client() as client:
            crate = SparseIndex(client).crate(CRATE)
        newest = crate.newest_release(major=allowed.max.major)
    except (httpx.HTTPError, NoReleaseError, ValueError) as error:
        msg = f"couldn't find clap's latest release: {error}"
        raise click.ClickException(msg) from error
    click.echo(f"latest={newest}\nnewer={str(newest > allowed.max).lower()}")


@main.command("set-max")
@manifest_option
@click.argument("version")
def set_max(manifest: Path, version: str) -> None:
    """Allow clap up to VERSION, in place of the current max."""
    try:
        raised = supported(manifest).with_max(Version.parse(version))
        current = WorkspaceManifest(manifest.read_text())
        _ = manifest.write_text(current.with_dependency_range(CRATE, raised).text)
    except ValueError as error:
        raise click.ClickException(str(error)) from error
    echo_range(raised)


if __name__ == "__main__":
    main()
