//! Zensical module for `AsciiDoc` support.
//!
//! This crate provides a Zensical module that renders `AsciiDoc` files using
//! the `asciidoctor` CLI and post-processes the HTML output for Material
//! theme compatibility.

pub mod config;
pub mod html;
pub mod renderer;

pub use config::Config;

use std::collections::BTreeMap;

use zrx::module::{Context, Module};
use zrx::stream::Value;

use crate::html::TocEntry;

// ---------------------------------------------------------------------------
// FilePath newtype
// ---------------------------------------------------------------------------

/// Newtype around `String` for file paths flowing through zrx streams.
///
/// The zrx `Value` trait is intentionally not implemented for built-in types
/// like `String`, requiring explicit newtypes so that subscriptions between
/// modules are intentional and well-defined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePath(pub String);

impl Value for FilePath {}

// ---------------------------------------------------------------------------
// RenderedDoc
// ---------------------------------------------------------------------------

/// A rendered `AsciiDoc` document ready for consumption by downstream modules.
///
/// Implements `zrx::stream::Value` so it can flow through zrx streams.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedDoc {
    pub title: String,
    pub meta: BTreeMap<String, String>,
    pub content: String,
    pub toc: Vec<TocEntry>,
}

impl Value for RenderedDoc {}

// ---------------------------------------------------------------------------
// AsciiDoc module
// ---------------------------------------------------------------------------

/// Zensical module that renders `AsciiDoc` files via Asciidoctor.
pub struct AsciiDoc {
    config: Config,
}

impl AsciiDoc {
    /// Create a new `AsciiDoc` module with the given configuration.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

impl Module for AsciiDoc {
    fn setup(&self, ctx: &mut Context) -> zrx::module::Result {
        let files = ctx.add::<FilePath>();

        let adoc_files = files.filter(|path: &FilePath| {
            Ok(std::path::Path::new(&path.0)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("adoc")))
        });

        let config = self.config.clone();
        let _ = adoc_files.map(move |path: &FilePath| {
            let renderer = crate::renderer::Renderer::new(&config);
            let raw_html = renderer
                .render(&path.0)
                .map_err(|e| zrx::scheduler::step::Error::Panic(Box::new(e)))?;

            let processor = crate::html::HtmlProcessor::new();
            let processed = processor.process(&raw_html, Some(&path.0));

            let mut meta = BTreeMap::new();
            if let Some(title) = &processed.meta.title {
                meta.insert(String::from("title"), title.clone());
            }
            if let Some(desc) = &processed.meta.description {
                meta.insert(String::from("description"), desc.clone());
            }

            let title = processed
                .meta
                .title
                .clone()
                .unwrap_or_else(|| file_stem_title(&path.0));

            Ok(RenderedDoc {
                title,
                meta,
                content: processed.html,
                toc: processed.toc,
            })
        });

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive a human-readable title from a file path by converting the file
/// stem to title case (replacing hyphens and underscores with spaces).
fn file_stem_title(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    let mut title = String::new();
    let mut capitalize_next = true;
    for ch in stem.chars() {
        if ch == '-' || ch == '_' {
            title.push(' ');
            capitalize_next = true;
        } else if capitalize_next {
            title.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            title.push(ch);
        }
    }
    title
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asciidoc_module_default_config() {
        let module = AsciiDoc::new(Config::default());
        assert_eq!(module.config.asciidoctor_cmd, "asciidoctor");
    }

    #[test]
    fn test_asciidoc_module_custom_config() {
        let config = Config {
            asciidoctor_cmd: String::from("/usr/local/bin/asciidoctor"),
            ..Config::default()
        };
        let module = AsciiDoc::new(config);
        assert_eq!(module.config.asciidoctor_cmd, "/usr/local/bin/asciidoctor");
    }

    #[test]
    fn test_rendered_doc_value_impl() {
        fn assert_value<T: Value>() {}
        assert_value::<RenderedDoc>();
    }

    #[test]
    fn test_file_path_value_impl() {
        fn assert_value<T: Value>() {}
        assert_value::<FilePath>();
    }

    #[test]
    fn test_file_stem_title() {
        assert_eq!(file_stem_title("docs/my-page.adoc"), "My Page");
        assert_eq!(file_stem_title("index.adoc"), "Index");
        assert_eq!(file_stem_title("getting_started.adoc"), "Getting Started");
    }
}
