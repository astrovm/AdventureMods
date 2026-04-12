#!/usr/bin/env python3
"""Extract release notes for a given version from the metainfo XML as Markdown."""

import sys
import xml.etree.ElementTree as ET

if len(sys.argv) < 2:
    sys.stderr.write("usage: extract-release-notes.py <version>\n")
    sys.exit(1)

version = sys.argv[1]
root = ET.parse("data/io.github.astrovm.AdventureMods.metainfo.xml.in").getroot()

for release in root.findall(".//release"):
    if release.get("version") != version:
        continue
    lines = []
    description = release.find("description")
    if description is not None:
        for child in description:
            if child.tag == "p":
                lines.append((child.text or "").strip())
            elif child.tag == "ul":
                for li in child.findall("li"):
                    lines.append(f"- {(li.text or '').strip()}")
    print("\n".join(lines))
    sys.exit(0)

sys.stderr.write(f"error: no release entry found for version {version}\n")
sys.exit(1)
