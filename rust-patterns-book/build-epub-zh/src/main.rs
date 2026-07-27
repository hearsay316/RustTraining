//! 构建 rust-patterns-book 中文 EPUB
//!
//! 功能：
//! 1. 将 Mermaid 图表通过 mermaid.ink 渲染为 PNG 图片（兼容所有 EPUB 阅读器）
//! 2. 替换 markdown 中的 mermaid 代码块为图片引用
//! 3. 应用代码块优化 CSS
//! 4. 调用 mdbook 生成 EPUB

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use regex::Regex;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

const EPUB_CSS: &str = r#"/* EPUB 代码块与排版优化 */
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
"#;

const BOOK_TOML: &str = r#"[book]
title = "Rust 模式与工程实践"
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
"#;

fn book_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("must be in rust-patterns-book/build-epub-zh/")
        .to_path_buf()
}

/// 通过 mermaid.ink 将 Mermaid 代码渲染为 PNG 图片（带 3 次重试）
fn render_mermaid_to_png(
    mermaid_code: &str,
    idx: usize,
    images_dir: &Path,
) -> Option<String> {
    let encoded = URL_SAFE_NO_PAD.encode(mermaid_code.as_bytes());
    let url = format!("https://mermaid.ink/img/{}", encoded);
    let img_path = images_dir.join(format!("mermaid-{:02}.png", idx));

    for attempt in 1..=3 {
        match ureq::get(&url)
            .set("User-Agent", "Mozilla/5.0")
            .timeout(Duration::from_secs(30))
            .call()
        {
            Ok(resp) => {
                let mut reader = resp.into_reader();
                let mut buf = Vec::new();
                if std::io::Read::read_to_end(&mut reader, &mut buf).is_ok()
                    && buf.len() > 200
                {
                    if fs::write(&img_path, &buf).is_ok() {
                        println!("  [OK] mermaid-{:02}.png ({} bytes)", idx, buf.len());
                        return Some(format!("images/mermaid-{:02}.png", idx));
                    }
                }
                println!("  [WARN] mermaid-{:02} attempt {} too small", idx, attempt);
            }
            Err(e) => {
                println!("  [WARN] mermaid-{:02} attempt {} failed: {}", idx, attempt, e);
            }
        }
        if attempt < 3 {
            thread::sleep(Duration::from_secs(2));
        }
    }
    println!("  [FAIL] mermaid-{:02}.png — keeping as code block", idx);
    None
}

/// 将 markdown 中的 ```mermaid 块替换为渲染后的图片引用
fn process_mermaid_blocks(
    content: &str,
    counter: &mut usize,
    images_dir: &Path,
) -> String {
    let re = Regex::new(r"(?s)```mermaid\n(.*?)```").unwrap();
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

fn rmrf(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

fn main() {
    let root = book_dir();
    let src_zh = root.join("src-zh");
    let build_dir = root.join(".epub-zh-build");
    let output_dir = root.join("book-zh-epub");

    println!("{}", "=".repeat(60));
    println!("Building rust-patterns-book Chinese EPUB (Rust)");
    println!("{}", "=".repeat(60));

    // ── [1/4] 准备构建目录 ──
    println!("\n[1/4] Preparing build directory...");
    rmrf(&build_dir);
    let src_dir = build_dir.join("src");
    let images_dir = src_dir.join("images");
    fs::create_dir_all(&images_dir).expect("failed to create build dirs");

    // ── [2/4] 复制并处理 markdown 文件 ──
    println!("\n[2/4] Processing markdown files and rendering Mermaid diagrams...");
    let mermaid_re = Regex::new(r"```mermaid").unwrap();
    let mut counter = 0usize;

    // 按文件名排序
    let mut md_files: BTreeMap<String, PathBuf> = BTreeMap::new();
    for entry in fs::read_dir(&src_zh).expect("failed to read src-zh") {
        if let Ok(entry) = entry {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".md") {
                md_files.insert(name, entry.path());
            }
        }
    }

    for (name, path) in &md_files {
        let content = fs::read_to_string(path).expect("failed to read markdown file");
        let processed = if mermaid_re.is_match(&content) {
            process_mermaid_blocks(&content, &mut counter, &images_dir)
        } else {
            content
        };
        fs::write(src_dir.join(name), processed).expect("failed to write processed markdown");
        println!("  [OK] {}", name);
    }

    // ── [3/4] 写入 CSS 和 book.toml ──
    println!("\n[3/4] Writing CSS and book.toml...");
    fs::write(build_dir.join("epub-code.css"), EPUB_CSS).expect("failed to write CSS");
    fs::write(build_dir.join("book.toml"), BOOK_TOML).expect("failed to write book.toml");

    // ── [4/4] 构建 EPUB ──
    println!("\n[4/4] Building EPUB with mdbook...");
    let output = Command::new("mdbook")
        .arg("build")
        .current_dir(&build_dir)
        .output()
        .expect("failed to run mdbook — is it installed?");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.is_empty() {
        println!("{}", stdout.trim_end());
    }
    if !stderr.is_empty() {
        eprintln!("STDERR: {}", stderr.trim_end());
    }

    if !output.status.success() {
        eprintln!("\n[FAIL] mdbook build failed");
        eprintln!("Build directory kept for debugging: {}", build_dir.display());
        std::process::exit(1);
    }

    // ── 复制 EPUB 到输出目录 ──
    let epub_dir = build_dir.join("book-zh-epub").join("epub");
    if !epub_dir.exists() {
        eprintln!("\n[FAIL] No EPUB output found");
        eprintln!("Build directory kept for debugging: {}", build_dir.display());
        std::process::exit(1);
    }

    fs::create_dir_all(&output_dir).expect("failed to create output dir");

    let mut found = false;
    for entry in fs::read_dir(&epub_dir).expect("failed to read epub dir") {
        if let Ok(entry) = entry {
            let src = entry.path();
            if src.extension().and_then(|e| e.to_str()) == Some("epub") {
                let meta = fs::metadata(&src).expect("failed to stat epub");
                if meta.len() < 1000 {
                    eprintln!("[FAIL] EPUB file too small ({} bytes), likely broken", meta.len());
                    continue;
                }
                let dest = output_dir.join(entry.file_name());
                fs::copy(&src, &dest).expect("failed to copy epub");
                let size_kb = dest.metadata().map(|m| m.len() / 1024).unwrap_or(0);
                println!("\n[OK] EPUB generated: {} ({} KB)", dest.display(), size_kb);
                found = true;
            }
        }
    }

    if found {
        rmrf(&build_dir);
        println!("\nDone!");
    } else {
        eprintln!("\n[FAIL] No valid EPUB file produced");
        eprintln!("Build directory kept for debugging: {}", build_dir.display());
        std::process::exit(1);
    }
}
