#!/usr/bin/env python3
"""Read and move the clap versions the workspace supports, for CI's clap bump.

The root `Cargo.toml` pins clap with one line, `clap = ">=MIN, <=MAX"`, since couture
copies clap internals that any release may change. Each command prints `key=value`
lines, ready for `$GITHUB_OUTPUT`.

    scripts/clap_bounds.py bounds         # min=4.5.0, max=4.6.7
    scripts/clap_bounds.py latest         # latest=4.6.8, newer=true: newer than max?
    scripts/clap_bounds.py set-max 4.6.8  # rewrites MAX in Cargo.toml
    scripts/clap_bounds.py self-test      # runs this file's doctests

Only the standard library is used, so CI runs this without installing anything.
"""

import argparse
import doctest
import importlib
import json
import re
import sys
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Self, cast, override

MANIFEST = Path(__file__).resolve().parent.parent / "Cargo.toml"
INDEX_URL = "https://index.crates.io/cl/ap/clap"
USER_AGENT = "clap-couture-clap-bump (+https://github.com/markovejnovic/clap-couture)"

# The whole line, so a reworded requirement fails loudly instead of being half-read.
REQUIREMENT = re.compile(
    r'^clap = ">=(?P<min>\d+\.\d+\.\d+), <=(?P<max>\d+\.\d+\.\d+)"$', re.MULTILINE
)
ANY_CLAP_LINE = re.compile(r"^clap\s*=.*$", re.MULTILINE)
SEMVER = re.compile(
    r"""
    ^(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)
    (?:-(?P<pre>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?
    (?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$
    """,
    re.VERBOSE,
)

Precedence = tuple[int, int, int, int, tuple[tuple[int, int, str], ...]]


class BoundsError(ValueError):
    """The manifest's clap requirement, or a version, isn't in the expected form."""


@dataclass(frozen=True)
class Version:
    """A semver version; build metadata is dropped, as it doesn't affect precedence.

    >>> Version.parse("4.10.0") > Version.parse("4.9.9")
    True
    >>> Version.parse("4.6.8-rc.1") < Version.parse("4.6.8")
    True
    >>> Version.parse("1.0.0-alpha.2") < Version.parse("1.0.0-alpha.10")
    True
    >>> Version.parse("1.0.0-alpha.1") < Version.parse("1.0.0-alpha.beta")
    True
    >>> Version.parse("1.0.0-alpha") < Version.parse("1.0.0-alpha.1")
    True
    >>> Version.parse("4.6.7+build.5") == Version.parse("4.6.7")
    True
    >>> Version.parse("4.6")
    Traceback (most recent call last):
    ...
    clap_bounds.BoundsError: not a semver version: '4.6'
    """

    major: int
    minor: int
    patch: int
    prerelease: tuple[str, ...] = ()

    @classmethod
    def parse(cls, text: str) -> Self:
        """The version `text` spells.

        Raises:
            BoundsError: `text` isn't a semver version.
        """
        matched = SEMVER.match(text)
        if matched is None:
            msg = f"not a semver version: {text!r}"
            raise BoundsError(msg)
        pre = matched["pre"]
        return cls(
            int(matched["major"]),
            int(matched["minor"]),
            int(matched["patch"]),
            tuple(pre.split(".")) if pre else (),
        )

    @property
    def is_prerelease(self) -> bool:
        """Whether this is a pre-release, e.g. `4.0.0-rc.1`."""
        return bool(self.prerelease)

    def precedence(self) -> Precedence:
        """A key that sorts versions by semver precedence."""
        # A release sorts after its pre-releases; numeric identifiers sort before
        # alphanumeric ones, and a shorter identifier list before a longer one.
        identifiers = tuple(
            (0, int(part), "") if part.isdigit() else (1, 0, part)
            for part in self.prerelease
        )
        release = int(not self.prerelease)
        return (self.major, self.minor, self.patch, release, identifiers)

    def __lt__(self, other: Self) -> bool:
        """Whether this precedes `other`."""
        return self.precedence() < other.precedence()

    def __le__(self, other: Self) -> bool:
        """Whether this precedes or equals `other`."""
        return self.precedence() <= other.precedence()

    def __gt__(self, other: Self) -> bool:
        """Whether this follows `other`."""
        return self.precedence() > other.precedence()

    def __ge__(self, other: Self) -> bool:
        """Whether this follows or equals `other`."""
        return self.precedence() >= other.precedence()

    @override
    def __str__(self) -> str:
        core = f"{self.major}.{self.minor}.{self.patch}"
        return f"{core}-{'.'.join(self.prerelease)}" if self.prerelease else core


@dataclass(frozen=True)
class ClapBounds:
    r"""The oldest and newest clap the workspace supports, both inclusive.

    >>> bounds = ClapBounds.parse('[dependencies]\nclap = ">=4.5.0, <=4.6.7"\n')
    >>> print(bounds.min, bounds.max)
    4.5.0 4.6.7
    >>> ClapBounds.parse('clap = "4.6"\n')
    Traceback (most recent call last):
    ...
    clap_bounds.BoundsError: expected `clap = ">=MIN, <=MAX"`, found: clap = "4.6"
    >>> ClapBounds.parse('clap = ">=4.7.0, <=4.6.7"\n')
    Traceback (most recent call last):
    ...
    clap_bounds.BoundsError: clap's minimum 4.7.0 is above its maximum 4.6.7
    """

    min: Version
    max: Version

    @classmethod
    def parse(cls, manifest: str) -> Self:
        """The bounds in `manifest`, the root `Cargo.toml`'s text.

        Raises:
            BoundsError: there isn't exactly one well-formed clap line, or its minimum
                is above its maximum.
        """
        requirements = list(REQUIREMENT.finditer(manifest))
        if len(requirements) != 1:
            found = ", ".join(ANY_CLAP_LINE.findall(manifest)) or "no clap line"
            msg = f'expected `clap = ">=MIN, <=MAX"`, found: {found}'
            raise BoundsError(msg)
        requirement = requirements[0]
        oldest = Version.parse(requirement["min"])
        newest = Version.parse(requirement["max"])
        if oldest > newest:
            msg = f"clap's minimum {oldest} is above its maximum {newest}"
            raise BoundsError(msg)
        return cls(oldest, newest)

    def with_max(self, new_max: Version) -> Self:
        """These bounds, up to `new_max`.

        Raises:
            BoundsError: `new_max` is a pre-release, or below the minimum.
        """
        if new_max.is_prerelease:
            msg = f"won't support the pre-release clap {new_max}"
            raise BoundsError(msg)
        if new_max < self.min:
            msg = f"clap {new_max} is below the minimum {self.min}"
            raise BoundsError(msg)
        return type(self)(self.min, new_max)

    def rewrite(self, manifest: str) -> str:
        r"""`manifest` with its clap line set to these bounds; nothing else changes.

        >>> manifest = 'a = "1"\nclap = ">=4.5.0, <=4.6.7"\nb = "2"\n'
        >>> bounds = ClapBounds.parse(manifest).with_max(Version.parse("4.10.0"))
        >>> print(bounds.rewrite(manifest), end="")
        a = "1"
        clap = ">=4.5.0, <=4.10.0"
        b = "2"
        """
        # `parse` checks there is exactly one line to replace.
        _ = ClapBounds.parse(manifest)
        return REQUIREMENT.sub(f'clap = ">={self.min}, <={self.max}"', manifest)


def newest_release(index: str, major: int) -> Version:
    r"""The newest non-yanked, non-pre-release `major.x` in a sparse-index file.

    >>> index = "\n".join([
    ...     '{"vers": "4.9.0", "yanked": false}',
    ...     '{"vers": "4.10.0", "yanked": false}',
    ...     '{"vers": "4.11.0", "yanked": true}',
    ...     '{"vers": "4.12.0-rc.1", "yanked": false}',
    ...     '{"vers": "5.0.0", "yanked": false}',
    ... ])
    >>> print(newest_release(index, 4))
    4.10.0

    Raises:
        BoundsError: no release of `major` is listed.
    """
    releases: list[Version] = []
    for line in index.splitlines():
        if not line.strip():
            continue
        entry = cast("dict[str, object]", json.loads(line))
        version = Version.parse(str(entry["vers"]))
        if entry["yanked"] or version.is_prerelease or version.major != major:
            continue
        releases.append(version)
    if not releases:
        msg = f"the index lists no clap {major}.x release"
        raise BoundsError(msg)
    return max(releases, key=Version.precedence)


def fetch_index() -> str:
    """The crates.io sparse-index file for clap: a JSON line per published version."""
    request = urllib.request.Request(INDEX_URL, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=30) as response:  # noqa: S310 - fixed https URL
        return cast("bytes", response.read()).decode()


def main() -> None:
    """Run the command named on the command line."""
    parser = argparse.ArgumentParser(description="Read and move the clap bounds.")
    _ = parser.add_argument("--manifest", type=Path, default=MANIFEST)
    commands = parser.add_subparsers(dest="command", required=True)
    _ = commands.add_parser("bounds", help="print min= and max=")
    _ = commands.add_parser("latest", help="print latest= and newer= (than max)")
    set_max = commands.add_parser("set-max", help="rewrite the maximum in Cargo.toml")
    _ = set_max.add_argument("version", type=Version.parse)
    _ = commands.add_parser("self-test", help="run the doctests")
    arguments = parser.parse_args()
    manifest_path = cast("Path", arguments.manifest)
    command = cast("str", arguments.command)

    if command == "self-test":
        # Imported by name, so tracebacks in the doctests name `clap_bounds`.
        failed, _ = doctest.testmod(importlib.import_module("clap_bounds"))
        sys.exit(1 if failed else 0)

    manifest = manifest_path.read_text()
    bounds = ClapBounds.parse(manifest)
    if command == "bounds":
        _ = sys.stdout.write(f"min={bounds.min}\nmax={bounds.max}\n")
    elif command == "latest":
        latest = newest_release(fetch_index(), bounds.max.major)
        newer = str(latest > bounds.max).lower()
        _ = sys.stdout.write(f"latest={latest}\nnewer={newer}\n")
    else:
        new_bounds = bounds.with_max(cast("Version", arguments.version))
        _ = manifest_path.write_text(new_bounds.rewrite(manifest))
        _ = sys.stdout.write(f"min={new_bounds.min}\nmax={new_bounds.max}\n")


if __name__ == "__main__":
    try:
        main()
    except BoundsError as error:
        sys.exit(f"error: {error}")
