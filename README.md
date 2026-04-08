# zensical-asciidoctor-backend

A [Zensical](https://zensical.org) module that adds AsciiDoc support to Zensical sites. It renders `.adoc` files through [Asciidoctor](https://asciidoctor.org) and transforms the HTML output to be fully compatible with the Zensical theme, so AsciiDoc and Markdown pages are visually indistinguishable.

## Features

- **Subprocess rendering** via the `asciidoctor` CLI -- no Ruby embedding required
- **Full Zensical theme compatibility** -- admonitions, tables, code blocks, figures, TOC, and navigation all match the Markdown output
- **Cross-reference rewriting** -- `xref:page.adoc[]` links are rewritten for directory-URL routing
- **Mixed sites** -- AsciiDoc and Markdown pages coexist in the same `docs/` directory
- **Search integration** -- AsciiDoc pages are indexed alongside Markdown pages
- **Configurable** -- safe mode, attributes, required Ruby libraries, error handling

## Prerequisites

- **Rust** 1.86+ (edition 2024)
- **Asciidoctor** -- install with `gem install asciidoctor`
- **Zensical** -- install with `pip install zensical`

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
zensical-asciidoctor-backend = { git = "https://github.com/aireilly/zensical-asciidoctor-backend.git" }
```

## Usage

### As a Zensical module

Register the module in your Zensical application:

```rust
use zensical_asciidoctor_backend::{AsciiDoc, Config, FilePath};

let config = Config::default();
let module = AsciiDoc::new(config);

// In your module setup:
// The module subscribes to FilePath streams, filters .adoc files,
// and produces RenderedDoc values with processed HTML, metadata, and TOC.
```

### Standalone HTML processing

Use the renderer and processor independently:

```rust
use zensical_asciidoctor_backend::config::Config;
use zensical_asciidoctor_backend::renderer::Renderer;
use zensical_asciidoctor_backend::html::HtmlProcessor;

let config = Config::default();
let renderer = Renderer::new(&config);
let processor = HtmlProcessor::new();

// Render an AsciiDoc file to raw HTML
let raw_html = renderer.render("docs/page.adoc")?;

// Transform to Zensical-compatible HTML
let result = processor.process(&raw_html, Some("page.adoc"));

println!("Title: {:?}", result.meta.title);
println!("TOC entries: {}", result.toc.len());
println!("HTML: {}", result.html);
```

### Configuration

```rust
use std::collections::HashMap;
use zensical_asciidoctor_backend::config::{Config, SafeMode};

let config = Config {
    asciidoctor_cmd: "asciidoctor".into(),  // Path to asciidoctor binary
    safe_mode: SafeMode::Safe,              // Safe, Unsafe, Server, or Secure
    attributes: HashMap::from([
        ("showtitle".into(), String::new()),
        ("sectanchors".into(), String::new()),
        ("source-highlighter".into(), "rouge".into()),
        ("imagesdir".into(), "images".into()),
    ]),
    requires: vec![],                       // Ruby libraries to require
    fail_on_error: true,                    // Fail build on render errors
};
```

## HTML transformations

The `HtmlProcessor` applies these transformations to Asciidoctor output:

| Transformation | Description |
|---|---|
| Metadata extraction | Extracts document title and description |
| Heading IDs | Ensures all headings have anchor IDs |
| Table of contents | Builds a hierarchical TOC from headings |
| Admonitions | Converts Asciidoctor admonition blocks to Zensical `<div class="admonition">` |
| Callout lists | Converts table-based callout lists to ordered lists |
| Code cleanup | Removes callout markers (`<b class="conum">`) from code blocks |
| Tables | Wraps in `md-typeset__table`, strips Asciidoctor classes, removes colgroups |
| Figures | Converts `div.imageblock` to HTML5 `<figure>` elements |
| Cross-references | Rewrites `xref:` links for directory-URL routing |

## Demo

The `demo/` directory contains a complete Zensical site with both Markdown and AsciiDoc pages.

```bash
# Render AsciiDoc pages to .md wrappers, then build with Zensical
make demo

# Or render and serve with live reload
make serve
```

The `render_asciidoc` example processes each `.adoc` file through Asciidoctor and the HTML post-processor, then writes a `.md` file containing the rendered HTML. Zensical builds and serves these `.md` files alongside hand-written Markdown pages, handling navigation, search, and TOC automatically.

> **Note:** Once Zensical ships a plugin registration API, this crate's
> `Module` trait implementation will allow `zensical build` to render `.adoc`
> files natively -- no separate step required.

## Testing

```bash
# Unit tests
cargo test

# Integration tests (requires asciidoctor installed)
cargo test --features integration
```

## Project structure

```
src/
  lib.rs        -- Module trait impl, FilePath/RenderedDoc types
  config.rs     -- Config struct, SafeMode enum
  renderer.rs   -- Asciidoctor subprocess invocation
  html.rs       -- HTML post-processing pipeline
tests/
  integration.rs -- Full pipeline tests (feature-gated)
  fixtures/      -- Test AsciiDoc files
examples/
  render_asciidoc.rs -- Renders .adoc files to .md wrappers for Zensical
demo/
  docs/          -- Source .adoc and .md files
  site/          -- Built output
  zensical.toml  -- Zensical config
```

## Current limitations

- **No live reload for `.adoc` files** -- `zensical serve` watches `.md` files only. Editing a `.adoc` file requires re-running `cargo run --example render_asciidoc` (or a file watcher like `watchexec -w demo/docs -e adoc -- cargo run --example render_asciidoc`) before changes appear.
- **Code line selection unavailable** -- AsciiDoc code blocks use Rouge (Pygments-compatible) highlighting with the same CSS token classes as Zensical's Pygments output, so colors match. However, the per-line wrapping spans that enable Zensical's code line-selection feature are not generated.
- **`attr_list` extension required for heading IDs** -- the generated `.md` files use `## Heading {#id}` syntax. If the `attr_list` Markdown extension is not enabled in Zensical, heading IDs will render as literal text.
- **No incremental builds** -- every run of `render_asciidoc` re-renders all `.adoc` files, even if unchanged.
- **Two-step build** -- AsciiDoc rendering is a pre-build step (`cargo run --example render_asciidoc`) rather than a native Zensical plugin. The generated `.md` files must be committed or regenerated in CI.

## Roadmap

- [ ] **Publish to crates.io** -- once the zrx dependency is published
- [ ] **Native Zensical integration** -- register as a first-class Zensical plugin so `zensical build` renders `.adoc` files automatically without the separate `render_asciidoc` step
- [ ] **Diagram support** -- render Mermaid/PlantUML diagrams embedded in AsciiDoc
- [ ] **Incremental builds** -- only re-render changed `.adoc` files

## CI/CD

The project includes a GitHub Actions workflow (`.github/workflows/ci.yml`) that:

1. Runs `cargo test` and `cargo clippy` on every push and PR
2. Runs integration tests with Asciidoctor installed
3. Builds the demo site and deploys to GitHub Pages on pushes to `main`

See [Setting up GitHub Pages](#github-pages-deployment) below.

### GitHub Pages deployment

To enable the demo site deployment:

1. Go to your repo **Settings > Pages**
2. Set **Source** to **GitHub Actions**
3. Push to `main` -- the workflow renders AsciiDoc pages, builds the Zensical site, and deploys

The deployed site demonstrates AsciiDoc and Markdown pages side-by-side with identical Zensical theme styling.

## License

MIT
