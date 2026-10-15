#!/usr/bin/env python3
"""Extract the documented Noobietoria Studio API surface into a normalised spec file.

Source of truth: spec/source/api.md (a vendored copy of docs/api.md from the
upstream Noobietoria/Docs repository). This tool never invents API: it only
normalises what the documentation already states, so the Rust test suite can
assert that the runtime implements *exactly* the documented surface.

Usage:
    python spec/tools/extract_signatures.py

Outputs:
    spec/api-signatures.txt    human readable, one signature per line
    spec/api-signatures.json   machine readable, consumed by nb-api tests
"""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass, asdict, field
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SOURCE = REPO / "spec" / "source" / "api.md"
OVERRIDES = REPO / "spec" / "signature-overrides.json"
OUT_TXT = REPO / "spec" / "api-signatures.txt"
OUT_JSON = REPO / "spec" / "api-signatures.json"

# `<img ... alt="Name icon" ... /> Name` section headings
SECTION_RE = re.compile(r'^##\s+<img[^>]*alt="([^"]+?) icon"[^>]*/>\s*(.+?)\s*$')
FENCE_RE = re.compile(r"^```")
# `Service.Method(`, `Service:Method(`, or callable `Service(`
CALL_RE = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)(?:([.:])([A-Za-z_][A-Za-z0-9_]*))?\(")

VARARGS = "..."


@dataclass
class Signature:
    service: str          # e.g. "CurrencyService"
    method: str           # e.g. "Add"; "" for a callable service
    params: list[str] = field(default_factory=list)
    varargs: bool = False
    source_section: str = ""
    raw: str = ""

    @property
    def arity(self) -> int:
        return len(self.params)

    @property
    def key(self) -> str:
        return f"{self.service}.{self.method}" if self.method else self.service


def split_top_level(text: str) -> list[str]:
    """Split on commas that are not inside quotes or nested brackets."""
    parts: list[str] = []
    depth = 0
    quote: str | None = None
    current: list[str] = []
    for ch in text:
        if quote:
            current.append(ch)
            if ch == quote:
                quote = None
            continue
        if ch in "\"'":
            quote = ch
            current.append(ch)
            continue
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append("".join(current))
            current = []
            continue
        current.append(ch)
    tail = "".join(current)
    if tail.strip() or parts:
        parts.append(tail)
    return parts


def parse_param(token: str) -> str:
    """`Id: int` -> `Id`; `Name` -> `Name`."""
    token = token.strip()
    if not token:
        return ""
    # Strip a Luau type annotation, but keep `:` inside quoted defaults.
    token = token.split(":", 1)[0].strip()
    return token


def collect_signatures(text: str) -> list[Signature]:
    lines = text.splitlines()
    found: list[Signature] = []
    section = "(none)"
    in_fence = False
    i = 0

    while i < len(lines):
        line = lines[i]

        heading = SECTION_RE.match(line)
        if heading and not in_fence:
            section = heading.group(2).strip()
            i += 1
            continue

        if FENCE_RE.match(line.strip()):
            in_fence = not in_fence
            i += 1
            continue

        if not in_fence:
            i += 1
            continue

        stripped = line.strip()
        # Skip comments and blank lines inside code fences.
        if not stripped or stripped.startswith("--") or stripped.startswith("//"):
            i += 1
            continue

        match = CALL_RE.match(line)
        if not match:
            i += 1
            continue

        service = match.group(1)
        method = match.group(3) or ""

        # Gather the full call, which may span several lines.
        call = line
        open_paren = call.find("(")
        depth = call.count("(") - call.count(")")
        while depth > 0 and i + 1 < len(lines):
            i += 1
            nxt = lines[i].strip()
            call += " " + nxt
            depth += nxt.count("(") - nxt.count(")")

        inner_start = call.find("(", open_paren)
        inner_end = call.rfind(")")
        if inner_start == -1 or inner_end <= inner_start:
            i += 1
            continue
        inner = call[inner_start + 1:inner_end]

        params: list[str] = []
        varargs = False
        for token in split_top_level(inner):
            token = token.strip()
            if not token:
                continue
            if token == VARARGS:
                varargs = True
                continue
            name = parse_param(token)
            if name:
                params.append(name)

        # A call whose arguments are all string literals is a usage example,
        # not a signature; record it but flag via a stable synthetic name.
        if all(p.startswith(("'", '"')) for p in params) and params:
            params = [f"arg{n + 1}" for n in range(len(params))]

        found.append(
            Signature(
                service=service,
                method=method,
                params=params,
                varargs=varargs,
                source_section=section,
                raw=" ".join(call.split()),
            )
        )
        i += 1

    return found


def apply_overrides(sigs: list[Signature]) -> list[Signature]:
    """Merge operator-supplied corrections.

    Overrides exist so that documented-but-odd signatures (a service invoked
    directly, arguments shown as literals) can be given real parameter names
    without editing the vendored upstream documentation.
    """
    if not OVERRIDES.exists():
        return sigs
    data = json.loads(OVERRIDES.read_text(encoding="utf-8"))
    by_key = {s.key: s for s in sigs}
    for entry in data.get("signatures", []):
        key = entry["key"]
        existing = by_key.get(key)
        if existing is None:
            existing = Signature(service=entry["service"], method=entry.get("method", ""))
            sigs.append(existing)
            by_key[key] = existing
        if "params" in entry:
            existing.params = list(entry["params"])
        if "varargs" in entry:
            existing.varargs = bool(entry["varargs"])
        if "reason" in entry:
            existing.source_section = existing.source_section or entry["reason"]
    return sigs


def main() -> int:
    if not SOURCE.exists():
        print(f"error: missing vendored documentation at {SOURCE}", file=sys.stderr)
        return 2

    text = SOURCE.read_text(encoding="utf-8")
    sigs = apply_overrides(collect_signatures(text))

    # De-duplicate: the docs repeat a few signatures across sections.
    unique: dict[str, Signature] = {}
    for sig in sigs:
        unique.setdefault(sig.key, sig)
    ordered = sorted(unique.values(), key=lambda s: (s.service, s.method))

    services = sorted({s.service for s in ordered})
    OUT_TXT.write_text(
        "\n".join(sig.raw for sig in ordered) + "\n", encoding="utf-8"
    )
    OUT_JSON.write_text(
        json.dumps(
            {
                "source": "spec/source/api.md",
                "upstream": "https://noobietoria.github.io/Docs/",
                "signature_count": len(ordered),
                "service_count": len(services),
                "services": services,
                "signatures": [asdict(s) | {"arity": s.arity} for s in ordered],
            },
            indent=2,
            ensure_ascii=False,
        )
        + "\n",
        encoding="utf-8",
    )

    print(f"signatures: {len(ordered)}")
    print(f"services:   {len(services)}")
    print(f"wrote {OUT_TXT.relative_to(REPO)} and {OUT_JSON.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
