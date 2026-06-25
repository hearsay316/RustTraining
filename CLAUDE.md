# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

This repository is an mdBook-based Rust training curriculum. The root Cargo workspace contains only the `xtask` helper crate; most content is prose and examples under individual book directories.

The published site is a unified landing page plus seven mdBook outputs:

- `c-cpp-book/` — Rust for C/C++ Programmers
- `csharp-book/` — Rust for C# Programmers
- `python-book/` — Rust for Python Programmers
- `async-book/` — Async Rust, with both English `src/` and Chinese `src-zh/`
- `rust-patterns-book/` — Rust Patterns
- `type-driven-correctness-book/` — Type-Driven Correctness
- `engineering-book/` — Rust Engineering Practices

Each book has its own `book.toml`, `src/SUMMARY.md`, chapter markdown files, and Mermaid assets. The root README is bilingual English/Chinese and includes the public reading links and maintainer commands.

## Common commands

Install local documentation tooling:

```bash
cargo install mdbook@0.4.52 mdbook-mermaid@0.14.0
```

Build and preview through the workspace xtask alias from `.cargo/config.toml`:

```bash
cargo xtask build     # build all books into site/
cargo xtask serve     # build all books and serve site/ at http://localhost:3000
cargo xtask deploy    # build all books into docs/ for GitHub Pages artifacts
cargo xtask clean     # remove site/ and docs/
```

Build or serve one book directly:

```bash
cd async-book && mdbook build
cd async-book && mdbook serve --open
```

For the Chinese Async Rust book, use `book-zh.toml` as the active config when building directly, or use `cargo xtask build`/`serve` which handles it automatically.

Rust checks for the xtask crate:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --package xtask <test_name>
```

## Architecture notes

- `xtask/src/main.rs` is the site orchestrator. It defines the canonical `BOOKS` list, builds every book, writes the unified landing page, serves `site/` locally, and cleans generated output.
- `cargo xtask build` writes local preview output to `site/`; `cargo xtask deploy` writes GitHub Pages output to `docs/`.
- The async book is special-cased in xtask: it builds the English book normally, then creates a temporary `.mdbook-zh-work/` directory, copies `src-zh/`, `book-zh.toml`, and shared language-switcher/Mermaid assets, and builds Chinese output under `async-book/zh/` in the destination.
- The local server in xtask is a small static file server with path decoding, null-byte rejection, traversal blocking, and canonical-path prefix checks. Preserve those checks when modifying request handling.
- `.github/workflows/pages.yml` installs Rust, mdBook, and mdbook-mermaid, runs `cargo xtask deploy`, uploads `docs/`, and deploys to GitHub Pages on pushes to `main` or manual dispatch.

## Content conventions

- Keep README-visible material bilingual where the surrounding section is bilingual.
- Book content is standard mdBook markdown. Update the relevant `src/SUMMARY.md` when adding, removing, or renaming chapters.
- Mermaid support is configured per book through `mdbook-mermaid` and the local Mermaid JS/init files.
- The Async Rust book has language-switcher assets in addition to Mermaid assets; keep English and Chinese structure aligned when changing shared navigation or chapter organization.
