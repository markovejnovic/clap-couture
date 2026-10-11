"""Rust doc comments as markdown: find blocks in docs, and edit them.

Parsing is pulldown-cmark's, rustdoc's own parser, through pyromark. An edit splices
markdown into the docs at a block's position, so everything else stays as written.

```python
docs = ModuleDocs(markdown)
fence = docs.fences("terminal")[0]
docs = replaced(docs, fence, fence.with_content("output"))
```
"""

from collections.abc import Iterator
from dataclasses import dataclass
from functools import cached_property

import pyromark

# The extensions rustdoc enables on top of CommonMark.
_RUSTDOC_OPTIONS = (
    pyromark.Options.ENABLE_TABLES
    | pyromark.Options.ENABLE_FOOTNOTES
    | pyromark.Options.ENABLE_STRIKETHROUGH
    | pyromark.Options.ENABLE_TASKLISTS
    | pyromark.Options.ENABLE_SMART_PUNCTUATION
)


@dataclass(frozen=True)
class Span:
    """Where a block is in the docs' markdown, in UTF-8 bytes."""

    start: int
    end: int


@dataclass(frozen=True)
class Paragraph:
    """A paragraph; `markdown` is its source, inline markup included."""

    span: Span
    markdown: str


@dataclass(frozen=True)
class Heading:
    """A heading, e.g. `# Example Output` at `level` 1."""

    span: Span
    level: int
    markdown: str


@dataclass(frozen=True)
class Fence:
    """A fenced code block: ```` ```info ```` around `content`."""

    span: Span
    info: str
    content: str
    markdown: str

    def with_content(self, content: str) -> str:
        """This fence's markdown around `content`, keeping its opening and closing."""
        lines = self.markdown.split("\n")
        return "\n".join([lines[0], *([content] if content else []), lines[-1]])


type Block = Paragraph | Heading | Fence
"""A top-level block of the docs; others are kept as written, but not modelled."""


class ForeignBlockError(ValueError):
    """A block that isn't one of the docs' own, e.g. from an earlier version."""


@dataclass(frozen=True)
class ModuleDocs:
    """`//!` docs, as markdown without the `//!` prefixes."""

    markdown: str

    @cached_property
    def blocks(self) -> tuple[Block, ...]:
        """The top-level blocks pulldown-cmark finds, in order."""
        return tuple(self._parse_blocks())

    def fences(self, info: str) -> tuple[Fence, ...]:
        """The fences labelled `info`, e.g. `terminal` for ```` ```terminal ````."""
        return tuple(
            block
            for block in self.blocks
            if isinstance(block, Fence) and block.info == info
        )

    def before(self, block: Block) -> Iterator[Block]:
        """The blocks above `block`, nearest first.

        Raises:
            ForeignBlockError: `block` isn't one of these docs' blocks.
        """
        return reversed(self.blocks[: self._index(block)])

    def _index(self, block: Block) -> int:
        if block not in self.blocks:
            msg = f"not a block of these docs: {block}"
            raise ForeignBlockError(msg)

        return self.blocks.index(block)

    def _parse_blocks(self) -> Iterator[Block]:
        source = self.markdown.encode()
        depth = 0
        start = 0
        content: list[str] = []
        for event, span in pyromark.events_with_range(
            self.markdown, options=_RUSTDOC_OPTIONS
        ):
            match event:
                case {"Start": _}:
                    if depth == 0:
                        start, content = span["start"], []
                    depth += 1
                case {"End": ended}:
                    depth -= 1
                    if depth == 0 and (
                        block := self._block(
                            ended, Span(start, span["end"]), source, content
                        )
                    ):
                        yield block
                case {"Text": text} if depth == 1:
                    content.append(text)
                case _:
                    pass

    @staticmethod
    def _block(
        ended: object, span: Span, source: bytes, content: list[str]
    ) -> Block | None:
        """The block that just ended, if it's one of the kinds that are modelled."""
        markdown = source[span.start : span.end].decode().rstrip("\n")
        match ended:
            case "Paragraph":
                return Paragraph(span, markdown)
            case {"Heading": str(level)}:
                return Heading(span, int(level[1:]), markdown)
            case "CodeBlock" if markdown.startswith(("```", "~~~")):
                info = markdown.split("\n", 1)[0].lstrip("`~").strip()
                body = "".join(content).removesuffix("\n")
                return Fence(span, info, body, markdown)
            case _:
                return None


def replaced(docs: ModuleDocs, block: Block, markdown: str) -> ModuleDocs:
    """`docs` with `block` replaced by `markdown`, everything else unchanged.

    Raises:
        ForeignBlockError: `block` isn't one of `docs`' blocks.
    """
    if block not in docs.blocks:
        msg = f"not a block of these docs: {block}"
        raise ForeignBlockError(msg)

    source = docs.markdown.encode()
    old = source[block.span.start : block.span.end]
    trailing = old[len(old.rstrip(b"\n")) :]
    spliced = (
        source[: block.span.start]
        + markdown.encode()
        + trailing
        + source[block.span.end :]
    )
    return ModuleDocs(spliced.decode())
