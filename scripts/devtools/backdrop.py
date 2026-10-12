"""A flat backdrop with a window's drop shadow, for vhs's `Set MarginFill`.

vhs draws the window over its margin flat; the shadow has to be in the fill.

```python
backdrop = Backdrop(color="#0d1117")
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
    """A flat `color`."""

    color: str

    def save(self, path: Path, *, size: tuple[int, int], window: Window) -> None:
        """Write the backdrop for a `size` recording to `path`, shadowing `window`."""
        width, height = size
        image = Image.new("RGB", size, self.color)

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
