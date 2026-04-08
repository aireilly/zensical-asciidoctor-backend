//! Renders the demo AsciiDoc files into Markdown wrappers for Zensical.
//!
//! Usage: cargo run --example render_asciidoc
//!
//! This example processes all .adoc files in demo/docs/,
//! renders them through Asciidoctor + HTML post-processing, and
//! writes .md files that Zensical can build and serve alongside
//! hand-written Markdown pages.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use regex::Regex;

use zensical_asciidoctor_backend::config::Config;
use zensical_asciidoctor_backend::html::HtmlProcessor;
use zensical_asciidoctor_backend::renderer::Renderer;

/// Asciidoctor structural wrapper classes/IDs to unwrap.
/// These divs have no visual meaning — they just group sections.
const STRUCTURAL_CLASSES: &[&str] = &[
    "sect1",
    "sect2",
    "sect3",
    "sect4",
    "sect5",
    "sect6",
    "sectionbody",
    "paragraph",
    "ulist",
    "olist",
    "dlist",
    "listingblock",
    "content",
    "imageblock",
];

const STRUCTURAL_IDS: &[&str] = &["preamble"];

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let docs_dir = manifest_dir.join("demo/docs");

    // Configure renderer.
    // Use ../images as imagesdir because Zensical serves each page from its
    // own subdirectory (e.g. /asciidoc/), so image paths need to go up one
    // level to reach /images/.
    let config = Config {
        attributes: HashMap::from([
            (String::from("showtitle"), String::new()),
            (String::from("sectanchors"), String::new()),
            (String::from("source-highlighter"), String::from("rouge")),
            (String::from("imagesdir"), String::from("../images")),
        ]),
        ..Config::default()
    };

    let renderer = Renderer::new(&config);
    let processor = HtmlProcessor::new();

    // Find all .adoc files in docs/
    let entries: Vec<_> = fs::read_dir(&docs_dir)
        .expect("failed to read docs directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "adoc"))
        .collect();

    println!("Rendering {} AsciiDoc files...\n", entries.len());

    for entry in &entries {
        let src_path = entry.path();
        let file_stem = src_path.file_stem().unwrap().to_string_lossy().to_string();
        let rel_path = src_path.file_name().unwrap().to_string_lossy().to_string();

        print!("  {} ... ", rel_path);

        let raw_html = match renderer.render(src_path.to_str().unwrap()) {
            Ok(html) => html,
            Err(err) => {
                println!("FAILED: {err}");
                continue;
            }
        };

        let processed = processor.process(&raw_html, Some(&rel_path));
        let title = processed.meta.title.unwrap_or_else(|| file_stem.clone());

        // Strip asciidoctor structural wrappers and convert headings to
        // Markdown so Zensical's TOC and Markdown parser work correctly.
        let md_body = to_markdown_hybrid(&processed.html);

        let md_path = docs_dir.join(format!("{file_stem}.md"));
        let md_content = format!(
            "<!-- Generated from {} by render_asciidoc — do not edit -->\n\n{}\n",
            rel_path, md_body,
        );
        fs::write(&md_path, &md_content).expect("failed to write .md file");

        println!("OK (title: {title}, toc: {} entries)", processed.toc.len());
    }

    println!("\nDone! Generated .md files in demo/docs/");
}

/// Strip Asciidoctor structural wrapper divs and convert headings to
/// Markdown. This produces a file that mixes Markdown headings with raw
/// HTML content blocks — each properly separated by blank lines so that
/// Python-Markdown (used by Zensical) recognises the headings for TOC.
fn to_markdown_hybrid(html: &str) -> String {
    let stripped = strip_structural_divs(html);
    headings_to_markdown(&stripped)
}

/// Remove Asciidoctor structural `<div>` wrappers (and their matching
/// `</div>` closers) by tracking nesting depth. Content divs like
/// admonitions and table wrappers are preserved.
fn strip_structural_divs(html: &str) -> String {
    let lines: Vec<&str> = html.lines().collect();
    let mut result: Vec<&str> = Vec::with_capacity(lines.len());

    // Depths at which we've stripped a structural opening div.
    let mut structural_depths: Vec<usize> = Vec::new();
    let mut depth: usize = 0;

    for line in &lines {
        let trimmed = line.trim();

        // Count ALL div opens and closes on this line so that multi-line
        // content blocks (e.g. `</code></pre></div>`) adjust depth correctly.
        let opens = trimmed.matches("<div").count();
        let closes = trimmed.matches("</div>").count();

        // Structural wrappers always appear as a single opening div on
        // their own line (no closing tag on the same line).
        if opens == 1 && closes == 0 && trimmed.starts_with("<div") && is_structural_div(trimmed) {
            depth += 1;
            structural_depths.push(depth);
            continue;
        }

        // Adjust depth for non-structural opens on this line.
        depth += opens;

        // Standalone `</div>` — check if it closes a structural wrapper.
        if trimmed == "</div>" && structural_depths.last() == Some(&depth) {
            structural_depths.pop();
            depth = depth.saturating_sub(closes);
            continue;
        }

        // Adjust depth for closes on this line.
        depth = depth.saturating_sub(closes);

        result.push(line);
    }

    result.join("\n")
}

fn is_structural_div(line: &str) -> bool {
    STRUCTURAL_CLASSES
        .iter()
        .any(|c| line.contains(&format!("class=\"{c}\"")))
        || STRUCTURAL_IDS
            .iter()
            .any(|id| line.contains(&format!("id=\"{id}\"")))
        // Also match class="olist arabic" etc. (olist with subclass)
        || line.contains("class=\"olist ")
}

/// Replace `<h1-6 id="...">...</h1-6>` with Markdown headings.
fn headings_to_markdown(html: &str) -> String {
    let heading_re = Regex::new(r#"<h([1-6])\s+id="([^"]*)">(.*?)</h[1-6]>"#).unwrap();
    let anchor_re = Regex::new(r#"<a class="anchor"[^>]*></a>\s*"#).unwrap();

    heading_re
        .replace_all(html, |caps: &regex::Captures| {
            let level: usize = caps[1].parse().unwrap();
            let id = &caps[2];
            let inner = &caps[3];
            let text = anchor_re.replace_all(inner, "");
            let hashes = "#".repeat(level);
            format!("\n{hashes} {text} {{#{id}}}\n")
        })
        .to_string()
}
