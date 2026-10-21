#!/usr/bin/env python3
"""Notes d'une version (liste Markdown de CHANGELOG.md) en HTML simple, pour
le lien par langue du flux Sparkle (scripts/release.sh). Puces « - »,
**gras**, `code` ; le reste est échappé.

Usage : scripts/notes-html.py < notes.md > notes.html
"""

import html
import re
import sys


def inline(text: str) -> str:
    out = html.escape(text, quote=False)
    out = re.sub(r"`([^`]+)`", r"<code>\1</code>", out)
    return re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", out)


def main() -> None:
    items, para = [], []
    for line in sys.stdin.read().splitlines():
        if line.startswith("- "):
            items.append(line[2:])
        elif line.strip() and items:
            items[-1] += " " + line.strip()
        elif line.strip():
            para.append(line.strip())
    parts = ['<meta charset="utf-8">']
    parts += [f"<p>{inline(p)}</p>" for p in para]
    if items:
        parts.append("<ul>" + "".join(f"<li>{inline(i)}</li>" for i in items) + "</ul>")
    print("\n".join(parts))


if __name__ == "__main__":
    main()
