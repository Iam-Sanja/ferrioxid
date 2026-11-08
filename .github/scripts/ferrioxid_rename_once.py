#!/usr/bin/env python3
"""One-time Ferrum -> Ferrioxid source migration (run on a dedicated topic branch).

Keep database schemas, historical migrations, old config filenames, environment
variables and generated-project tracking markers intact for compatibility.
"""
from pathlib import Path
import json
import re

ROOT = Path(__file__).resolve().parents[2]
OLD = "ferrum"
NEW = "ferrioxid"
KEEP = [
    re.compile(r"ferrum\.toml"),
    re.compile(r"\.ferrum(?=[/\\\"' ]|$)"),
    re.compile(r"_ferrum_"),
    re.compile(r"FERRUM_[A-Z0-9_]+"),
    re.compile(r"ferrum:models(?::end)?"),
]
EXTENSIONS = {".rs", ".toml", ".lock", ".md", ".json", ".yml", ".yaml",
              ".html", ".css", ".js", ".txt", ".example", ".gitignore",
              ".dockerignore"}
SKIP = {"docs/ferrioxid-rename-plan.md", ".github/scripts/ferrioxid_rename_once.py",
        ".github/workflows/rename-migration-once.yml"}
DIRECTORIES = [
    "ferrum", "ferrum-core", "ferrum-macros", "ferrum-db",
    "ferrum-api", "ferrum-cli", "ferrum-migrations", "ferrum-auth",
    "ferrum-admin", "ferrum-forms", "ferrum-static", "ferrum-storage",
    "ferrum-middleware", "ferrum-mail", "ferrum-jobs",
]


def rewrite(source: str) -> str:
    preserved = []
    combined = re.compile("|".join(f"(?:{x.pattern})" for x in KEEP))
    def save(match):
        index = len(preserved)
        preserved.append(match.group())
        return f"___LEGACY_IDENTIFIER_{index}___"
    source = combined.sub(save, source)
    source = source.replace("Ferrum", "Ferrioxid").replace("ferrum", "ferrioxid")
    source = source.replace("NERAVOL", "FERRIOXID").replace("neravol", "ferrioxid")
    for index, value in enumerate(preserved):
        source = source.replace(f"___LEGACY_IDENTIFIER_{index}___", value)
    return source


def main() -> None:
    crates = ROOT / "crates"
    moved = []
    for old in DIRECTORIES:
        source = crates / old
        dest = crates / old.replace(OLD, NEW)
        if not source.is_dir() or dest.exists():
            raise RuntimeError(f"unexpected crate directory state: {source} -> {dest}")
        source.rename(dest)
        moved.append((str(source.relative_to(ROOT)), str(dest.relative_to(ROOT))))

    updated = []
    for file in ROOT.rglob("*"):
        if not file.is_file():
            continue
        relative = file.relative_to(ROOT).as_posix()
        if relative.startswith(".git/") or relative.startswith("target/"):
            continue
        if relative in SKIP or "/migrations/" in relative and file.suffix == ".json":
            continue
        if file.suffix not in EXTENSIONS and file.name not in {
            ".gitignore", ".dockerignore", "Cargo.lock", "Cargo.toml", "README.md"
        }:
            continue
        try:
            original = file.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        result = rewrite(original)
        if result != original:
            file.write_text(result, encoding="utf-8")
            updated.append(relative)

    # The workspace identity changes, not the persisted schema or migration IDs.
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    assert "crates/ferrioxid-cli" in cargo
    assert "version = \"0.1.0\"" in cargo
    cli = (ROOT / "crates/ferrioxid-cli/Cargo.toml").read_text(encoding="utf-8")
    assert 'name = "ferrioxid-cli"' in cli
    assert 'name = "ferrioxid"' in cli
    macros = (ROOT / "crates/ferrioxid-macros/src/lib.rs").read_text(encoding="utf-8")
    assert 'proc_macro_derive(FerrioxidModel, attributes(ferrioxid, field, admin))' in macros

    report = {"renamed_directories": moved, "updated_files": updated}
    print(json.dumps({"renamed_crates": len(moved),
                      "updated_text_files": len(updated),
                      "examples": updated[:20]}, indent=2))
    (ROOT / "target").mkdir(exist_ok=True)
    (ROOT / "target/ferrioxid-rename-report.json").write_text(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
