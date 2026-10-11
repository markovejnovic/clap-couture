"""Interact with cargo.

```python
with Cargo() as cargo:
    package = cargo.workspace(Path(".")).package("clap-couture")
    example = package.examples["01-orbit-organized"]
    executable = example.build(features=OnlyFeatures((Feature("markdown"),)))
    source = example.source.source()
```
"""

import json
import shlex
import shutil
import subprocess
from collections.abc import Mapping
from dataclasses import dataclass, field
from pathlib import Path
from types import TracebackType
from typing import Self, cast, override

from devtools.rust.source import RustFile


class CargoUnavailableError(RuntimeError):
    """`cargo` is not on PATH."""


class CargoFailedError(RuntimeError):
    """cargo exited unsuccessfully."""


class UnsupportedCargoRunError(ValueError):
    """A `cargo run` command with arguments `CargoRun` doesn't model."""


@dataclass(frozen=True)
class CargoOutput:
    """What a cargo run printed, and how it exited."""

    stdout: str
    stderr: str
    exit_code: int

    def check(self) -> Self:
        """This output, if cargo succeeded.

        Raises:
            CargoFailedError: cargo exited unsuccessfully; the message is its stderr.
        """
        if self.exit_code != 0:
            msg = f"cargo exited with {self.exit_code}:\n{self.stderr}"
            raise CargoFailedError(msg)
        return self


@dataclass(frozen=True)
class Feature:
    """A cargo feature, by its label."""

    label: str


@dataclass(frozen=True)
class DefaultFeatures:
    """The package's default features, plus `extra`."""

    extra: tuple[Feature, ...] = ()


@dataclass(frozen=True)
class OnlyFeatures:
    """Exactly `features`, without the package's defaults."""

    features: tuple[Feature, ...] = ()


@dataclass(frozen=True)
class AllFeatures:
    """Every feature of the package."""


type Features = DefaultFeatures | OnlyFeatures | AllFeatures
"""The features a build enables."""

_DEFAULT_FEATURES = DefaultFeatures()


def _feature_args(features: Features) -> list[str]:
    """The cargo arguments that enable `features`."""

    def listed(features: tuple[Feature, ...]) -> list[str]:
        labels = ",".join(feature.label for feature in features)
        return ["--features", labels] if features else []

    match features:
        case DefaultFeatures(extra=extra):
            return listed(extra)
        case OnlyFeatures(features=only):
            return ["--no-default-features", *listed(only)]
        case AllFeatures():
            return ["--all-features"]


@dataclass(frozen=True)
class CargoRun:
    """`cargo run [-p <package>] [feature flags] --example <example> [-- <args>]`."""

    example: str
    package: str | None = None
    features: Features = _DEFAULT_FEATURES
    args: tuple[str, ...] = ()

    @classmethod
    def parse(cls, command: str) -> Self:
        """Read a `cargo run` command line.

        Raises:
            UnsupportedCargoRunError: It isn't `cargo run`, runs no example, or passes
                cargo an argument this type doesn't model.
        """
        words = shlex.split(command)
        if words[:2] != ["cargo", "run"]:
            msg = f"not a `cargo run` command: {command!r}"
            raise UnsupportedCargoRunError(msg)

        separator = words.index("--") if "--" in words else len(words)
        cargo_args, args = words[2:separator], words[separator + 1 :]
        options = _options(cargo_args, command)
        labels = [
            Feature(label)
            for value in options.get("--features", [])
            for label in value.replace(",", " ").split()
        ]
        if "--example" not in options:
            msg = f"runs no example: {command!r}"
            raise UnsupportedCargoRunError(msg)

        features: Features
        if "--all-features" in options:
            features = AllFeatures()
        elif "--no-default-features" in options:
            features = OnlyFeatures(tuple(labels))
        else:
            features = DefaultFeatures(tuple(labels))

        return cls(
            example=options["--example"][-1],
            package=options["--package"][-1] if "--package" in options else None,
            features=features,
            args=tuple(args),
        )

    @override
    def __str__(self) -> str:
        """The command line, as a shell would take it."""
        words = [
            "cargo",
            "run",
            *(["-p", self.package] if self.package is not None else []),
            *_feature_args(self.features),
            "--example",
            self.example,
            *(["--", *self.args] if self.args else []),
        ]
        return shlex.join(words)


# The `cargo run` options `CargoRun` models, by long name: whether each takes a value.
_RUN_OPTIONS = {
    "--package": True,
    "--example": True,
    "--features": True,
    "--no-default-features": False,
    "--all-features": False,
}
_SHORT_OPTIONS = {"-p": "--package", "-F": "--features"}


def _options(words: list[str], command: str) -> dict[str, list[str]]:
    """`cargo run`'s options in `words`, each with its values in order.

    Raises:
        UnsupportedCargoRunError: An argument isn't one of `_RUN_OPTIONS`.
    """
    options: dict[str, list[str]] = {}
    remaining = iter(words)
    for word in remaining:
        name, has_value, value = word.partition("=")
        name = _SHORT_OPTIONS.get(name, name)
        if name not in _RUN_OPTIONS:
            msg = f"unsupported `cargo run` argument {word!r} in {command!r}"
            raise UnsupportedCargoRunError(msg)

        if _RUN_OPTIONS[name]:
            value = value if has_value else next(remaining, "")
            options.setdefault(name, []).append(value)
        else:
            _ = options.setdefault(name, [])
    return options


@dataclass(frozen=True)
class Example:
    """An example target of a package."""

    name: str
    # In a virtual workspace, cargo ignores feature flags unless `--package` is given.
    package: str
    required_features: tuple[Feature, ...]
    source: RustFile
    workspace: "Workspace" = field(repr=False, compare=False)

    def build(
        self, *, features: Features = _DEFAULT_FEATURES, target_dir: Path | None = None
    ) -> Path:
        """Build the example with `features`, returning its executable.

        Raises:
            RuntimeError: cargo reported no executable for the example.
        """
        args = [
            "build",
            "--quiet",
            "--message-format=json",
            f"--package={self.package}",
            *_feature_args(features),
            f"--example={self.name}",
            *([f"--target-dir={target_dir}"] if target_dir is not None else []),
        ]

        messages = (
            cast("dict[str, object]", json.loads(line))
            for line in self.workspace.run(*args).check().stdout.splitlines()
        )
        executable = next(
            (
                Path(cast("str", message["executable"]))
                for message in messages
                if message.get("reason") == "compiler-artifact"
                and message.get("executable") is not None
                and cast("dict[str, object]", message["target"])["name"] == self.name
            ),
            None,
        )

        if executable is None:
            msg = f"cargo built no executable for example {self.name}"
            raise RuntimeError(msg)

        return executable


@dataclass(frozen=True)
class Package:
    """A workspace package, as `cargo metadata` describes it."""

    name: str
    directory: Path
    examples: Mapping[str, Example]

    @classmethod
    def parse(cls, package: Mapping[str, object], workspace: "Workspace") -> Self:
        """Read one entry of `cargo metadata`'s `packages`, in `workspace`."""
        name = cast("str", package["name"])

        examples = (
            Example(
                name=cast("str", target["name"]),
                package=name,
                required_features=tuple(
                    Feature(label)
                    for label in cast("list[str]", target.get("required-features", []))
                ),
                source=RustFile(Path(cast("str", target["src_path"]))),
                workspace=workspace,
            )
            for target in cast("list[dict[str, object]]", package["targets"])
            if "example" in cast("list[str]", target["kind"])
        )

        return cls(
            name=name,
            directory=Path(cast("str", package["manifest_path"])).parent,
            examples={example.name: example for example in examples},
        )


@dataclass(frozen=True)
class CargoMetadata:
    """What `cargo metadata --no-deps` reports: the workspace's packages, by name."""

    packages: Mapping[str, Package]

    @classmethod
    def parse(cls, output: str, workspace: "Workspace") -> Self:
        """Read `workspace`'s `cargo metadata --format-version 1` output."""
        metadata = cast("dict[str, object]", json.loads(output))
        packages = (
            Package.parse(package, workspace)
            for package in cast("list[dict[str, object]]", metadata["packages"])
        )
        return cls(packages={package.name: package for package in packages})


class Cargo:
    """Finds cargo; entering it hands out a `CargoManager` to run it with."""

    def __enter__(self) -> "CargoManager":
        """Find cargo.

        Raises:
            CargoUnavailableError: `cargo` is not on PATH.
        """
        executable = shutil.which("cargo")
        if executable is None:
            msg = "cargo is not on PATH; install it with rustup: https://rustup.rs"
            raise CargoUnavailableError(msg)

        return CargoManager(Path(executable))

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        """Nothing to clean up: cargo holds no resources between runs."""


@dataclass(frozen=True)
class CargoManager:
    """A cargo known to be installed. `Cargo` makes one."""

    executable: Path

    def workspace(self, root: Path) -> "Workspace":
        """The cargo workspace at `root`."""
        return Workspace(self, root)

    def run(self, *args: str, cwd: Path | None = None) -> CargoOutput:
        """Run `cargo <args>` in `cwd`, or the current directory.

        A failing cargo is no error here; `CargoOutput.check` makes it one.
        """
        completed = subprocess.run(
            [self.executable, *args],
            cwd=cwd,
            check=False,
            capture_output=True,
            text=True,
        )
        return CargoOutput(
            stdout=completed.stdout,
            stderr=completed.stderr,
            exit_code=completed.returncode,
        )


@dataclass(frozen=True)
class Workspace:
    """A cargo workspace, which cargo runs in."""

    cargo: CargoManager = field(repr=False)
    root: Path

    def metadata(self) -> CargoMetadata:
        """The workspace's packages, as `cargo metadata` reports them."""
        output = self.run(
            "metadata", "--no-deps", "--format-version", "1", "--quiet"
        ).check()
        return CargoMetadata.parse(output.stdout, self)

    def package(self, name: str) -> Package:
        """The workspace's package called `name`.

        Raises:
            KeyError: The workspace has no such package.
        """
        return self.metadata().packages[name]

    def run(self, *args: str) -> CargoOutput:
        """Run `cargo <args>` in the workspace, returning what it printed."""
        return self.cargo.run(*args, cwd=self.root)
