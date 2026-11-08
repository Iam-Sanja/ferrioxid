#!/usr/bin/env python3
"""Offline Argos German-to-English Markdown translation; never send source to an API."""
from pathlib import Path
import re
import argostranslate.translate
from langdetect import DetectorFactory, LangDetectException, detect
DetectorFactory.seed = 0
ROOT = Path(__file__).resolve().parents[2]
FILES = sorted((ROOT / "docs").glob("*.md")) + [ROOT / "README.md"]
SKIP = {"ferrioxid-rename-plan.md", "translation-review.md"}
MARKERS = re.compile(r"\b(?:und|oder|nicht|eine|einen|einem|einer|der|die|das|den|dem|ist|sind|wird|werden|kann|können|muss|müssen|für|über|ohne|mit|auch|nur|noch|beim|bereits|dabei|beispiel|verzeichnis|datei|befehle|anwendung|einstellungen|benutzer|projekt|migrationen|verwaltung|dokumentation|voraussetzungen|ausführen|wichtig|hinweis|erstellen)\b", re.I)
PROTECT = re.compile(r"(\x60+[^\x60\n]+\x60+|https?://[^\s<>)]+|\]\([^\)\n]+\)|<(?:/?[A-Za-z][^>\n]*|https?://[^>\n]+)>|\{\{[^}\n]*\}\}|\{%[^%\n]*%\}|\$\{[^}\n]+\}|(?<!\w)(?:FERRUM|FERRIOXID)_[A-Z_0-9]+)")
FENCE = re.compile(r"^\s*(\x60{3,}|~{3,})")
PREFIX = re.compile(r"^(\s*(?:#{1,6}\s+|[-*+]\s+|(?:\d+[.)])\s+|>\s+))")
errors = []
translated = 0
def is_german(text):
    plain = PROTECT.sub(" ", text)
    plain = re.sub(r"[\[\](){}/*#_=:|\\-]", " ", plain)
    plain = " ".join(plain.split())
    if len(plain) < 4:
        return False
    hits = MARKERS.findall(plain)
    if len(hits) >= 2 or (len(hits) == 1 and len(plain) < 65):
        return True
    if len(plain) < 20:
        return bool(re.search("[äöüßÄÖÜ]", plain))
    try:
        return detect(plain) == "de"
    except LangDetectException:
        return False
def fragment(value):
    global translated
    if not is_german(value):
        return value
    held = []
    def save(match):
        n = len(held)
        held.append(match.group())
        return f"ZXQITEM{n}ZXQ"
    safe = PROTECT.sub(save, value)
    output = argostranslate.translate.translate(safe, "de", "en")
    for n, item in enumerate(held):
        marker = f"ZXQITEM{n}ZXQ"
        if marker not in output:
            errors.append(f"Protected token missing: {value[:100]}")
            return value
        output = output.replace(marker, item)
    if not output.strip():
        errors.append(f"Empty output: {value[:100]}")
        return value
    translated += 1
    return output
def line_translate(line):
    if not line.strip() or line.lstrip().startswith(("<!--", "-->")):
        return line
    if re.match(r"^\s*\|[\s:|-]+\|\s*$", line):
        return line
    m = PREFIX.match(line)
    first = m.group(0) if m else ""
    body = line[len(first):]
    end = "\n" if body.endswith("\n") else ""
    body = body.rstrip("\n")
    if body.lstrip().startswith("|") and "|" in body:
        body = "|".join(fragment(x) if x.strip() else x for x in body.split("|"))
    else:
        body = fragment(body)
    return first + body + end
def process(path):
    if path.name in SKIP:
        return
    source = path.read_text(encoding="utf-8")
    output = []
    fence = None
    for line in source.splitlines(keepends=True):
        m = FENCE.match(line)
        if m:
            kind = m.group(1)[0]
            if fence is None:
                fence = kind
            elif fence == kind:
                fence = None
            output.append(line)
        elif fence is not None:
            output.append(line)
        else:
            output.append(line_translate(line))
    result = "".join(output)
    if result.count(chr(96)*3) != source.count(chr(96)*3):
        raise ValueError(f"Code fence count changed: {path}")
    if result != source:
        path.write_text(result, encoding="utf-8")
def main():
    for path in FILES:
        process(path)
    print(f"Translated lines/table cells: {translated}; protected-marker issues: {len(errors)}")
    for issue in errors[:100]:
        print(issue)
    if errors:
        raise RuntimeError("Unrestored protected tokens need manual review")
if __name__ == "__main__":
    main()
