"""crates.io's sparse index: every version published of a crate.

```python
with httpx.Client() as client:
    newest = SparseIndex(client).crate("clap").newest_release(major=4)
```
"""

import json
from dataclasses import dataclass
from typing import Self, cast

import httpx
from semver import Version

INDEX_URL = "https://index.crates.io"
# crates.io asks automated clients to say who they are.
USER_AGENT = "clap-couture-devtools (+https://github.com/markovejnovic/clap-couture)"


class NoReleaseError(LookupError):
    """The index lists no release the caller would take."""


@dataclass(frozen=True)
class Release:
    """One published version of a crate, a line of its index file."""

    version: Version
    yanked: bool

    @classmethod
    def parse(cls, line: str) -> Self:
        """Read one line of an index file.

        Raises:
            ValueError: The line isn't an index entry with a semver `vers`.
        """
        entry = cast("dict[str, object]", json.loads(line))
        return cls(Version.parse(str(entry["vers"])), yanked=bool(entry["yanked"]))

    @property
    def is_stable(self) -> bool:
        """Whether this is neither yanked nor a pre-release."""
        return not self.yanked and self.version.prerelease is None


@dataclass(frozen=True)
class IndexedCrate:
    """A crate, with every release the index lists."""

    name: str
    releases: tuple[Release, ...]

    @classmethod
    def parse(cls, name: str, index_file: str) -> Self:
        """Read `name`'s index file: one JSON entry per line.

        Raises:
            ValueError: A line isn't an index entry.
        """
        lines = (line for line in index_file.splitlines() if line.strip())
        return cls(name, tuple(Release.parse(line) for line in lines))

    def newest_release(self, *, major: int) -> Version:
        """The newest `major.x` release that is neither yanked nor a pre-release.

        Raises:
            NoReleaseError: There is none.
        """
        versions = [
            release.version
            for release in self.releases
            if release.is_stable and release.version.major == major
        ]
        if not versions:
            msg = f"the index lists no {self.name} {major}.x release"
            raise NoReleaseError(msg)
        return max(versions)


@dataclass(frozen=True)
class SparseIndex:
    """A sparse registry index, crates.io's unless told otherwise."""

    client: httpx.Client
    url: str = INDEX_URL

    @staticmethod
    def path(name: str) -> str:
        """Where the index keeps `name`'s file, e.g. `cl/ap/clap`."""
        name = name.lower()
        match len(name):
            case 1 | 2:
                return f"{len(name)}/{name}"
            case 3:
                return f"3/{name[0]}/{name}"
            case _:
                return f"{name[:2]}/{name[2:4]}/{name}"

    def crate(self, name: str) -> IndexedCrate:
        """`name`, with every release the index lists.

        Raises:
            httpx.HTTPError: The index couldn't be read.
            ValueError: The index file is malformed.
        """
        response = self.client.get(
            f"{self.url}/{self.path(name)}",
            headers={"User-Agent": USER_AGENT},
            timeout=30,
        )
        return IndexedCrate.parse(name, response.raise_for_status().text)
