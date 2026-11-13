#!/usr/bin/env python3
"""Deterministic Ferrioxid/Ferrum to Fexu rename.

Dry-run is the default. ``--apply`` writes text replacements and renames
paths with ``git mv`` so history stays attached to the new paths.

The mapping is applied in the order below. Longer Ferrioxid spellings are
replaced before Ferrum spellings. A second run finds nothing to change.

Live GitHub URLs of ``Iam-Sanja/ferrioxid`` are left in place because the
repository has not been renamed. Historical plan and CodeQL evidence files
are not rewritten. ``vendor/`` is not rewritten.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

MAPPINGS: tuple[tuple[str, str], ...] = (
    ("FERRIOXID", "FEXU"),
    ("Ferrioxid", "Fexu"),
    ("ferrioxid", "fexu"),
    ("FERRUM", "FEXU"),
    ("Ferrum", "Fexu"),
    ("ferrum", "fexu"),
)

# The development CSRF key must stay 32 bytes. Replacing the brand inside it
# would shorten the literal and break the [u8; 32] assignment.
FIXED_LITERALS: tuple[tuple[str, str], ...] = (
    (
        'b"ferrioxid-dev-csrf-key-32-bytes!"',
        'b"fexu-dev-csrf-key-32-bytes!!!!!!"',
    ),
)

PROTECTED_URL = "Iam-Sanja/ferrioxid"
# Longer phrases are held before the mapping runs. Historical filenames stay
# linked from other documents, and the live repository URL stays unchanged.
PROTECTED_PHRASES = (
    "https://github.com/Iam-Sanja/ferrioxid",
    "Iam-Sanja/ferrioxid",
    "docs/ferrioxid-rename-plan.md",
    "docs/ferrum-legacy-removal.md",
    "ferrioxid-rename-plan.md",
    "ferrum-legacy-removal.md",
)
PLACEHOLDER_PREFIX = "\ue000FEXUURL"
PLACEHOLDER_SUFFIX = "\ue001"

# These files keep old names on purpose. The scanner reports them as history
# or as this tool's own mapping table.
EXCLUDED_CONTENT = {
    "CHANGELOG.md",
    "docs/ferrioxid-rename-plan.md",
    "docs/ferrum-legacy-removal.md",
    "docs/fexu-rename-plan.md",
    "docs/security/codeql-review.md",
    "docs/security/codeql-alerts.csv",
    "scripts/rename_to_fexu.py",
}
EXCLUDED_RENAME = {
    "docs/ferrioxid-rename-plan.md",
    "docs/ferrum-legacy-removal.md",
}

SKIP_DIRS = {
    ".git",
    "target",
    "node_modules",
    ".venv",
    "vendor",
    "playwright-report",
    "test-results",
    "tmp",
    "temp",
    ".ferrioxid",
    ".fexu",
}
BINARY_SUFFIXES = {
    ".png",
    ".jpg",
    ".jpeg",
    ".gif",
    ".webp",
    ".ico",
    ".pdf",
    ".zip",
    ".gz",
    ".wasm",
    ".woff",
    ".woff2",
    ".exe",
    ".dll",
    ".so",
    ".dylib",
    ".pyc",
    ".sqlite",
    ".sqlite3",
    ".db",
    ".bin",
    ".lockb",
}
MAX_TEXT_BYTES = 8 * 1024 * 1024


def posix(path: Path) -> str:
    return path.as_posix()


def map_text(text: str) -> tuple[str, dict[str, int]]:
    counts = {old: 0 for old, _new in MAPPINGS}
    for old, new in FIXED_LITERALS:
        found = text.count(old)
        if found:
            counts["ferrioxid"] = counts.get("ferrioxid", 0) + found
            text = text.replace(old, new)
    protected: list[str] = []
    phrases = tuple(sorted(PROTECTED_PHRASES, key=len, reverse=True))
    cursor = 0
    pieces: list[str] = []
    while cursor <= len(text):
        found: int | None = None
        phrase = ""
        for candidate in phrases:
            index = text.find(candidate, cursor)
            if index >= 0 and (found is None or index < found):
                found = index
                phrase = candidate
        if found is None:
            pieces.append(text[cursor:])
            break
        pieces.append(text[cursor:found])
        token = f"{PLACEHOLDER_PREFIX}{len(protected)}{PLACEHOLDER_SUFFIX}"
        protected.append(text[found : found + len(phrase)])
        pieces.append(token)
        cursor = found + len(phrase)
    text = "".join(pieces)
    for old, new in MAPPINGS:
        counts[old] += text.count(old)
        text = text.replace(old, new)
    for index, original in enumerate(protected):
        text = text.replace(f"{PLACEHOLDER_PREFIX}{index}{PLACEHOLDER_SUFFIX}", original)
    return text, counts


def map_component(name: str) -> str:
    for old, new in MAPPINGS:
        name = name.replace(old, new)
    return name


def should_skip_dir(name: str) -> bool:
    return name in SKIP_DIRS


def is_binary_candidate(path: Path) -> bool:
    return path.suffix.lower() in BINARY_SUFFIXES


def read_text(path: Path) -> str | None:
    if is_binary_candidate(path):
        return None
    try:
        size = path.stat().st_size
    except OSError:
        return None
    if size > MAX_TEXT_BYTES:
        return None
    data = path.read_bytes()
    if b"\0" in data:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def write_text(path: Path, original: bytes, text: str) -> None:
    payload = text.encode("utf-8")
    newline = b"\r\n" if b"\r\n" in original and b"\n" in original else None
    if newline is None:
        path.write_bytes(payload)
        return
    # decode already turned CRLF into the logical text when Python's decoder
    # keeps \n only if we decoded as utf-8 without newline translation.
    # read_bytes + decode keeps \r\n as two characters, so payload is faithful.
    path.write_bytes(payload)


def is_git_metadata(path: Path, root: Path) -> bool:
    try:
        relative_path = path.relative_to(root)
    except ValueError:
        return False
    return bool(relative_path.parts) and relative_path.parts[0] == ".git"


def iter_files(root: Path):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [name for name in dirnames if not should_skip_dir(name)]
        current = Path(dirpath)
        for filename in filenames:
            path = current / filename
            if filename == ".git" or is_git_metadata(path, root):
                continue
            yield path


def relative(root: Path, path: Path) -> Path:
    return path.relative_to(root)


def tracked_paths(root: Path) -> set[str]:
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=root,
        check=True,
        capture_output=True,
    )
    raw = result.stdout.decode("utf-8", errors="surrogateescape")
    return {item for item in raw.split("\0") if item}


def planned_renames(root: Path) -> list[tuple[Path, Path]]:
    moves: list[tuple[Path, Path]] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [name for name in dirnames if not should_skip_dir(name)]
        current = Path(dirpath)
        entries = list(dirnames) + list(filenames)
        for name in entries:
            source = current / name
            rel = relative(root, source)
            if posix(rel) in EXCLUDED_RENAME:
                continue
            renamed = map_component(name)
            if renamed != name:
                moves.append((rel, rel.with_name(renamed)))
    moves.sort(key=lambda item: (-len(item[0].parts), posix(item[0])))
    targets: dict[str, str] = {}
    for source, dest in moves:
        key = posix(dest)
        if key in targets:
            raise SystemExit(f"Rename collision: {targets[key]} and {posix(source)} both become {key}")
        targets[key] = posix(source)
    return moves


def git_status_lines(root: Path) -> list[str]:
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return [line for line in result.stdout.splitlines() if line.strip()]


def assert_clean_enough(root: Path) -> None:
    allowed = {"scripts/rename_to_fexu.py"}
    unexpected = []
    for line in git_status_lines(root):
        path = line[3:].strip()
        if " -> " in path:
            path = path.split(" -> ", 1)[1]
        path = path.strip('"')
        if path.replace("\\", "/") not in allowed:
            unexpected.append(line)
    if unexpected:
        raise SystemExit("Working tree has changes other than the rename script:\n" + "\n".join(unexpected))


def apply_content(root: Path) -> tuple[int, dict[str, int], int]:
    files_changed = 0
    totals = {old: 0 for old, _new in MAPPINGS}
    skipped_binary = 0
    for path in iter_files(root):
        rel = posix(relative(root, path))
        if rel in EXCLUDED_CONTENT:
            continue
        original = path.read_bytes() if path.is_file() else b""
        text = read_text(path)
        if text is None:
            if path.is_file() and not is_binary_candidate(path) and b"\0" in original:
                skipped_binary += 1
            continue
        updated, counts = map_text(text)
        for key, value in counts.items():
            totals[key] += value
        if updated != text:
            write_text(path, original, updated)
            files_changed += 1
    return files_changed, totals, skipped_binary


def apply_renames(root: Path, moves: list[tuple[Path, Path]], tracked: set[str]) -> int:
    renamed = 0
    for source, dest in moves:
        source_path = root / source
        dest_path = root / dest
        if not source_path.exists():
            continue
        if dest_path.exists():
            raise SystemExit(f"Refusing to replace existing path {posix(dest)}")
        source_key = posix(source)
        tracked_hit = source_key in tracked or any(
            item.startswith(source_key + "/") for item in tracked
        )
        if tracked_hit:
            subprocess.run(
                ["git", "mv", "--", source_key, posix(dest)],
                cwd=root,
                check=True,
            )
        else:
            dest_path.parent.mkdir(parents=True, exist_ok=True)
            os.rename(source_path, dest_path)
        renamed += 1
    local_old = root / ".ferrioxid"
    local_new = root / ".fexu"
    if local_old.is_dir() and not local_new.exists():
        os.rename(local_old, local_new)
        renamed += 1
    return renamed


def classify_hit(path: str) -> str:
    if path == "scripts/rename_to_fexu.py":
        return "tooling"
    if path in EXCLUDED_CONTENT or path in EXCLUDED_RENAME:
        return "historical"
    if path.startswith("vendor/"):
        return "third-party"
    return "unwanted"


def scan(root: Path) -> dict[str, list[str]]:
    groups = {"unwanted": [], "historical": [], "third-party": [], "tooling": [], "github-url": []}
    needles = [old for old, _new in MAPPINGS]
    extra_roots = [root / "vendor"]
    paths = list(iter_files(root))
    for extra in extra_roots:
        if extra.is_dir():
            for dirpath, dirnames, filenames in os.walk(extra):
                dirnames[:] = [name for name in dirnames if name != ".git"]
                for filename in filenames:
                    paths.append(Path(dirpath) / filename)
    seen: set[str] = set()
    for path in paths:
        rel = posix(relative(root, path))
        if rel in seen:
            continue
        seen.add(rel)
        for part in Path(rel).parts:
            if any(needle in part for needle in needles):
                groups[classify_hit(rel) if classify_hit(rel) != "unwanted" else "unwanted"].append(
                    f"path {rel}"
                )
                break
        text = read_text(path)
        if text is None:
            continue
        for line_number, line in enumerate(text.splitlines(), start=1):
            if not any(needle in line for needle in needles):
                continue
            stripped = line
            historical_link = False
            for phrase in sorted(PROTECTED_PHRASES, key=len, reverse=True):
                if phrase not in stripped:
                    continue
                if phrase == PROTECTED_URL or phrase.endswith(PROTECTED_URL):
                    if classify_hit(rel) == "unwanted":
                        groups["github-url"].append(f"{rel}:{line_number}")
                else:
                    historical_link = True
                stripped = stripped.replace(phrase, "")
            if any(needle in stripped for needle in needles):
                groups[classify_hit(rel)].append(f"{rel}:{line_number}")
            elif historical_link and classify_hit(rel) == "unwanted":
                groups["historical"].append(f"{rel}:{line_number}")
    for key in groups:
        groups[key] = sorted(set(groups[key]))
    return groups


def preview(root: Path) -> tuple[int, dict[str, int], list[tuple[Path, Path]]]:
    totals = {old: 0 for old, _new in MAPPINGS}
    files = 0
    for path in iter_files(root):
        rel = posix(relative(root, path))
        if rel in EXCLUDED_CONTENT:
            continue
        text = read_text(path)
        if text is None:
            continue
        updated, counts = map_text(text)
        if updated != text:
            files += 1
        for key, value in counts.items():
            totals[key] += value
    return files, totals, planned_renames(root)


def print_counts(files: int, totals: dict[str, int], moves: list[tuple[Path, Path]]) -> None:
    replaced = sum(totals.values())
    print(f"files with text changes: {files}")
    print(f"name replacements: {replaced}")
    for old, _new in MAPPINGS:
        print(f"  {old} -> {_new}: {totals[old]}")
    print(f"path renames: {len(moves)}")
    for source, dest in moves:
        print(f"  {posix(source)} -> {posix(dest)}")


def print_scan(groups: dict[str, list[str]]) -> None:
    labels = {
        "unwanted": "Unwanted remaining names",
        "historical": "Historical references kept on purpose",
        "github-url": "Live GitHub links kept until the repository rename",
        "third-party": "Third-party or vendored sources left unchanged",
        "tooling": "Rename tool mapping, kept so the script stays idempotent",
    }
    for key, title in labels.items():
        rows = groups.get(key, [])
        print(f"\n{title}: {len(rows)}")
        for row in rows[:80]:
            print(f"  {row}")
        if len(rows) > 80:
            print(f"  ... {len(rows) - 80} more")


def main() -> int:
    parser = argparse.ArgumentParser(description="Rename Ferrioxid and Ferrum to Fexu.")
    parser.add_argument("--apply", action="store_true", help="Write the rename. Default is a dry-run.")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    root = args.root.resolve()
    inside = FIXED_LITERALS[0][1]
    literal = inside.removeprefix('b"').removesuffix('"')
    if len(literal.encode("utf-8")) != 32:
        raise SystemExit("The replacement development key is not 32 bytes.")
    if args.apply:
        assert_clean_enough(root)
        files, totals, _skipped = apply_content(root)
        tracked = tracked_paths(root)
        moves = planned_renames(root)
        renamed = apply_renames(root, moves, tracked)
        print("Applied.")
        print_counts(files, totals, moves)
        print(f"completed path operations: {renamed}")
    else:
        files, totals, moves = preview(root)
        print("Dry-run. Pass --apply to write changes.")
        print_counts(files, totals, moves)
    print_scan(scan(root))
    return 0


if __name__ == "__main__":
    sys.exit(main())
