#![cfg(feature = "integration")]

use zensical_asciidoctor_backend::config::Config;
use zensical_asciidoctor_backend::html::HtmlProcessor;
use zensical_asciidoctor_backend::renderer::Renderer;

const FIXTURE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple.adoc");

#[test]
fn test_full_pipeline() {
    let config = Config::default();
    let renderer = Renderer::new(&config);

    let raw_html = renderer
        .render(FIXTURE_PATH)
        .expect("rendering should succeed");

    // Raw HTML is non-empty and contains the title.
    assert!(!raw_html.is_empty(), "raw HTML should not be empty");
    assert!(
        raw_html.contains("Simple Test Document"),
        "raw HTML should contain the document title"
    );

    // Process the HTML.
    let processor = HtmlProcessor::new();
    let processed = processor.process(&raw_html, Some("docs/simple.adoc"));

    // Meta: In standalone mode (-s), asciidoctor does not emit h1.sect0 or
    // <meta> tags, so title and description are None.  Verify that the
    // processor gracefully handles this.
    //
    // The document title *is* present as a plain <h1> in the raw HTML, which
    // the ensure_heading_ids pass will pick up for the TOC, but the meta
    // extractor specifically looks for h1.sect0 (full-document mode).
    assert!(
        processed.meta.title.is_none()
            || processed.meta.title.as_deref() == Some("Simple Test Document"),
        "meta title should be None (standalone mode) or match the document title"
    );

    // Description is only available from <meta name=\"description\"> which
    // asciidoctor omits in standalone mode.
    assert!(
        processed.meta.description.is_none()
            || processed.meta.description.as_deref()
                == Some("A test document for integration testing"),
        "meta description should be None (standalone mode) or match"
    );

    // TOC: In standalone mode the plain <h1> (no sect0 class) is included as
    // a top-level entry, with the h2 sections as its children.  Verify we
    // have meaningful TOC data.
    assert!(!processed.toc.is_empty(), "TOC should not be empty");

    // Collect all TOC entry titles (top-level and children) so we can assert
    // that "Introduction" appears somewhere.
    fn collect_titles(entries: &[zensical_asciidoctor_backend::html::TocEntry]) -> Vec<String> {
        let mut out = Vec::new();
        for e in entries {
            out.push(e.title.clone());
            out.extend(collect_titles(&e.children));
        }
        out
    }
    let all_titles = collect_titles(&processed.toc);
    assert!(
        all_titles.len() >= 2,
        "TOC should have at least 2 entries (including nested), got {}: {:?}",
        all_titles.len(),
        all_titles
    );
    assert!(
        all_titles.iter().any(|t| t == "Introduction"),
        "TOC should contain 'Introduction', got: {:?}",
        all_titles
    );

    // Admonitions transformed.
    assert!(
        processed.html.contains("admonition note"),
        "admonitions should be transformed (expected 'admonition note')"
    );
    assert!(
        processed.html.contains("admonition-title"),
        "admonitions should have admonition-title class"
    );

    // Tables wrapped.
    assert!(
        processed.html.contains("md-typeset__table"),
        "tables should be wrapped with md-typeset__table"
    );

    // Figures transformed.
    assert!(
        processed.html.contains("adoc-figure"),
        "figures should be transformed to adoc-figure"
    );

    // Xrefs rewritten (should not contain "other.adoc").
    assert!(
        !processed.html.contains("other.adoc"),
        "xref URLs should be rewritten (should not contain 'other.adoc')"
    );
}

#[test]
fn test_renderer_error_on_missing_file() {
    let config = Config {
        fail_on_error: true,
        ..Config::default()
    };
    let renderer = Renderer::new(&config);

    let result = renderer.render("/nonexistent/path/missing.adoc");
    assert!(
        result.is_err(),
        "rendering a missing file with fail_on_error=true should return Err"
    );
}

#[test]
fn test_renderer_graceful_on_missing_file() {
    let config = Config {
        fail_on_error: false,
        ..Config::default()
    };
    let renderer = Renderer::new(&config);

    let result = renderer.render("/nonexistent/path/missing.adoc");
    assert!(
        result.is_ok(),
        "rendering a missing file with fail_on_error=false should return Ok"
    );

    let html = result.unwrap();
    assert!(
        !html.is_empty(),
        "graceful error should produce non-empty HTML"
    );
}
