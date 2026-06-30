#!/usr/bin/env python3
"""
Build Chinese EPUB for rust-patterns-book.
- Pre-renders Mermaid diagrams to SVG via mermaid.ink
- Applies code formatting CSS
- Generates EPUB via mdbook
"""

import base64
import os
import re
import shutil
import subprocess
import sys
import urllib.request
import urllib.error
from pathlib import Path

BOOK_DIR = Path(__file__).parent
SRC_ZH = BOOK_DIR / "src-zh"
BUILD_DIR = BOOK_DIR / ".epub-zh-build"
OUTPUT_DIR = BOOK_DIR / "book-zh-epub"

MERMAID_RE = re.compile(r"```mermaid\n(.*?)```", re.DOTALL)

def log(msg):
    sys.stdout.buffer.write((msg + "\n").encode("utf-8", errors="replace"))
    sys.stdout.buffer.flush()


import time

def render_mermaid_to_svg(mermaid_code, idx):
    """Render mermaid diagram to SVG via mermaid.ink API (with retries)."""
    encoded = base64.urlsafe_b64encode(mermaid_code.encode("utf-8")).decode("ascii")
    url = f"https://mermaid.ink/svg/{encoded}"
    svg_path = BUILD_DIR / "src" / "images" / f"mermaid-{idx:02d}.svg"
    svg_path.parent.mkdir(parents=True, exist_ok=True)

    for attempt in range(3):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
            with urllib.request.urlopen(req, timeout=30) as resp:
                data = resp.read()
                if len(data) > 100:  # sanity check
                    svg_path.write_bytes(data)
                    log(f"  [OK] mermaid-{idx:02d}.svg")
                    return f"images/mermaid-{idx:02d}.svg"
        except Exception as e:
            if attempt < 2:
                time.sleep(2)
                continue
            log(f"  [FAIL] mermaid-{idx:02d}: {e}")
    return None


def process_markdown(content, base_counter):
    """Replace mermaid code blocks with rendered SVG images."""
    counter = [base_counter]

    def replacer(match):
        counter[0] += 1
        mermaid_code = match.group(1).strip()
        svg_rel = render_mermaid_to_svg(mermaid_code, counter[0])
        if svg_rel:
            return f'![Mermaid-{counter[0]}]({svg_rel})'
        return f"```mermaid\n{mermaid_code}\n```"

    return MERMAID_RE.sub(replacer, content), counter[0]


def main():
    log("=" * 60)
    log("Building rust-patterns-book Chinese EPUB")
    log("=" * 60)

    # Clean and create build directory
    if BUILD_DIR.exists():
        shutil.rmtree(BUILD_DIR)
    BUILD_DIR.mkdir(parents=True)
    (BUILD_DIR / "src").mkdir()
    (BUILD_DIR / "src" / "images").mkdir()

    # Copy markdown files and process mermaid
    log("\n[1/4] Processing markdown files and rendering Mermaid diagrams...")
    mermaid_counter = 0
    for md_file in sorted(SRC_ZH.glob("*.md")):
        content = md_file.read_text(encoding="utf-8")
        if "```mermaid" in content:
            content, mermaid_counter = process_markdown(content, mermaid_counter)
        dest = BUILD_DIR / "src" / md_file.name
        dest.write_text(content, encoding="utf-8")
        log(f"  [OK] {md_file.name}")

    # Write code formatting CSS
    log("\n[2/4] Writing CSS for code block optimization...")
    css_content = """\
/* EPUB code block readability tweaks */
pre {
  background: #f6f8fa;
  border: 1px solid #d0d7de;
  border-radius: 6px;
  padding: 0.85em;
  margin: 1em 0;
  line-height: 1.5;
  font-size: 0.85em;
  overflow-wrap: break-word;
  word-wrap: break-word;
  white-space: pre-wrap;
}

code {
  font-family: Consolas, "Cascadia Mono", "Courier New", monospace;
  font-size: 0.92em;
}

pre code {
  display: block;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
  line-height: 1.5;
}

p code, li code, td code {
  background: #f6f8fa;
  border: 1px solid #d0d7de;
  border-radius: 3px;
  padding: 0.1em 0.25em;
  font-size: 0.88em;
}

img {
  max-width: 100%;
  height: auto;
  display: block;
  margin: 1em auto;
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9em;
  margin: 1em 0;
}

th, td {
  border: 1px solid #d0d7de;
  padding: 0.4em 0.5em;
  vertical-align: top;
}

th {
  background: #f0f3f6;
  font-weight: bold;
}

blockquote {
  border-left: 4px solid #d0d7de;
  margin: 1em 0;
  padding: 0.5em 1em;
  background: #f9fafb;
  color: #555;
}

body, p, li {
  line-height: 1.8;
}

h1, h2, h3, h4 {
  margin-top: 1.5em;
  margin-bottom: 0.6em;
  line-height: 1.3;
}

h1 { font-size: 1.8em; border-bottom: 2px solid #e1e4e8; padding-bottom: 0.3em; }
h2 { font-size: 1.5em; border-bottom: 1px solid #e1e4e8; padding-bottom: 0.3em; }
h3 { font-size: 1.25em; }
h4 { font-size: 1.1em; }
"""
    css_path = BUILD_DIR / "epub-code.css"
    css_path.write_text(css_content, encoding="utf-8")

    # Write book.toml for EPUB
    log("\n[3/4] Writing book.toml for EPUB build...")
    book_toml = """\
[book]
title = "Rust \u6a21\u5f0f\u4e0e\u5de5\u7a0b\u5b9e\u8df5"
authors = ["Rust Training Team"]
language = "zh-CN"
src = "src"

[build]
build-dir = "book-zh-epub"

[output.epub]
additional-css = ["epub-code.css"]

[output.html]
default-theme = "light"
additional-css = ["epub-code.css"]
"""
    (BUILD_DIR / "book.toml").write_text(book_toml, encoding="utf-8")

    # Build EPUB
    log("\n[4/4] Building EPUB with mdbook...")
    result = subprocess.run(
        ["mdbook", "build"],
        cwd=str(BUILD_DIR),
        capture_output=True,
    )
    try:
        stdout = result.stdout.decode("utf-8", errors="replace")
        stderr = result.stderr.decode("utf-8", errors="replace")
    except Exception:
        stdout = str(result.stdout)
        stderr = str(result.stderr)
    if stdout:
        log(stdout)
    if stderr:
        log("STDERR: " + stderr)

    # Copy EPUB to output directory
    epub_src_dir = BUILD_DIR / "book-zh-epub" / "epub"

    if epub_src_dir.exists():
        OUTPUT_DIR.mkdir(exist_ok=True)
        for epub_file in epub_src_dir.glob("*.epub"):
            dest = OUTPUT_DIR / epub_file.name
            shutil.copy2(epub_file, dest)
            size_kb = dest.stat().st_size / 1024
            log(f"\n[OK] EPUB generated: {dest} ({size_kb:.0f} KB)")
        # Cleanup on success
        shutil.rmtree(BUILD_DIR, ignore_errors=True)
        log("\nDone!")
    else:
        log("\n[FAIL] EPUB generation failed - build dir kept for debugging: " + str(BUILD_DIR))
        sys.exit(1)


if __name__ == "__main__":
    main()
