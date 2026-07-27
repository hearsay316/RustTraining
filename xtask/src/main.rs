use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

/// (slug, title, description, category)
const BOOKS: &[(&str, &str, &str, &str)] = &[
    (
        "c-cpp-book",
        "Rust for C/C++ Programmers",
        "Move semantics, RAII, FFI, embedded, no_std",
        "bridge",
    ),
    (
        "csharp-book",
        "Rust for C# Programmers",
        "Best for Swift / C# / Java developers",
        "bridge",
    ),
    (
        "python-book",
        "Rust for Python Programmers",
        "Dynamic → static typing, GIL-free concurrency",
        "bridge",
    ),
    (
        "async-book",
        "Async Rust: From Futures to Production",
        "Tokio, streams, cancellation safety",
        "deep-dive",
    ),
    (
        "rust-patterns-book",
        "Rust Patterns",
        "Pin, allocators, lock-free structures, unsafe",
        "advanced",
    ),
    (
        "type-driven-correctness-book",
        "Type-Driven Correctness",
        "Type-state, phantom types, capability tokens",
        "expert",
    ),
    (
        "engineering-book",
        "Rust Engineering Practices",
        "Build scripts, cross-compilation, coverage, CI/CD",
        "practices",
    ),
];

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must live in a workspace subdirectory")
        .to_path_buf()
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("build") => cmd_build(),
        Some("serve") => {
            cmd_build();
            cmd_serve();
        }
        Some("deploy") => cmd_deploy(),
        Some("epub") => cmd_epub(&args[1..]),
        Some("clean") => cmd_clean(),
        Some("--help" | "-h" | "help") | None => print_usage(0),
        Some(other) => {
            eprintln!("Unknown command: {other}\n");
            print_usage(1);
        }
    }
}

fn print_usage(code: i32) {
    let stream: &mut dyn Write = if code == 0 {
        &mut std::io::stdout()
    } else {
        &mut std::io::stderr()
    };
    let _ = writeln!(
        stream,
        "\
Usage: cargo xtask <COMMAND>

Commands:
  build    Build all books into site/ (for local preview)
  serve    Build and serve at http://localhost:3000
  deploy   Build all books into docs/ (for GitHub Pages)
  epub     Build EPUB files (all or specific book)
  clean    Remove site/ and docs/ directories

epub usage:
  cargo xtask epub            Build EPUB for all books (EN + ZH if available)
  cargo xtask epub <slug>     Build EPUB for a specific book (e.g. async-book)
  cargo xtask epub --zh       Build EPUB only for Chinese versions"
    );
    std::process::exit(code);
}

// ── build ────────────────────────────────────────────────────────────

fn cmd_build() {
    if !check_mdbook() {
        eprintln!("Error: 'mdbook' not found in PATH. Please install it: https://rust-lang.github.io/mdbook/guide/installation.html");
        std::process::exit(1);
    }
    build_to("site");
}

fn cmd_deploy() {
    if !check_mdbook() {
        eprintln!("Error: 'mdbook' not found in PATH.");
        std::process::exit(1);
    }
    build_to("docs");
    println!("\nTo publish, commit docs/ and enable GitHub Pages → \"Deploy from a branch\" → /docs.");
}

fn check_mdbook() -> bool {
    Command::new("mdbook")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn build_to(dir_name: &str) {
    let root = project_root();
    let out = root.join(dir_name);

    if out.exists() {
        fs::remove_dir_all(&out).expect("failed to clean output dir");
    }
    fs::create_dir_all(&out).expect("failed to create output dir");

    println!("Building unified site into {dir_name}/\n");

    let mut ok = 0u32;
    for &(slug, _, _, _) in BOOKS {
        let book_dir = root.join(slug);
        if !book_dir.is_dir() {
            eprintln!("  ✗ {slug}/ not found, skipping");
            continue;
        }
        let dest = out.join(slug);
        let status = if slug == "async-book" {
            build_async_book(&book_dir, &dest)
        } else if slug == "rust-patterns-book" {
            build_rust_patterns_book(&book_dir, &dest)
        } else {
            Command::new("mdbook")
                .args(["build", "--dest-dir"])
                .arg(&dest)
                .current_dir(&book_dir)
                .status()
                .expect("failed to run mdbook — is it installed?")
                .success()
        };

        if status {
            println!("  ✓ {slug}");
            ok += 1;
        } else {
            eprintln!("  ✗ {slug} FAILED");
        }
    }
    println!("\n  {ok}/{} books built", BOOKS.len());

    write_landing_page(&out);

    // Prevent GitHub Pages from processing the output with Jekyll
    fs::write(out.join(".nojekyll"), "").expect("failed to create .nojekyll");
    println!("\nDone! Output in {dir_name}/");
}

fn build_async_book(book_dir: &Path, dest: &Path) -> bool {
    let en_ok = Command::new("mdbook")
        .args(["build", "--dest-dir"])
        .arg(dest)
        .current_dir(book_dir)
        .status()
        .expect("failed to run mdbook — is it installed?")
        .success();

    if !en_ok {
        return false;
    }

    let zh_work = book_dir.join(".mdbook-zh-work");
    if zh_work.exists() {
        fs::remove_dir_all(&zh_work).expect("failed to clean async zh work dir");
    }
    fs::create_dir_all(&zh_work).expect("failed to create async zh work dir");

    copy_dir_all(&book_dir.join("src-zh"), &zh_work.join("src-zh"))
        .expect("failed to copy async zh source");
    for file in [
        "book-zh.toml",
        "mermaid.min.js",
        "mermaid-init.js",
        "language-switcher.css",
        "language-switcher.js",
    ] {
        let target = if file == "book-zh.toml" {
            zh_work.join("book.toml")
        } else {
            zh_work.join(file)
        };
        fs::copy(book_dir.join(file), target).expect("failed to copy async zh asset");
    }

    let zh_ok = Command::new("mdbook")
        .args(["build", "--dest-dir"])
        .arg(dest.join("zh"))
        .current_dir(&zh_work)
        .status()
        .expect("failed to run mdbook — is it installed?")
        .success();

    let _ = fs::remove_dir_all(&zh_work);
    zh_ok
}

fn build_rust_patterns_book(book_dir: &Path, dest: &Path) -> bool {
    let en_ok = Command::new("mdbook")
        .args(["build", "--dest-dir"])
        .arg(dest)
        .current_dir(book_dir)
        .status()
        .expect("failed to run mdbook — is it installed?")
        .success();

    if !en_ok {
        return false;
    }

    let zh_work = book_dir.join(".mdbook-zh-work");
    if zh_work.exists() {
        fs::remove_dir_all(&zh_work).expect("failed to clean rust-patterns zh work dir");
    }
    fs::create_dir_all(&zh_work).expect("failed to create rust-patterns zh work dir");

    copy_dir_all(&book_dir.join("src-zh"), &zh_work.join("src-zh"))
        .expect("failed to copy rust-patterns zh source");
    for file in ["book-zh.toml", "mermaid.min.js", "mermaid-init.js"] {
        let target = if file == "book-zh.toml" {
            zh_work.join("book.toml")
        } else {
            zh_work.join(file)
        };
        fs::copy(book_dir.join(file), target).expect("failed to copy rust-patterns zh asset");
    }

    let zh_ok = Command::new("mdbook")
        .args(["build", "--dest-dir"])
        .arg(dest.join("zh"))
        .current_dir(&zh_work)
        .status()
        .expect("failed to run mdbook — is it installed?")
        .success();

    let _ = fs::remove_dir_all(&zh_work);
    zh_ok
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn category_label(cat: &str) -> &str {
    match cat {
        "bridge" => "Bridge",
        "deep-dive" => "Deep Dive",
        "advanced" => "Advanced",
        "expert" => "Expert",
        "practices" => "Practices",
        _ => cat,
    }
}

fn write_landing_page(site: &Path) {
    let cards: String = BOOKS
        .iter()
        .map(|&(slug, title, desc, cat)| {
            let label = category_label(cat);
            if slug == "async-book" || slug == "rust-patterns-book" {
                format!(
                    r#"    <div class="card cat-{cat}">
      <h2>{title} <span class="label">{label}</span></h2>
      <p>{desc}</p>
      <p class="links"><a href="{slug}/">English</a><a href="{slug}/zh/">中文</a></p>
    </div>"#
                )
            } else {
                format!(
                    r#"    <a class="card cat-{cat}" href="{slug}/">
      <h2>{title} <span class="label">{label}</span></h2>
      <p>{desc}</p>
    </a>"#
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let html = format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Rust Training Books</title>
  <style>
    :root {{
      --bg: #1a1a2e;
      --card-bg: #16213e;
      --accent: #e94560;
      --text: #eee;
      --muted: #a8a8b3;
      --clr-bridge: #4ade80;
      --clr-deep-dive: #22d3ee;
      --clr-advanced: #fbbf24;
      --clr-expert: #c084fc;
      --clr-practices: #2dd4bf;
    }}
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, sans-serif;
      background: var(--bg);
      color: var(--text);
      min-height: 100vh;
      display: flex;
      flex-direction: column;
      align-items: center;
      padding: 3rem 1rem;
    }}
    h1 {{ font-size: 2.5rem; margin-bottom: 0.5rem; }}
    h1 span {{ color: var(--accent); }}
    .subtitle {{ color: var(--muted); font-size: 1.1rem; margin-bottom: 1.2rem; }}

    /* Legend */
    .legend {{
      display: flex; flex-wrap: wrap; gap: 0.6rem 1.4rem;
      justify-content: center; margin-bottom: 2.2rem;
      font-size: 0.8rem; color: var(--muted);
    }}
    .legend-item {{ display: flex; align-items: center; gap: 0.35rem; }}
    .legend-dot {{
      width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0;
    }}

    /* Grid & Cards */
    .grid {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
      gap: 1.5rem;
      max-width: 1000px;
      width: 100%;
    }}
    .card {{
      background: var(--card-bg);
      border-radius: 12px;
      padding: 1.5rem 1.5rem 1.5rem 1.25rem;
      text-decoration: none;
      color: var(--text);
      transition: transform 0.15s, box-shadow 0.15s;
      border: 1px solid rgba(255,255,255,0.05);
      border-left: 4px solid var(--stripe);
    }}
    .card:hover {{
      transform: translateY(-4px);
      box-shadow: 0 8px 25px color-mix(in srgb, var(--stripe) 30%, transparent);
      border-color: rgba(255,255,255,0.08);
      border-left-color: var(--stripe);
    }}
    .card h2 {{ font-size: 1.2rem; margin-bottom: 0.5rem; display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; }}
    .card p  {{ color: var(--muted); font-size: 0.9rem; line-height: 1.4; }}
    .card .links {{ display: flex; gap: 0.75rem; margin-top: 0.9rem; }}
    .card .links a {{ color: var(--accent); font-weight: 600; text-decoration: none; }}
    .card .links a:hover {{ text-decoration: underline; }}

    /* Category colours */
    .cat-bridge     {{ --stripe: var(--clr-bridge); }}
    .cat-deep-dive  {{ --stripe: var(--clr-deep-dive); }}
    .cat-advanced   {{ --stripe: var(--clr-advanced); }}
    .cat-expert     {{ --stripe: var(--clr-expert); }}
    .cat-practices  {{ --stripe: var(--clr-practices); }}

    /* Label pill */
    .label {{
      font-size: 0.55rem; font-weight: 700; letter-spacing: 0.08em;
      text-transform: uppercase; padding: 0.15em 0.55em;
      border-radius: 4px; white-space: nowrap; flex-shrink: 0;
      color: var(--bg); background: var(--stripe);
    }}

    footer {{ margin-top: 3rem; color: var(--muted); font-size: 0.85rem; }}
  </style>
</head>
<body>
  <h1>🦀 <span>Rust</span> Training Books</h1>
  <p class="subtitle">Pick the guide that matches your background</p>

  <div class="legend">
    <span class="legend-item"><span class="legend-dot" style="background:var(--clr-bridge)"></span> Bridge &mdash; learn Rust from another language</span>
    <span class="legend-item"><span class="legend-dot" style="background:var(--clr-deep-dive)"></span> Deep Dive</span>
    <span class="legend-item"><span class="legend-dot" style="background:var(--clr-advanced)"></span> Advanced</span>
    <span class="legend-item"><span class="legend-dot" style="background:var(--clr-expert)"></span> Expert</span>
    <span class="legend-item"><span class="legend-dot" style="background:var(--clr-practices)"></span> Practices</span>
  </div>

  <div class="grid">
{cards}
  </div>
  <footer>Built with <a href="https://rust-lang.github.io/mdBook/" style="color:var(--accent)">mdBook</a></footer>
</body>
</html>
"##
    );

    let path = site.join("index.html");
    fs::write(&path, html).expect("failed to write index.html");
    println!("  ✓ index.html");
}

enum ResolveResult {
    File(PathBuf),
    Redirect(String),
    NotFound,
}

/// Resolve `request_target` (HTTP request path, e.g. `/foo/bar?x=1`) to a file under `site_canon`.
/// Returns `ResolveResult::File` for success, `Redirect` if a trailing slash is needed for a directory,
/// or `NotFound` for traversal attempts or missing files.
///
/// NOTE: This function preserves and hardens the multi-layer security from PR#18:
/// 1. Percent-decoding via `percent_decode_path`.
/// 2. Null byte rejection.
/// 3. Traversal blocking (`..`).
/// 4. Symlink escape prevention via canonicalization and prefix checking.
fn resolve_site_file(site_canon: &Path, request_target: &str) -> ResolveResult {
    let path_only = match request_target
        .split('?')
        .next()
        .and_then(|s| s.split('#').next())
    {
        Some(p) => p,
        None => return ResolveResult::NotFound,
    };

    // [Security] Handle percent-encoding and reject null bytes (from PR#18)
    let decoded = percent_decode_path(path_only);
    if decoded.as_bytes().contains(&0) {
        return ResolveResult::NotFound;
    }

    let rel = decoded.trim_start_matches('/');
    let mut file_path = site_canon.to_path_buf();
    if !rel.is_empty() {
        for seg in rel.split('/').filter(|s| !s.is_empty()) {
            // [Security] Block directory traversal (from PR#18)
            if seg == ".." {
                return ResolveResult::NotFound;
            }
            file_path.push(seg);
        }
    }

    if file_path.is_dir() {
        // If it refers to a directory but lacks a trailing slash, redirect so relative links work.
        if !request_target.ends_with('/') && !request_target.is_empty() {
            return ResolveResult::Redirect(format!("{path_only}/"));
        }
        file_path.push("index.html");
    }

    // [Security] Canonicalize and verify we're still within site_canon (from PR#18)
    let real = match fs::canonicalize(&file_path) {
        Ok(r) => r,
        Err(_) => return ResolveResult::NotFound,
    };

    if !real.starts_with(site_canon) || !real.is_file() {
        return ResolveResult::NotFound;
    }

    ResolveResult::File(real)
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn percent_decode_path(input: &str) -> String {
    let mut decoded = Vec::with_capacity(input.len());
    let b = input.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(hi), Some(lo)) = (hex_val(b[i + 1]), hex_val(b[i + 2])) {
                decoded.push(hi << 4 | lo);
                i += 3;
                continue;
            }
        }
        decoded.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

// ── serve ────────────────────────────────────────────────────────────

fn cmd_serve() {
    let site = project_root().join("site");
    let site_canon = fs::canonicalize(&site).expect(
        "site/ not found — run `cargo xtask build` first (e.g. `cargo xtask serve` runs build automatically)",
    );
    let addr = "127.0.0.1:3000";
    let listener = TcpListener::bind(addr).expect("failed to bind port 3000");

    // Handle Ctrl+C gracefully so cargo doesn't report an error
    ctrlc_exit();

    println!("\nServing at http://localhost:3000  (Ctrl+C to stop)");

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);

        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");

        match resolve_site_file(&site_canon, path) {
            ResolveResult::File(file_path) => {
                let body = fs::read(&file_path).unwrap_or_default();
                let mime = guess_mime(&file_path);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
            }
            ResolveResult::Redirect(new_path) => {
                let header = format!(
                    "HTTP/1.1 301 Moved Permanently\r\nLocation: {new_path}\r\nContent-Length: 0\r\n\r\n"
                );
                let _ = stream.write_all(header.as_bytes());
            }
            ResolveResult::NotFound => {
                let body = b"404 Not Found";
                let header = format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(body);
            }
        }
    }
}

/// Install a Ctrl+C handler that exits cleanly (code 0) instead of
/// letting the OS terminate with STATUS_CONTROL_C_EXIT.
fn ctrlc_exit() {
    ctrlc::set_handler(move || {
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");
}

fn guess_mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

// ── epub ────────────────────────────────────────────────────────────

/// EPUB code formatting CSS (embedded for zero-config)
const EPUB_CSS: &str = r#"/* EPUB code block readability tweaks */
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
h1 { font-size: 1.8em; border-bottom: 2px solid #e1e4e8; padding-bottom: 0.3em; }
h2 { font-size: 1.5em; border-bottom: 1px solid #e1e4e8; padding-bottom: 0.3em; }
h3 { font-size: 1.25em; }
h4 { font-size: 1.1em; }
"#;

/// Books that have Chinese translations (slug → zh title)
const ZH_BOOKS: &[(&str, &str)] = &[
    ("async-book", "Async Rust：从 Future 到生产实践"),
    ("rust-patterns-book", "Rust 模式与工程实践"),
];

/// Check if a book has a Chinese version
fn has_zh(slug: &str) -> bool {
    ZH_BOOKS.iter().any(|(s, _)| *s == slug)
}

fn cmd_epub(args: &[String]) {
    if !check_mdbook() {
        eprintln!("Error: 'mdbook' not found in PATH.");
        std::process::exit(1);
    }
    let root = project_root();
    let epub_out = root.join("epub");

    // Parse args: optional slug or --zh flag
    let only_zh = args.iter().any(|a| a == "--zh");
    let single_slug: Option<&str> = args.iter().find_map(|a| {
        if a.starts_with("--") {
            None
        } else {
            Some(a.as_str())
        }
    });

    if epub_out.exists() {
        let _ = fs::remove_dir_all(&epub_out);
    }
    fs::create_dir_all(&epub_out).expect("failed to create epub/ dir");

    println!("Building EPUB files into epub/\n");

    let mut en_ok = 0u32;
    let mut zh_ok = 0u32;
    let mut en_fail = 0u32;
    let mut zh_fail = 0u32;

    for &(slug, title, _, _) in BOOKS {
        // Filter by single slug if specified
        if let Some(s) = single_slug {
            if s != slug {
                continue;
            }
        }

        let book_dir = root.join(slug);
        if !book_dir.is_dir() {
            eprintln!("  ✗ {slug}/ not found, skipping");
            continue;
        }

        // English EPUB
        if !only_zh {
            match build_single_epub(&book_dir, slug, title, false, &epub_out) {
                Ok(f) => {
                    println!("  ✓ {slug} (EN): {}", f.display());
                    en_ok += 1;
                }
                Err(e) => {
                    eprintln!("  ✗ {slug} (EN) FAILED: {e}");
                    en_fail += 1;
                }
            }
        }

        // Chinese EPUB (if available)
        if has_zh(slug) {
            let zh_title = ZH_BOOKS
                .iter()
                .find(|(s, _)| *s == slug)
                .map(|(_, t)| *t)
                .unwrap_or(title);
            match build_single_epub(&book_dir, slug, zh_title, true, &epub_out) {
                Ok(f) => {
                    println!("  ✓ {slug} (ZH): {}", f.display());
                    zh_ok += 1;
                }
                Err(e) => {
                    eprintln!("  ✗ {slug} (ZH) FAILED: {e}");
                    zh_fail += 1;
                }
            }
        }
    }

    println!("\n  EN: {en_ok} ok, {en_fail} failed");
    if zh_ok + zh_fail > 0 {
        println!("  ZH: {zh_ok} ok, {zh_fail} failed");
    }
    println!("\nDone! EPUB files in epub/");
}

/// Build EPUB for a single book (English or Chinese).
///
/// For Chinese: renders Mermaid diagrams to PNG, replaces code blocks with image references.
/// For English: builds directly from book.toml.
fn build_single_epub(
    book_dir: &Path,
    slug: &str,
    title: &str,
    is_zh: bool,
    output_dir: &Path,
) -> Result<PathBuf, String> {
    // Create temp build dir
    let work = book_dir.join(".epub-work");
    if work.exists() {
        fs::remove_dir_all(&work).map_err(|e| format!("clean work dir: {e}"))?;
    }
    let src_dir = work.join("src");
    let images_dir = src_dir.join("images");
    fs::create_dir_all(&images_dir).map_err(|e| format!("create dirs: {e}"))?;

    // Determine source markdown directory
    let md_src = if is_zh {
        book_dir.join("src-zh")
    } else {
        book_dir.join("src")
    };

    // Copy and process markdown files
    let mut mermaid_counter = 0usize;
    let mermaid_re = regex::Regex::new(r"(?s)```mermaid\n(.*?)```").unwrap();

    let mut md_files: Vec<(String, PathBuf)> = Vec::new();
    for entry in fs::read_dir(&md_src).map_err(|e| format!("read src: {e}"))? {
        let entry = entry.map_err(|e| format!("read entry: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".md") {
            md_files.push((name, entry.path()));
        }
    }
    md_files.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, path) in &md_files {
        let content = fs::read_to_string(path).map_err(|e| format!("read {name}: {e}"))?;
        let processed = if mermaid_re.is_match(&content) {
            process_mermaid_blocks(&content, &mut mermaid_counter, &images_dir, &mermaid_re)
        } else {
            content
        };
        fs::write(src_dir.join(name), processed)
            .map_err(|e| format!("write {name}: {e}"))?;
    }

    // Write CSS
    fs::write(work.join("epub-code.css"), EPUB_CSS)
        .map_err(|e| format!("write css: {e}"))?;

    // Write book.toml
    // NOTE: mdbook-epub uses title as the EPUB filename, so it must not
    // contain characters like `/`, `\`, `:` etc. We sanitize by using slug
    // as a prefix when title contains special chars.
    let safe_title = if title.contains('/') || title.contains('\\') || title.contains(':') {
        slug.to_string()
    } else {
        title.to_string()
    };
    let lang = if is_zh { "zh-CN" } else { "en" };
    let book_toml = format!(
        r#"[book]
title = "{safe_title}"
authors = ["Rust Training Team"]
language = "{lang}"
src = "src"

[build]
build-dir = "book-epub"

[output.epub]
additional-css = ["epub-code.css"]

[output.html]
default-theme = "light"
additional-css = ["epub-code.css"]
"#
    );
    fs::write(work.join("book.toml"), book_toml)
        .map_err(|e| format!("write book.toml: {e}"))?;

    // Run mdbook build — use status() (not output()) because mdbook-epub
    // communicates via STDIN/STDOUT pipe with mdbook.
    let status = Command::new("mdbook")
        .arg("build")
        .current_dir(&work)
        .status()
        .map_err(|e| format!("run mdbook: {e}"))?;

    if !status.success() {
        // Keep work dir for debugging
        return Err(format!("mdbook build failed (work dir: {})", work.display()));
    }

    // Find and copy EPUB file
    let epub_build_dir = work.join("book-epub").join("epub");
    let suffix = if is_zh { "-zh" } else { "" };
    let dest_name = format!("{slug}{suffix}.epub");
    let dest = output_dir.join(&dest_name);

    let mut found_epub = false;
    if epub_build_dir.exists() {
        for entry in fs::read_dir(&epub_build_dir).map_err(|e| format!("read epub dir: {e}"))? {
            let entry = entry.map_err(|e| format!("read epub entry: {e}"))?;
            let src = entry.path();
            if src.extension().and_then(|e| e.to_str()) == Some("epub") {
                let meta = fs::metadata(&src).map_err(|e| format!("stat epub: {e}"))?;
                if meta.len() < 1000 {
                    continue;
                }
                fs::copy(&src, &dest).map_err(|e| format!("copy epub: {e}"))?;
                found_epub = true;
                break;
            }
        }
    }

    // Clean up temp dir
    let _ = fs::remove_dir_all(&work);

    if found_epub {
        Ok(dest)
    } else {
        Err("no valid EPUB file produced".to_string())
    }
}

/// Replace ```mermaid blocks with rendered PNG images
fn process_mermaid_blocks(
    content: &str,
    counter: &mut usize,
    images_dir: &Path,
    re: &regex::Regex,
) -> String {
    re.replace_all(content, |caps: &regex::Captures| {
        *counter += 1;
        let mermaid_code = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        match render_mermaid_to_png(mermaid_code, *counter, images_dir) {
            Some(rel_path) => format!("![流程图 {}]({})", *counter, rel_path),
            None => format!("```mermaid\n{}\n```", mermaid_code),
        }
    })
    .into_owned()
}

/// Render Mermaid code to PNG via mermaid.ink API (3 retries)
fn render_mermaid_to_png(mermaid_code: &str, idx: usize, images_dir: &Path) -> Option<String> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    let encoded = URL_SAFE_NO_PAD.encode(mermaid_code.as_bytes());
    let url = format!("https://mermaid.ink/img/{}", encoded);
    let img_path = images_dir.join(format!("mermaid-{:02}.png", idx));

    for attempt in 1..=3u32 {
        match ureq::get(&url)
            .set("User-Agent", "Mozilla/5.0")
            .timeout(Duration::from_secs(30))
            .call()
        {
            Ok(resp) => {
                let mut reader = resp.into_reader();
                let mut buf = Vec::new();
                if std::io::Read::read_to_end(&mut reader, &mut buf).is_ok() && buf.len() > 200 {
                    if fs::write(&img_path, &buf).is_ok() {
                        println!("    [mermaid] rendered diagram {}", idx);
                        return Some(format!("images/mermaid-{:02}.png", idx));
                    }
                }
                if attempt < 3 {
                    thread::sleep(Duration::from_secs(2));
                }
            }
            Err(_) => {
                if attempt < 3 {
                    thread::sleep(Duration::from_secs(2));
                }
            }
        }
    }
    eprintln!("    [mermaid] FAILED diagram {}, keeping as code block", idx);
    None
}

// ── clean ────────────────────────────────────────────────────────────

fn cmd_clean() {
    let root = project_root();
    for dir_name in ["site", "docs", "epub"] {
        let dir = root.join(dir_name);
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("failed to remove dir");
            println!("Removed {dir_name}/");
        }
    }
}
