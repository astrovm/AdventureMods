#!/usr/bin/env python3
"""Check that the Flatpak Cargo sources match Cargo.lock."""

import json
import sys
import tomllib
from pathlib import Path


def fail(messages: list[str]) -> None:
    for message in messages:
        print(message, file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    root = Path(__file__).resolve().parent.parent
    lock_path = root / "Cargo.lock"
    sources_path = root / "build-aux" / "cargo-sources.json"

    with lock_path.open("rb") as lock_file:
        packages = tomllib.load(lock_file)["package"]
    sources = json.loads(sources_path.read_text(encoding="utf-8"))

    expected = {
        f"cargo/vendor/{package['name']}-{package['version']}": package["checksum"]
        for package in packages
        if str(package.get("source", "")).startswith("registry+")
    }
    archives = {
        source["dest"]: source["sha256"]
        for source in sources
        if source.get("type") == "archive"
    }

    problems = []
    for destination, checksum in expected.items():
        actual = archives.get(destination)
        if actual is None:
            problems.append(f"Missing Cargo source: {destination}")
        elif actual != checksum:
            problems.append(f"Wrong checksum for Cargo source: {destination}")

    for destination in sorted(archives.keys() - expected.keys()):
        problems.append(f"Stale Cargo source: {destination}")

    config_sources = [
        source
        for source in sources
        if source.get("dest") == "cargo"
        and source.get("dest-filename") == "config.toml"
    ]
    if len(
        config_sources
    ) != 1 or 'replace-with = "vendored-sources"' not in config_sources[0].get(
        "contents", ""
    ):
        problems.append("Cargo sources must contain one vendored-sources config")

    if problems:
        fail(problems)

    print(f"Validated {len(expected)} offline Cargo sources")


if __name__ == "__main__":
    main()
