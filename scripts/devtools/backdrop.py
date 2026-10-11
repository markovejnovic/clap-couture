"""A gradient backdrop with a window's drop shadow, for vhs's `Set MarginFill`.

vhs draws the window over its margin flat; the shadow has to be in the fill.

```python
backdrop = Backdrop(start="#db2777", end="#7c3aed")
backdrop.save(path, size=(1200, 800), window=Window(margin=64, radius=24))
```
"""

from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

# How far the shadow falls below the window, how soft it is, and how dark, out of 255.
_SHADOW_DROP = 24
_SHADOW_BLUR = 36
_SHADOW_ALPHA = 110


@dataclass(frozen=True)
class Window:
    """Where vhs draws its window: `margin` in from each edge, with rounded corners."""

    margin: int
    radius: int


@dataclass(frozen=True)
class Backdrop:
    """A diagonal gradient from `start`, top left, to `end`, bottom right."""

    start: str
    end: str

    def save(self, path: Path, *, size: tuple[int, int], window: Window) -> None:
        """Write the backdrop for a `size` recording to `path`, shadowing `window`."""
        width, height = size
        # The mean of a top-to-bottom and a left-to-right ramp runs corner to corner.
        across = [255 * x // (width - 1) for x in range(width)]
        rows = (
            bytes((step + 255 * y // (height - 1)) // 2 for step in across)
            for y in range(height)
        )
        ramp = Image.frombytes("L", size, b"".join(rows))
        image = Image.composite(
            Image.new("RGB", size, self.end), Image.new("RGB", size, self.start), ramp
        )

        shadow = Image.new("L", size, 0)
        ImageDraw.Draw(shadow).rounded_rectangle(
            (
                window.margin,
                window.margin + _SHADOW_DROP,
                width - window.margin,
                height - window.margin + _SHADOW_DROP,
            ),
            window.radius,
            fill=_SHADOW_ALPHA,
        )
        shadow = shadow.filter(ImageFilter.GaussianBlur(_SHADOW_BLUR))
        image.paste("black", mask=shadow)
        image.save(path)
