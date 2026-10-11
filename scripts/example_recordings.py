"""Where the repo keeps its example recordings, shared by the recording scripts.

An example run with its default features is recorded to
`assets/<example>-{dark,light}.gif` and `assets/<example>.txt`; a `--help` screen gets
a still `.png` in place of the GIF. A run with other
features, e.g. another prompt backend, is a variant: only its text is recorded, to
`assets/<example>.<variant>.txt`, and its tape is `<example>.<variant>.tape` when there
is one, since backends take different keys.
"""

from dataclasses import dataclass
from itertools import groupby
from pathlib import Path
from typing import Self

from devtools.backdrop import Backdrop
from devtools.rust.cargo import AllFeatures, CargoRun, DefaultFeatures, OnlyFeatures
from devtools.vhs import Recording

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
PACKAGE = "clap-couture"


@dataclass(frozen=True)
class Theme:
    """How a recording looks in one color scheme: vhs's theme, on a backdrop."""

    vhs: str
    backdrop: Backdrop


# Catppuccin on the brand's pink fading to violet: deep for dark pages, pale for light.
THEMES: dict[str, Theme] = {
    "dark": Theme("Catppuccin Mocha", Backdrop(start="#db2777", end="#7c3aed")),
    "light": Theme("Catppuccin Latte", Backdrop(start="#f9a8d4", end="#c4b5fd")),
}
# `--help` screens: nothing moves, so they get a still of their last frame, not a GIF.
STILLS = frozenset(
    {"01-orbit-disorganized", "01-orbit-organized", "02-markdown-simple"}
)
# What separates the screens in a text capture, as in vhs's own.
_SCREEN_RULE = "─" * 80


@dataclass(frozen=True)
class ExampleRun(CargoRun):
    """A `cargo run` of one of the repo's examples, and where its recording goes."""

    @classmethod
    def from_argument(cls, argument: str) -> Self:
        """The run an argument names: an example as is, or a `cargo run` command."""
        if argument.startswith("cargo "):
            return cls.parse(argument)
        return cls(example=argument, package=PACKAGE)

    @property
    def variant(self) -> str | None:
        """Its features' name unless they're the defaults: `interactive-inquire`."""
        match self.features:
            case DefaultFeatures(extra=extra):
                labels = (feature.label for feature in extra)
                return "+".join(["default", *labels]) if extra else None
            case OnlyFeatures(features=only):
                return "+".join(feature.label for feature in only) or "none"
            case AllFeatures():
                return "all"

    @property
    def stem(self) -> str:
        """The recording files' shared name: `03-interactive.interactive-inquire`."""
        return (
            self.example if self.variant is None else f"{self.example}.{self.variant}"
        )

    @property
    def capture(self) -> Path:
        """Where the recording is captured as text."""
        return ASSETS / f"{self.stem}.txt"

    @property
    def images(self) -> dict[str, Path]:
        """The recording's GIF, or still, per theme; only default runs get them."""
        if self.variant is not None:
            return {}
        extension = "png" if self.example in STILLS else "gif"
        return {
            theme: ASSETS / f"{self.example}-{theme}.{extension}" for theme in THEMES
        }

    @property
    def target_dir(self) -> Path | None:
        """Where a variant builds: apart, so other features' builds don't undo it."""
        return (
            None if self.variant is None else ROOT / "target" / "record" / self.variant
        )

    def tape(self, tapes: Path) -> Path:
        """The run's own tape in `tapes`, if it has one, else its example's."""
        own = tapes / f"{self.stem}.tape"
        return own if own.exists() else tapes / f"{self.example}.tape"

    def write_capture(self, recording: Recording) -> Path:
        """Save `recording`'s screens as the run's text capture, returning where.

        A screen repeated back to back is kept once: how many times vhs captures an
        unchanged screen depends on timing, and the capture must not.
        """
        screens = (screen for screen, _ in groupby(recording.screens))
        _ = self.capture.write_text(
            "".join(f"{screen.text}\n{_SCREEN_RULE}\n" for screen in screens)
        )
        return self.capture
