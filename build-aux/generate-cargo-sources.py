#!/usr/bin/env python3
"""Regenerate Flatpak's offline crates.io sources from Cargo.lock."""

import json
import tomllib
from pathlib import Path


root = Path(__file__).resolve().parent.parent
with (root / "Cargo.lock").open("rb") as lock_file:
    packages = tomllib.load(lock_file)["package"]

sources = []
for package in packages:
    source = package.get("source")
    if source is None:
        continue
    if source != "registry+https://github.com/rust-lang/crates.io-index":
        raise ValueError(f"Unsupported Cargo source: {source}")

    name = package["name"]
    version = package["version"]
    checksum = package["checksum"]
    destination = f"cargo/vendor/{name}-{version}"
    sources.extend(
        [
            {
                "type": "archive",
                "archive-type": "tar-gzip",
                "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
                "sha256": checksum,
                "dest": destination,
            },
            {
                "type": "inline",
                "contents": json.dumps({"package": checksum, "files": {}}),
                "dest": destination,
                "dest-filename": ".cargo-checksum.json",
            },
        ]
    )

sources.append(
    {
        "type": "inline",
        "contents": '[source.vendored-sources]\ndirectory = "cargo/vendor"\n\n'
        '[source.crates-io]\nreplace-with = "vendored-sources"\n',
        "dest": "cargo",
        "dest-filename": "config.toml",
    }
)
(root / "build-aux/cargo-sources.json").write_text(
    json.dumps(sources, indent=4) + "\n", encoding="utf-8"
)
