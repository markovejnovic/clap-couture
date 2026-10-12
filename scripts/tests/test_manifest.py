"""`devtools.rust.manifest`: reading and raising a dependency's pinned range."""

import pytest
from semver import Version

from devtools.rust.manifest import (
    InclusiveRange,
    UnexpectedRequirementError,
    WorkspaceManifest,
)

MANIFEST = """\
[workspace]
members = ["crates/*"]

[workspace.dependencies]
heck = "0.5"
# couture copies clap internals.
clap = ">=4.5.0, <=4.6.7"  # tested
syn = { version = "3", features = ["full"] }
"""


def test_reads_the_range() -> None:
    bounds = WorkspaceManifest(MANIFEST).dependency_range("clap")
    assert bounds == InclusiveRange(Version(4, 5, 0), Version(4, 6, 7))


def test_raising_the_max_changes_only_that_requirement() -> None:
    manifest = WorkspaceManifest(MANIFEST)
    raised = manifest.dependency_range("clap").with_max(Version(4, 10, 0))
    assert manifest.with_dependency_range("clap", raised).text == MANIFEST.replace(
        "<=4.6.7", "<=4.10.0"
    )


def test_the_max_is_compared_as_semver() -> None:
    bounds = InclusiveRange.parse(">=4.9.0, <=4.10.0")
    assert bounds.min < bounds.max


@pytest.mark.parametrize(
    "requirement",
    [
        '"4.6"',
        '">=4.5.0,<=4.6.7"',
        '">=4.5, <=4.6.7"',
        '">=4.5.0, <4.6.7"',
        '{ version = ">=4.5.0, <=4.6.7" }',
    ],
)
def test_rejects_another_requirement(requirement: str) -> None:
    manifest = WorkspaceManifest(f"[workspace.dependencies]\nclap = {requirement}\n")
    with pytest.raises(UnexpectedRequirementError):
        _ = manifest.dependency_range("clap")


def test_rejects_a_missing_dependency() -> None:
    with pytest.raises(UnexpectedRequirementError, match="clap"):
        _ = WorkspaceManifest(
            '[workspace.dependencies]\nheck = "0.5"\n'
        ).dependency_range("clap")


def test_rejects_a_manifest_without_workspace_dependencies() -> None:
    with pytest.raises(UnexpectedRequirementError, match=r"workspace\.dependencies"):
        _ = WorkspaceManifest('[dependencies]\nclap = "4"\n').dependency_range("clap")


def test_rejects_an_empty_range() -> None:
    with pytest.raises(UnexpectedRequirementError, match="empty"):
        _ = InclusiveRange.parse(">=4.7.0, <=4.6.7")


def test_wont_lower_the_max_below_the_min() -> None:
    bounds = InclusiveRange.parse(">=4.5.0, <=4.6.7")
    with pytest.raises(UnexpectedRequirementError, match="empty"):
        _ = bounds.with_max(Version(4, 4, 0))


def test_wont_end_at_a_prerelease() -> None:
    bounds = InclusiveRange.parse(">=4.5.0, <=4.6.7")
    with pytest.raises(UnexpectedRequirementError, match="pre-release"):
        _ = bounds.with_max(Version.parse("4.7.0-rc.1"))
