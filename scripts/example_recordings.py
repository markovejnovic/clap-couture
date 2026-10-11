"""Where the repo keeps its example recordings, shared by the recording scripts.

An example run with its default features is recorded to
`assets/<example>-{dark,light}.gif` and `assets/<example>.txt`. A run with other
features, e.g. another prompt backend, is a variant: only its text is recorded, to
`assets/<example>.<variant>.txt`, and its tape is `<example>.<variant>.tape` when there
is one, since backends take different keys.
"""

from dataclasses import dataclass
from itertools import groupby
from pathlib import Path
from typing import Self

from devtools.rust.cargo import AllFeatures, CargoRun, DefaultFeatures, OnlyFeatures
from devtools.vhs import Recording

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
PACKAGE = "clap-couture"
# Catppuccin's colors on the background and text of rustdoc's code blocks, so the GIFs
# sit in the docs like code. Ayu, rustdoc's third theme, shows the dark GIFs.
THEMES: dict[str, dict[str, str]] = {
    "dark": {
        "name": "rustdoc dark",
        "background": "#2a2a2a",
        "foreground": "#dddddd",
        "cursor": "#f5e0dc",
        "selection": "#585b70",
        "black": "#45475a",
        "red": "#f38ba8",
        "green": "#a6e3a1",
        "yellow": "#f9e2af",
        "blue": "#89b4fa",
        "magenta": "#f5c2e7",
        "cyan": "#94e2d5",
        "white": "#bac2de",
        "brightBlack": "#585b70",
        "brightRed": "#f38ba8",
        "brightGreen": "#a6e3a1",
        "brightYellow": "#f9e2af",
        "brightBlue": "#89b4fa",
        "brightMagenta": "#f5c2e7",
        "brightCyan": "#94e2d5",
        "brightWhite": "#a6adc8",
    },
    "light": {
        "name": "rustdoc light",
        "background": "#eaeaea",
        "foreground": "#000000",
        "cursor": "#dc8a78",
        "selection": "#acb0be",
        "black": "#5c5f77",
        "red": "#d20f39",
        "green": "#40a02b",
        "yellow": "#df8e1d",
        "blue": "#1e66f5",
        "magenta": "#ea76cb",
        "cyan": "#179299",
        "white": "#acb0be",
        "brightBlack": "#6c6f85",
        "brightRed": "#d20f39",
        "brightGreen": "#40a02b",
        "brightYellow": "#df8e1d",
        "brightBlue": "#1e66f5",
        "brightMagenta": "#ea76cb",
        "brightCyan": "#179299",
        "brightWhite": "#bcc0cc",
    },
}
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
    def gifs(self) -> dict[str, Path]:
        """The recording's GIF per theme; only default-feature runs get them."""
        if self.variant is not None:
            return {}
        return {theme: ASSETS / f"{self.example}-{theme}.gif" for theme in THEMES}

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
