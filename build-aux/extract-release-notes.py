#!/usr/bin/env python3
"""Extract release notes for a given version from the metainfo XML as Markdown."""

import sys
import xml.etree.ElementTree as ET

version = sys.argv[1]
root = ET.parse("data/io.github.astrovm.AdventureMods.metainfo.xml.in").getroot()

for release in root.findall(".//release"):
    if release.get("version") != version:
        continue
    lines = []
    for child in release.find("description"):
        if child.tag == "p":
            lines.append(child.text.strip())
        elif child.tag == "ul":
            for li in child.findall("li"):
                lines.append(f"- {li.text.strip()}")
    print("\n".join(lines))
    break
