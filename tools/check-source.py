#!/usr/bin/env python3
"""Reject common private/generated files and credentials in the Git source tree.

This is a publication guard, not a copyright or authorship determination.
Use --staged to inspect the exact index contents before committing.
"""
from __future__ import annotations

import argparse
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys

ROOT_FILES = {
    ".editorconfig", ".gitattributes", ".gitignore", "AGENTS.md",
    "CONTRIBUTING.md", "Cargo.lock", "Cargo.toml", "LICENSE", "Play.cmd",
    "README.md", "rust-toolchain.toml",
}
SOURCE_ROOTS = {".cargo", ".github", ".vscode", "apps", "crates", "docs", "xtask"}
SOURCE_SUFFIXES = {".rs", ".toml", ".md", ".json", ".yml", ".yaml", ".wgsl", ".html"}
TOOL_FILES = {
    "tools/check-source.py", "tools/dashboard/package.json",
    "tools/dashboard/bun.lock", "tools/dashboard/dashboard.test.mjs",
}
PRIVATE_PARTS = {"local", "target", "dist", "node_modules", ".git", "__pycache__"}
SECRETS = re.compile(
    rb"-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----|"
    rb"gh[pousr]_[A-Za-z0-9]{30,255}|github_pat_[A-Za-z0-9_]{40,255}|"
    rb"AKIA[A-Z0-9]{16}"
)
MAX_FILE_BYTES = 1024 * 1024


def allowed(name: str) -> bool:
    path = PurePosixPath(name)
    if path.is_absolute() or any(part in PRIVATE_PARTS or part == ".." for part in path.parts):
        return False
    if path.name == ".env" or path.name.startswith(".env."):
        return False
    if name in ROOT_FILES or name in TOOL_FILES or name == "progress/roadmap.json":
        return True
    if path.parts[0] == "third-party":
        return path.name.startswith(("LICENSE", "COPYING", "NOTICE")) or path.name == "README.md"
    return path.parts[0] in SOURCE_ROOTS and path.suffix in SOURCE_SUFFIXES


def git(*args: str, **kwargs) -> bytes:
    return subprocess.run(["git", *args], check=True, stdout=subprocess.PIPE, **kwargs).stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--staged", action="store_true", help="read blobs from the index, not working files")
    args = parser.parse_args()
    root = Path(git("rev-parse", "--show-toplevel").decode().strip())
    records = []
    for row in git("ls-files", "--stage", "-z").split(b"\0"):
        if not row:
            continue
        info, name = row.split(b"\t", 1)
        mode, oid, stage = info.split()
        records.append((mode, oid, stage, name.decode("utf-8")))
    if not records:
        print("No tracked/staged source files to inspect.", file=sys.stderr)
        return 1
    blobs = b""
    if args.staged:
        blobs = git("cat-file", "--batch", input=b"\n".join(row[1] for row in records) + b"\n")
    cursor = total = 0
    errors = []
    for mode, _oid, stage, name in records:
        if args.staged:
            end = blobs.index(b"\n", cursor)
            header = blobs[cursor:end].split()
            if len(header) != 3:
                raise RuntimeError(f"{name}: invalid Git object header")
            size = int(header[2])
            data = blobs[end + 1:end + 1 + size]
            cursor = end + size + 2
            if header[1] != b"blob":
                errors.append(f"{name}: expected a regular source blob")
                continue
        else:
            data = (root / name).read_bytes()
        total += len(data)
        if mode not in (b"100644", b"100755") or stage != b"0":
            errors.append(f"{name}: symlinks, submodules and unresolved entries are not allowed")
        if not allowed(name):
            errors.append(f"{name}: outside the public source allowlist")
        if len(data) > MAX_FILE_BYTES:
            errors.append(f"{name}: exceeds the source-file size limit")
        try:
            data.decode("utf-8")
        except UnicodeDecodeError:
            errors.append(f"{name}: binary/non-UTF-8 content")
        if b"\0" in data or data.startswith((b"MZ", b"\x7fELF", b"PK\x03\x04", b"RVZ\x01")):
            errors.append(f"{name}: binary payload signature")
        if SECRETS.search(data):
            errors.append(f"{name}: possible credential; inspect locally (value withheld)")
    if errors:
        print("Public source check failed:\n" + "\n".join(errors), file=sys.stderr)
        return 1
    print(f"Public source check passed: {len(records)} text files, {total:,} bytes; no blocked paths or credential patterns.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
