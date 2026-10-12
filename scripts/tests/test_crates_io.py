"""`devtools.rust.crates_io`: finding a crate's newest release in the sparse index."""

import json

import httpx
import pytest
from semver import Version

from devtools.rust.crates_io import (
    USER_AGENT,
    IndexedCrate,
    NoReleaseError,
    SparseIndex,
)


def index_file(*releases: tuple[str, bool]) -> str:
    """An index file listing `(version, yanked)` releases, in order."""
    return "\n".join(
        json.dumps({"name": "clap", "vers": vers, "yanked": yanked, "deps": []})
        for vers, yanked in releases
    )


@pytest.mark.parametrize(
    ("name", "path"),
    [
        ("a", "1/a"),
        ("ab", "2/ab"),
        ("abc", "3/a/abc"),
        ("clap", "cl/ap/clap"),
        ("Clap_Builder", "cl/ap/clap_builder"),
    ],
)
def test_path(name: str, path: str) -> None:
    assert SparseIndex.path(name) == path


def test_newest_release_skips_yanked_prereleases_and_other_majors() -> None:
    crate = IndexedCrate.parse(
        "clap",
        index_file(
            ("4.9.0", False),
            ("4.10.0", False),
            ("4.11.0", True),
            ("4.12.0-rc.1", False),
            ("5.0.0", False),
        ),
    )
    assert crate.newest_release(major=4) == Version(4, 10, 0)


def test_newest_release_needs_one() -> None:
    crate = IndexedCrate.parse("clap", index_file(("4.0.0", True)))
    with pytest.raises(NoReleaseError, match=r"clap 4\.x"):
        _ = crate.newest_release(major=4)


def test_fetches_the_crate_file_and_says_who_asks() -> None:
    requests: list[httpx.Request] = []

    def respond(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, text=index_file(("4.6.7", False)) + "\n")

    with httpx.Client(transport=httpx.MockTransport(respond)) as client:
        crate = SparseIndex(client).crate("clap")

    assert crate.newest_release(major=4) == Version(4, 6, 7)
    [request] = requests
    assert str(request.url) == "https://index.crates.io/cl/ap/clap"
    assert request.headers["User-Agent"] == USER_AGENT


def test_a_failed_fetch_raises() -> None:
    def respond(_: httpx.Request) -> httpx.Response:
        return httpx.Response(404)

    with (
        httpx.Client(transport=httpx.MockTransport(respond)) as client,
        pytest.raises(httpx.HTTPStatusError),
    ):
        _ = SparseIndex(client).crate("clap")
