"""Rust source files, and the docs in them.

```python
file = RustFile(path)
module_docs = file.source().module_docs()
if (docs := module_docs.read()) is not None:
    file.write(module_docs.write(edited(docs)))
```

Doc comments are found by their line prefix, not by parsing Rust: a line in a string
literal that starts with `//!` would count, and `/*! */` and `#![doc = "..."]` docs are
not seen.
"""

from dataclasses import dataclass
from pathlib import Path

from devtools.rust.rsdoc import ModuleDocs

_MODULE_DOC_PREFIX = "//!"


class UnsupportedDocLineError(ValueError):
    """A `//!` line without a space after the `//!`, which can't be written back."""


class AmbiguousModuleDocsError(ValueError):
    """The source has more than one separate run of `//!` lines."""


class MissingModuleDocsError(ValueError):
    """The source has no `//!` lines to replace."""


@dataclass(frozen=True)
class RustFile:
    """A Rust source file on disk."""

    path: Path

    def source(self) -> "RustSource":
        """The file's source, as it is now."""
        return RustSource(self.path.read_text())

    def write(self, source: "RustSource") -> None:
        """Save `source` as the file's contents."""
        _ = self.path.write_text(source.text)


@dataclass(frozen=True)
class RustSource:
    """The text of Rust source."""

    text: str

    def module_docs(self) -> "ModuleDocsView":
        """A view of the source's `//!` docs, to read them or write new ones."""
        return ModuleDocsView(self)


@dataclass(frozen=True)
class ModuleDocsView:
    """The `//!` docs of `source`."""

    source: RustSource

    def read(self) -> ModuleDocs | None:
        """The `//!` docs, if there are any.

        Raises:
            AmbiguousModuleDocsError: The `//!` lines come in more than one run.
            UnsupportedDocLineError: A `//!` line has no space after the `//!`.
        """
        lines = self._lines()
        if lines is None:
            return None

        text_lines = self.source.text.split("\n")
        return ModuleDocs("".join(f"{self._doc_text(text_lines[i])}\n" for i in lines))

    def write(self, docs: ModuleDocs) -> RustSource:
        """The source with `docs` as its `//!` docs, every other line unchanged.

        Raises:
            AmbiguousModuleDocsError: The `//!` lines come in more than one run.
            MissingModuleDocsError: There are no `//!` docs to replace.
        """
        lines = self._lines()
        if lines is None:
            msg = f"no `{_MODULE_DOC_PREFIX}` docs to replace"
            raise MissingModuleDocsError(msg)

        text_lines = self.source.text.split("\n")
        doc_lines = (
            f"{_MODULE_DOC_PREFIX} {line}" if line else _MODULE_DOC_PREFIX
            for line in docs.markdown.removesuffix("\n").split("\n")
        )

        spliced = [*text_lines[: lines.start], *doc_lines, *text_lines[lines.stop :]]
        return RustSource("\n".join(spliced))

    def _lines(self) -> range | None:
        """The line numbers of the `//!` docs, if any.

        Raises:
            AmbiguousModuleDocsError: The `//!` lines come in more than one run.
        """
        numbers = [
            number
            for number, line in enumerate(self.source.text.split("\n"))
            if line.startswith(_MODULE_DOC_PREFIX)
        ]
        if not numbers:
            return None

        if numbers[-1] - numbers[0] + 1 != len(numbers):
            msg = f"`{_MODULE_DOC_PREFIX}` lines in more than one run"
            raise AmbiguousModuleDocsError(msg)

        return range(numbers[0], numbers[-1] + 1)

    @staticmethod
    def _doc_text(line: str) -> str:
        if line == _MODULE_DOC_PREFIX:
            return ""

        if not line.startswith(f"{_MODULE_DOC_PREFIX} "):
            msg = (
                f"expected `{_MODULE_DOC_PREFIX} ` or a bare `{_MODULE_DOC_PREFIX}`: "
                f"{line!r}"
            )
            raise UnsupportedDocLineError(msg)

        return line.removeprefix(f"{_MODULE_DOC_PREFIX} ")
