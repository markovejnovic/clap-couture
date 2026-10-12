"""A workspace's root `Cargo.toml`, and the version ranges it pins dependencies to.

```python
manifest = WorkspaceManifest(Path("Cargo.toml").read_text())
clap = manifest.dependency_range("clap").with_max(Version.parse("4.6.8"))
Path("Cargo.toml").write_text(manifest.with_dependency_range("clap", clap).text)
```
"""

import re
from dataclasses import dataclass
from typing import Self, override

import tomlkit
from semver import Version
from tomlkit.items import String, Table

# Exactly the form `InclusiveRange` writes, so a reworded requirement fails loudly.
_INCLUSIVE_RANGE = re.compile(r"^>=(?P<min>[^,\s]+), <=(?P<max>\S+)$")


class UnexpectedRequirementError(ValueError):
    """A dependency isn't pinned to a range of the form `InclusiveRange` reads."""


@dataclass(frozen=True)
class InclusiveRange:
    """The versions `>=min, <=max` allows; both are releases, not pre-releases."""

    min: Version
    max: Version

    def __post_init__(self) -> None:
        """Check the range is one cargo can satisfy with a release.

        Raises:
            UnexpectedRequirementError: An end is a pre-release, or `min > max`.
        """
        for end in (self.min, self.max):
            if end.prerelease is not None:
                msg = f"the range {self} ends at the pre-release {end}"
                raise UnexpectedRequirementError(msg)
        if self.min > self.max:
            msg = f"the range {self} is empty: {self.min} is above {self.max}"
            raise UnexpectedRequirementError(msg)

    @classmethod
    def parse(cls, requirement: str) -> Self:
        """Read a requirement of the form `>=MIN, <=MAX`.

        Raises:
            UnexpectedRequirementError: It isn't of that form, or its ends aren't
                semver releases.
        """
        matched = _INCLUSIVE_RANGE.match(requirement)
        if matched is None:
            msg = f'expected ">=MIN, <=MAX", found {requirement!r}'
            raise UnexpectedRequirementError(msg)
        try:
            oldest, newest = (
                Version.parse(matched["min"]),
                Version.parse(matched["max"]),
            )
        except ValueError as error:
            msg = f"{requirement!r} names a version that isn't semver: {error}"
            raise UnexpectedRequirementError(msg) from error
        return cls(oldest, newest)

    def with_max(self, new_max: Version) -> Self:
        """This range, up to `new_max`.

        Raises:
            UnexpectedRequirementError: `new_max` is a pre-release, or below `min`.
        """
        return type(self)(self.min, new_max)

    @override
    def __str__(self) -> str:
        """The requirement, as Cargo.toml spells it."""
        return f">={self.min}, <={self.max}"


@dataclass(frozen=True)
class WorkspaceManifest:
    """A workspace's root `Cargo.toml`, by its text."""

    text: str

    def dependency_range(self, name: str) -> InclusiveRange:
        """The range `[workspace.dependencies]` pins `name` to.

        Raises:
            UnexpectedRequirementError: `name` isn't pinned with a `">=MIN, <=MAX"`
                string.
        """
        requirement = self._requirement(tomlkit.parse(self.text), name)
        return InclusiveRange.parse(requirement.value)

    def with_dependency_range(self, name: str, bounds: InclusiveRange) -> Self:
        """This manifest with `name` pinned to `bounds`; nothing else changes.

        Raises:
            UnexpectedRequirementError: `name` isn't pinned with a `">=MIN, <=MAX"`
                string.
        """
        document = tomlkit.parse(self.text)
        _ = InclusiveRange.parse(self._requirement(document, name).value)
        # tomlkit keeps the line's position and comment.
        self._dependencies(document)[name] = str(bounds)
        return type(self)(tomlkit.dumps(document))

    @staticmethod
    def _dependencies(document: tomlkit.TOMLDocument) -> Table:
        """The document's `[workspace.dependencies]` table.

        Raises:
            UnexpectedRequirementError: It has none.
        """
        workspace = _child(document, "workspace")
        dependencies = (
            _child(workspace, "dependencies") if isinstance(workspace, Table) else None
        )
        if not isinstance(dependencies, Table):
            msg = "Cargo.toml has no [workspace.dependencies] table"
            raise UnexpectedRequirementError(msg)
        return dependencies

    @classmethod
    def _requirement(cls, document: tomlkit.TOMLDocument, name: str) -> String:
        """`name`'s version requirement string in `[workspace.dependencies]`.

        Raises:
            UnexpectedRequirementError: `name` isn't there, or isn't a plain string.
        """
        requirement = _child(cls._dependencies(document), name)
        if not isinstance(requirement, String):
            msg = f'expected `{name} = ">=MIN, <=MAX"` in [workspace.dependencies]'
            raise UnexpectedRequirementError(msg)
        return requirement


def _child(table: tomlkit.TOMLDocument | Table, key: str) -> object:
    """`table[key]`, or None when there is none."""
    # tomlkit types `[]` but not `get`, which strict pyright rejects.
    return table[key] if key in table else None  # noqa: SIM401
