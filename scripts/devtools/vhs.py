"""charmbracelet/vhs: recording tapes, and the screens they recorded.

```python
with Vhs() as vhs:
    recording = vhs.record(Tape(commands, outputs=(Path("demo.gif"),)))
last_screen = recording.screens[-1]
```
"""

import json
import re
import shutil
import subprocess
import tempfile
from collections.abc import Mapping
from contextlib import ExitStack
from dataclasses import dataclass, field, replace
from pathlib import Path
from types import TracebackType
from typing import Self

# vhs draws a rule of these between the screens of a `.txt` capture.
_SCREEN_RULE = re.compile(r"^─+$\n?", re.MULTILINE)
# The prompt of the shell vhs records in.
_SHELL_PROMPT = "> "


class VhsUnavailableError(RuntimeError):
    """`vhs` is not on PATH."""


class ScrolledAwayError(RuntimeError):
    """The recording's last screen no longer shows the command that was typed."""


@dataclass(frozen=True)
class Tape:
    """A tape: its `commands` as in a `.tape` file, after its outputs and settings."""

    commands: str
    outputs: tuple[Path, ...] = ()
    settings: Mapping[str, str | int] = field(default_factory=dict[str, str | int])

    def render(self) -> str:
        """The tape's source."""
        outputs = (f"Output {json.dumps(str(output))}" for output in self.outputs)
        settings = (
            f"Set {name} {json.dumps(value) if isinstance(value, str) else value}"
            for name, value in self.settings.items()
        )
        return "\n".join([*outputs, *settings, self.commands])


@dataclass(frozen=True)
class Screen:
    """What the terminal showed at one point of a recording, as text."""

    text: str


@dataclass(frozen=True)
class Session:
    """A command typed at the shell prompt, and what it printed before the next one."""

    command: str
    output: str


@dataclass(frozen=True)
class Recording:
    """The screens a tape recorded, in order, without the blank ones."""

    screens: tuple[Screen, ...]

    @classmethod
    def parse(cls, capture: str) -> Self:
        """Read a vhs `.txt` capture."""
        return cls(
            screens=tuple(
                Screen(text=screen.rstrip("\n"))
                for screen in _SCREEN_RULE.split(capture)
                if screen.strip()
            )
        )

    def session(self) -> Session:
        """The command the last screen shows, and its output.

        Raises:
            ScrolledAwayError: The terminal is too short to show the whole session, so
                the typed command scrolled off the top; raise the tape's `Set Height`.
        """
        command, *output = self.screens[-1].text.split("\n")
        if not command.startswith(_SHELL_PROMPT):
            msg = "the typed command scrolled away; raise the tape's `Set Height`"
            raise ScrolledAwayError(msg)

        # The shell prompts again once the command is done.
        while output and output[-1].strip() in ("", _SHELL_PROMPT.strip()):
            _ = output.pop()

        return Session(
            command=command.removeprefix(_SHELL_PROMPT),
            output="\n".join(line.rstrip() for line in output),
        )


class Vhs:
    """Finds vhs; entering it hands out a `VhsManager`, and a tape directory."""

    def __init__(self) -> None:
        """Find vhs once entered."""
        super().__init__()
        self._resources = ExitStack()

    def __enter__(self) -> "VhsManager":
        """Find vhs, and make a directory for the tapes it records.

        Raises:
            VhsUnavailableError: `vhs` is not on PATH.
        """
        executable = shutil.which("vhs")
        if executable is None:
            msg = "vhs is not on PATH; see https://github.com/charmbracelet/vhs#installation"
            raise VhsUnavailableError(msg)
        tapes = self._resources.enter_context(
            tempfile.TemporaryDirectory(prefix="vhs-tapes-")
        )
        return VhsManager(Path(executable), Path(tapes))

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        """Delete the tapes."""
        self._resources.close()


@dataclass(frozen=True)
class VhsManager:
    """A vhs known to be installed, writing its tapes to `tapes`. `Vhs` makes one."""

    executable: Path
    tapes: Path

    def record(self, tape: Tape, *, env: Mapping[str, str] | None = None) -> Recording:
        """Record `tape`, writing its outputs; its shell starts in `env`."""
        workdir = Path(tempfile.mkdtemp(dir=self.tapes))
        capture = workdir / "capture.txt"
        source = workdir / "recording.tape"

        _ = source.write_text(replace(tape, outputs=(*tape.outputs, capture)).render())
        _ = subprocess.run([self.executable, "--quiet", source], check=True, env=env)

        return Recording.parse(capture.read_text())
