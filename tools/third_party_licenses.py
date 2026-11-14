#!/usr/bin/env python3
"""Writes THIRD_PARTY_LICENSES.txt: every library (and font) built into PrintZilla,
with its license and the license texts shipped in its package.

Run from the app folder after changing dependencies:
    python3 tools/third_party_licenses.py
"""

import json
import os
import re
import subprocess
import sys

TARGETS = ["aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-pc-windows-msvc"]
LICENSE_FILE = re.compile(r"^(licen[cs]e|copying|notice|ofl|ufl|copyright)([-_.].*)?$|.*[-_]licen[cs]e(\..*)?$", re.I)
OUT = "THIRD_PARTY_LICENSES.txt"


def metadata(target):
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--filter-platform", target],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return json.loads(out)


def shipped_packages():
    """Packages that end up in the program (normal dependencies, not build or test tools)."""
    found = {}
    for t in TARGETS:
        m = metadata(t)
        pkgs = {p["id"]: p for p in m["packages"]}
        nodes = {n["id"]: n for n in m["resolve"]["nodes"]}
        root = m["resolve"]["root"]
        seen, stack = set(), [root]
        while stack:
            i = stack.pop()
            if i in seen:
                continue
            seen.add(i)
            for d in nodes[i]["deps"]:
                if any(k["kind"] is None for k in d["dep_kinds"]):
                    stack.append(d["pkg"])
        for i in seen:
            if i != root:
                p = pkgs[i]
                found[(p["name"], p["version"])] = p
    return [found[k] for k in sorted(found)]


def license_files(pkg):
    base = os.path.dirname(pkg["manifest_path"])
    files = []
    for dirpath, dirnames, filenames in os.walk(base):
        depth = os.path.relpath(dirpath, base).count(os.sep)
        dirnames[:] = [d for d in dirnames if d not in ("target", "tests", "benches", "examples") and depth < 2]
        for f in filenames:
            if LICENSE_FILE.match(f) and not f.endswith((".rs", ".toml", ".json")):
                files.append(os.path.join(dirpath, f))
    if pkg.get("license_file"):
        lf = os.path.join(base, pkg["license_file"])
        if os.path.exists(lf) and lf not in files:
            files.append(lf)
    return sorted(files)


def main():
    pkgs = shipped_packages()
    texts = {}  # text -> list of "name version (file)"
    lines = [
        "PrintZilla – Lizenzen von Drittanbietern / Third-party licenses",
        "=" * 64,
        "",
        "PrintZilla enthält die folgenden freien Bibliotheken und Schriften.",
        "Ihre Lizenzbedingungen und Urheberrechtsvermerke stehen weiter unten.",
        "PrintZilla includes the following open-source libraries and fonts.",
        "",
    ]
    for p in pkgs:
        lic = p.get("license") or "siehe Lizenztext"
        repo = p.get("repository") or p.get("homepage") or ""
        lines.append(f"- {p['name']} {p['version']} — {lic}" + (f" — {repo}" if repo else ""))
        for f in license_files(p):
            try:
                with open(f, encoding="utf-8", errors="replace") as fh:
                    t = fh.read().strip()
            except OSError:
                continue
            if t:
                texts.setdefault(t, []).append(f"{p['name']} {p['version']} ({os.path.basename(f)})")
    lines += ["", ""]
    for t, users in sorted(texts.items(), key=lambda kv: kv[1][0].lower()):
        lines.append("-" * 64)
        lines.append("Gilt für / Applies to:")
        lines += [f"  {u}" for u in users]
        lines.append("-" * 64)
        lines.append(t)
        lines.append("")
    with open(OUT, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")
    print(f"{OUT}: {len(pkgs)} Pakete, {len(texts)} verschiedene Lizenztexte", file=sys.stderr)


if __name__ == "__main__":
    main()
