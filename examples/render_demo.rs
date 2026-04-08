//! Renders the demo AsciiDoc files through the full pipeline.
//!
//! Usage: cargo run --example render_demo
//!
//! This example processes all .adoc files in demo/docs/asciidoc/,
//! renders them through Asciidoctor + HTML post-processing, and
//! writes the output HTML to demo/site/asciidoc/.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use zensical_asciidoctor_backend::config::Config;
use zensical_asciidoctor_backend::html::HtmlProcessor;
use zensical_asciidoctor_backend::renderer::Renderer;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let docs_dir = manifest_dir.join("demo/docs/asciidoc");
    let site_dir = manifest_dir.join("demo/site/asciidoc");

    // Create output directory
    fs::create_dir_all(&site_dir).expect("failed to create output directory");

    // Configure renderer
    let config = Config {
        attributes: HashMap::from([
            (String::from("showtitle"), String::new()),
            (String::from("sectanchors"), String::new()),
            (String::from("source-highlighter"), String::from("rouge")),
            (String::from("imagesdir"), String::from("images")),
        ]),
        ..Config::default()
    };

    let renderer = Renderer::new(&config);
    let processor = HtmlProcessor::default();

    // Find all .adoc files
    let entries: Vec<_> = fs::read_dir(&docs_dir)
        .expect("failed to read docs directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "adoc")
        })
        .collect();

    println!("Rendering {} AsciiDoc files...\n", entries.len());

    for entry in entries {
        let src_path = entry.path();
        let file_name = src_path.file_stem().unwrap().to_string_lossy();
        let rel_path = format!("asciidoc/{}", src_path.file_name().unwrap().to_string_lossy());

        print!("  {} ... ", src_path.file_name().unwrap().to_string_lossy());

        // Render through Asciidoctor
        let raw_html = match renderer.render(src_path.to_str().unwrap()) {
            Ok(html) => html,
            Err(err) => {
                println!("FAILED: {err}");
                continue;
            }
        };

        // Post-process HTML
        let processed = processor.process(&raw_html, Some(&rel_path));

        // Wrap in a basic HTML page
        let title = processed.meta.title.as_deref().unwrap_or(&file_name);
        let toc_html = build_toc_html(&processed.toc);

        let full_html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{title}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; max-width: 900px; margin: 0 auto; padding: 2rem; line-height: 1.6; color: #333; }}
        h1 {{ border-bottom: 2px solid #4051b5; padding-bottom: 0.5rem; }}
        h2 {{ border-bottom: 1px solid #e0e0e0; padding-bottom: 0.3rem; }}
        pre {{ background: #f5f5f5; padding: 1rem; border-radius: 4px; overflow-x: auto; }}
        code {{ background: #f5f5f5; padding: 0.2em 0.4em; border-radius: 3px; font-size: 0.9em; }}
        pre code {{ background: none; padding: 0; }}
        .admonition {{ border-left: 4px solid #448aff; padding: 0.8rem 1rem; margin: 1rem 0; background: #f8f9fa; border-radius: 0 4px 4px 0; }}
        .admonition.warning {{ border-left-color: #ff9100; }}
        .admonition.danger {{ border-left-color: #ff1744; }}
        .admonition.tip {{ border-left-color: #00c853; }}
        .admonition-title {{ font-weight: 700; margin-bottom: 0.3rem; }}
        .md-typeset__table {{ overflow-x: auto; }}
        table {{ border-collapse: collapse; width: 100%; }}
        th, td {{ border: 1px solid #ddd; padding: 0.6rem; text-align: left; }}
        th {{ background: #f5f5f5; }}
        .adoc-figure {{ margin: 1rem 0; text-align: center; }}
        figcaption {{ font-style: italic; color: #666; margin-top: 0.5rem; }}
        nav.toc {{ background: #f8f9fa; padding: 1rem; border-radius: 4px; margin-bottom: 2rem; }}
        nav.toc ul {{ list-style: none; padding-left: 1.2rem; }}
        nav.toc > ul {{ padding-left: 0; }}
        nav.toc a {{ text-decoration: none; color: #4051b5; }}
        nav.toc a:hover {{ text-decoration: underline; }}
        .colist {{ padding-left: 1.5rem; }}
        blockquote {{ border-left: 3px solid #ccc; margin: 1rem 0; padding: 0.5rem 1rem; color: #666; }}
    </style>
</head>
<body>
    <nav class="toc">
        <strong>Table of Contents</strong>
        {toc_html}
    </nav>
    {content}
</body>
</html>"#,
            title = title,
            toc_html = toc_html,
            content = processed.html,
        );

        // Write output
        let out_dir = site_dir.join(&*file_name);
        fs::create_dir_all(&out_dir).expect("failed to create output dir");
        let out_path = out_dir.join("index.html");
        fs::write(&out_path, &full_html).expect("failed to write HTML");

        println!("OK (title: {title}, toc: {} entries)", processed.toc.len());
    }

    println!("\nDone! Output written to demo/site/asciidoc/");
    println!("Open demo/site/asciidoc/index/index.html in a browser to view.");
}

fn build_toc_html(entries: &[zensical_asciidoctor_backend::html::TocEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut html = String::from("<ul>");
    for entry in entries {
        html.push_str(&format!(
            "<li><a href=\"#{}\">{}</a>",
            entry.id, entry.title
        ));
        if !entry.children.is_empty() {
            html.push_str(&build_toc_html(&entry.children));
        }
        html.push_str("</li>");
    }
    html.push_str("</ul>");
    html
}
