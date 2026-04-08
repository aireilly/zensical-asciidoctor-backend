//! HTML post-processor for Asciidoctor output.
//!
//! Transforms raw Asciidoctor HTML into Material-theme-compatible markup,
//! extracting metadata and table-of-contents along the way.
//!
//! Uses `scraper` for read-only operations (metadata extraction, TOC building)
//! and `regex` for all HTML-modifying transformations, because scraper
//! normalises the DOM (e.g. inserting `<tbody>`) which makes string-replacement
//! against the original HTML unreliable.

use std::fmt::Write as _;

use regex::Regex;
use scraper::{Html, Selector};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A single table-of-contents entry (may have nested children).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TocEntry {
    pub title: String,
    pub id: String,
    pub level: u8,
    pub children: Vec<TocEntry>,
}

/// Metadata extracted from the rendered HTML document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocMeta {
    pub title: Option<String>,
    pub description: Option<String>,
}

/// The result of processing a rendered HTML document.
#[derive(Clone, Debug)]
pub struct ProcessedDoc {
    pub html: String,
    pub toc: Vec<TocEntry>,
    pub meta: DocMeta,
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Convert arbitrary text into a URL-friendly slug (kebab-case).
///
/// Strips non-alphanumeric characters (except spaces, underscores, hyphens),
/// lowercases, and replaces whitespace / underscores with hyphens.
///
/// # Panics
///
/// Panics if the internal regex patterns are invalid (this should never happen).
#[must_use]
pub fn slugify(text: &str) -> String {
    let t = text.trim().to_lowercase();
    let nonword = Regex::new(r"[^0-9A-Za-z _-]+").expect("valid regex");
    let t = nonword.replace_all(&t, "");
    let spaces = Regex::new(r"[ _]+").expect("valid regex");
    spaces.replace_all(&t, "-").into_owned()
}

/// Return the parent directory portion of a POSIX path.
fn parent_path(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[..i],
        None => "",
    }
}

/// Return the file stem (name without extension) of a POSIX path.
fn file_stem(path: &str) -> &str {
    let name = match path.rfind('/') {
        Some(i) => &path[i + 1..],
        None => path,
    };
    match name.rfind('.') {
        Some(i) => &name[..i],
        None => name,
    }
}

/// Normalise a POSIX path by resolving `.` and `..` components.
fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// Compute a relative POSIX path from `from_dir` to `to`.
fn relative_path(to: &str, from_dir: &str) -> String {
    if from_dir.is_empty() || from_dir == "." {
        return to.to_string();
    }

    let to_parts: Vec<&str> = if to.is_empty() {
        vec![]
    } else {
        to.split('/').collect()
    };
    let from_parts: Vec<&str> = from_dir.split('/').collect();

    let common = to_parts
        .iter()
        .zip(from_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let ups = from_parts.len() - common;
    let mut result: Vec<&str> = vec![".."; ups];
    for part in &to_parts[common..] {
        result.push(part);
    }
    if result.is_empty() {
        ".".to_string()
    } else {
        result.join("/")
    }
}

/// Strip HTML tags from a string, replacing them with spaces.
fn strip_tags(html: &str) -> String {
    let re = Regex::new(r"<[^>]+>").expect("valid regex");
    re.replace_all(html, " ").to_string()
}

// ---------------------------------------------------------------------------
// HtmlProcessor
// ---------------------------------------------------------------------------

/// Post-processes Asciidoctor HTML for Material-theme compatibility.
#[derive(Clone, Debug)]
pub struct HtmlProcessor;

impl Default for HtmlProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::unused_self)]
impl HtmlProcessor {
    /// Create a new processor.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Run all transformations on the given HTML fragment.
    ///
    /// `page_rel_path` is the page's source-relative path (e.g.
    /// `"guides/install.adoc"`), used to resolve cross-reference URLs.
    #[must_use]
    pub fn process(&self, html: &str, page_rel_path: Option<&str>) -> ProcessedDoc {
        let mut result = html.to_string();

        // 1. Extract metadata (read-only, uses scraper).
        let meta = self.extract_meta(&result);

        // 2. Ensure headings have IDs (regex-based).
        result = self.ensure_heading_ids(&result);

        // 3. Extract TOC (read-only, uses scraper, after IDs are in place).
        let toc = self.extract_toc(&result);

        // 4-9. Transform content (all regex-based).
        result = self.transform_admonitions(&result);
        result = self.transform_callout_lists(&result);
        result = self.clean_callout_markers(&result);
        result = self.transform_code_blocks(&result);
        result = self.transform_tables(&result);
        result = self.transform_figures(&result);

        // 10. Fix xref URLs (regex-based).
        result = self.fix_xref_urls(&result, page_rel_path);

        ProcessedDoc {
            html: result,
            toc,
            meta,
        }
    }

    // -- Metadata (scraper, read-only) --------------------------------------

    fn extract_meta(&self, html: &str) -> DocMeta {
        let doc = Html::parse_fragment(html);
        let mut meta = DocMeta::default();

        let sel_h1 = Selector::parse("h1").expect("valid selector");
        if let Some(el) = doc.select(&sel_h1).next() {
            let text: String = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
            if !text.is_empty() {
                meta.title = Some(text);
            }
        }

        let sel_meta = Selector::parse("meta[name=description]").expect("valid selector");
        if let Some(el) = doc.select(&sel_meta).next() {
            if let Some(content) = el.value().attr("content") {
                if !content.is_empty() {
                    meta.description = Some(content.to_string());
                }
            }
        }

        meta
    }

    // -- Heading IDs (regex) ------------------------------------------------

    fn ensure_heading_ids(&self, html: &str) -> String {
        let has_id = Regex::new(r"(?i)\bid\s*=").expect("valid regex");
        let mut result = html.to_string();

        // Process each heading level separately to avoid backreference issues.
        for level in 1..=6u8 {
            let tag = format!("h{level}");
            // Match <hN ...>...</hN> for this specific level.
            let pattern = format!(r"(?is)<({tag})(\s[^>]*)?>(.+?)</{tag}>");
            let re = Regex::new(&pattern).expect("valid regex");

            result = re
                .replace_all(&result, |caps: &regex::Captures| {
                    let tag_name = &caps[1];
                    let attrs = caps.get(2).map_or("", |m| m.as_str());
                    let inner = &caps[3];

                    if has_id.is_match(attrs) {
                        caps[0].to_string()
                    } else {
                        let text = strip_tags(inner);
                        let id = slugify(text.trim());
                        if id.is_empty() {
                            caps[0].to_string()
                        } else {
                            format!("<{tag_name} id=\"{id}\"{attrs}>{inner}</{tag_name}>")
                        }
                    }
                })
                .into_owned();
        }

        result
    }

    // -- TOC (scraper, read-only) -------------------------------------------

    fn extract_toc(&self, html: &str) -> Vec<TocEntry> {
        let doc = Html::parse_fragment(html);
        let heading_sel = Selector::parse("h1, h2, h3, h4, h5, h6").expect("valid selector");

        let mut items: Vec<TocEntry> = Vec::new();
        let mut stack: Vec<(u8, Vec<usize>)> = Vec::new();

        for el in doc.select(&heading_sel) {
            // Skip h1 headings (document title).
            if el.value().name() == "h1" {
                continue;
            }

            let level = el.value().name()[1..].parse::<u8>().unwrap_or(2);

            let title: String = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
            let id = el.value().attr("id").unwrap_or("").to_string();

            let entry = TocEntry {
                title,
                id,
                level,
                children: Vec::new(),
            };

            // Pop stack entries whose level is >= current.
            while let Some((lv, _)) = stack.last() {
                if *lv >= level {
                    stack.pop();
                } else {
                    break;
                }
            }

            if let Some((_lv, path)) = stack.last() {
                let path = path.clone();
                let parent = get_entry_mut(&mut items, &path);
                let idx = parent.children.len();
                parent.children.push(entry);
                let mut new_path = path;
                new_path.push(idx);
                stack.push((level, new_path));
            } else {
                let idx = items.len();
                items.push(entry);
                stack.push((level, vec![idx]));
            }
        }

        items
    }

    // -- Admonitions (regex) ------------------------------------------------

    fn transform_admonitions(&self, html: &str) -> String {
        // Find the start of each admonitionblock, then find its matching end
        // by counting div nesting depth.
        let start_re =
            Regex::new(r#"<div\s+class="admonitionblock\s+(\w+)"[^>]*>"#).expect("valid regex");

        // Pre-compile regexes used inside the loop.
        let title_re = Regex::new(
            r#"(?is)<td\s+class="icon"[^>]*>.*?<div\s+class="title"[^>]*>\s*(.*?)\s*</div>"#,
        )
        .expect("valid regex");
        let content_re =
            Regex::new(r#"(?is)<td\s+class="content"[^>]*>(.*?)</td>"#).expect("valid regex");

        let mut result = html.to_string();
        // Process repeatedly until no more admonition blocks are found.
        loop {
            let start_match = start_re.find(&result);
            if start_match.is_none() {
                break;
            }
            let m = start_match.unwrap();
            let start_pos = m.start();
            let kind = start_re.captures(&result[start_pos..]).unwrap()[1].to_string();

            // Find the matching closing </div> by counting nesting.
            let after_open = m.end();
            let end_pos = find_matching_close_div(&result, after_open);
            if end_pos.is_none() {
                break;
            }
            let end_pos = end_pos.unwrap(); // position after the closing </div>

            let block_html = &result[start_pos..end_pos];

            let material_kind = match kind.as_str() {
                "caution" => "warning",
                "important" => "danger",
                other => other,
            };

            // Extract title from the icon td's .title div.
            let title_text = title_re
                .captures(block_html)
                .map_or_else(|| capitalize(&kind), |c| c[1].to_string());

            // Extract content from td.content.
            let content = content_re
                .captures(block_html)
                .map(|c| c[1].trim().to_string())
                .unwrap_or_default();

            let replacement = format!(
                "<div class=\"admonition {material_kind}\">\
                 <p class=\"admonition-title\">{title_text}</p>\
                 {content}\
                 </div>",
            );

            result.replace_range(start_pos..end_pos, &replacement);
        }

        result
    }

    // -- Callout lists (regex) ----------------------------------------------

    fn transform_callout_lists(&self, html: &str) -> String {
        let start_re = Regex::new(r#"<div\s+class="colist"[^>]*>"#).expect("valid regex");
        let tr_re = Regex::new(r"(?is)<tr[^>]*>\s*<td[^>]*>.*?</td>\s*<td[^>]*>(.*?)</td>\s*</tr>")
            .expect("valid regex");

        let mut result = html.to_string();

        loop {
            let start_match = start_re.find(&result);
            if start_match.is_none() {
                break;
            }
            let m = start_match.unwrap();
            let start_pos = m.start();
            let after_open = m.end();

            let end_pos = find_matching_close_div(&result, after_open);
            if end_pos.is_none() {
                break;
            }
            let end_pos = end_pos.unwrap();

            let block_html = &result[start_pos..end_pos];

            // Extract list items from tr elements.
            let mut items = Vec::new();
            for cap in tr_re.captures_iter(block_html) {
                items.push(cap[1].trim().to_string());
            }

            if items.is_empty() {
                // Skip this block to avoid infinite loop.
                // We need to mark it somehow - just break to prevent infinite loop.
                break;
            }

            let mut ol = String::from("<ol class=\"colist\">");
            for item in &items {
                let _ = write!(ol, "<li>{item}</li>");
            }
            ol.push_str("</ol>");

            result.replace_range(start_pos..end_pos, &ol);
        }

        result
    }

    // -- Code callout cleanup (regex) ---------------------------------------

    fn clean_callout_markers(&self, html: &str) -> String {
        let re = Regex::new(r"(</span>)\s*(?:\(\d+\)|<\d+>|&lt;\d+&gt;)").expect("valid regex");

        re.replace_all(html, "$1").into_owned()
    }

    // -- Code blocks (regex) ------------------------------------------------

    /// Convert Rouge-highlighted code blocks from Asciidoctor format to
    /// Zensical format so that Material theme CSS applies correctly.
    ///
    /// Asciidoctor produces:
    ///   `<pre class="rouge highlight"><code data-lang="rust">...</code></pre>`
    ///
    /// Zensical expects:
    ///   `<div class="language-rust highlight"><pre><code>...</code></pre></div>`
    fn transform_code_blocks(&self, html: &str) -> String {
        let re = Regex::new(
            r#"(?s)<pre\s+class="rouge highlight"><code\s+data-lang="([^"]+)">(.*?)</code></pre>"#,
        )
        .expect("valid regex");

        re.replace_all(html, |caps: &regex::Captures| {
            let lang = &caps[1];
            let code = &caps[2];
            format!("<div class=\"language-{lang} highlight\"><pre><code>{code}</code></pre></div>")
        })
        .into_owned()
    }

    // -- Tables (regex) -----------------------------------------------------

    fn transform_tables(&self, html: &str) -> String {
        let mut result = html.to_string();

        // First pass: handle tables inside div.tableblock with a title.
        let outer_re = Regex::new(r#"<div\s+class="tableblock"[^>]*>"#).expect("valid regex");

        // Pre-compile regexes used inside the loop.
        let title_re =
            Regex::new(r#"(?is)<div\s+class="title"[^>]*>(.*?)</div>"#).expect("valid regex");
        let inner_table_re =
            Regex::new(r#"(?is)(<table\s+class="[^"]*tableblock[^"]*"[^>]*>)(.*?</table>)"#)
                .expect("valid regex");

        // Process outer div.tableblock wrappers.
        loop {
            let m = outer_re.find(&result);
            if m.is_none() {
                break;
            }
            let m = m.unwrap();
            let start_pos = m.start();
            let after_open = m.end();

            let end_pos = find_matching_close_div(&result, after_open);
            if end_pos.is_none() {
                break;
            }
            let end_pos = end_pos.unwrap();

            let block_html = result[start_pos..end_pos].to_string();

            // Extract title if present.
            let title_text = title_re
                .captures(&block_html)
                .map(|c| strip_tags(&c[1]).trim().to_string());

            // Remove the title div from the block.
            let without_title = title_re.replace(&block_html, "").to_string();

            let replacement = if let Some(tcap) = inner_table_re.captures(&without_title) {
                let table_open = &tcap[1];
                let table_rest = &tcap[2];

                let new_table = if let Some(ref title) = title_text {
                    if title.is_empty() {
                        format!("{table_open}{table_rest}")
                    } else {
                        format!("{table_open}<caption>{title}</caption>{table_rest}")
                    }
                } else {
                    format!("{table_open}{table_rest}")
                };

                format!("<div class=\"md-typeset__table\">{new_table}</div>")
            } else {
                // No table found, leave as is but we need to avoid infinite loop.
                break;
            };

            result.replace_range(start_pos..end_pos, &replacement);
        }

        // Second pass: handle standalone table.tableblock (not inside div.tableblock).
        let table_re =
            Regex::new(r#"(?is)(<table\s+class="[^"]*tableblock[^"]*"[^>]*>.*?</table>)"#)
                .expect("valid regex");

        // Wrap any remaining unwrapped table.tableblock elements.
        result = table_re
            .replace_all(&result, |caps: &regex::Captures| {
                format!("<div class=\"md-typeset__table\">{}</div>", &caps[0])
            })
            .into_owned();

        // Clean up double-wrapping: if a table was already wrapped in pass 1,
        // pass 2 would have double-wrapped it.
        result = result.replace(
            "<div class=\"md-typeset__table\"><div class=\"md-typeset__table\">",
            "<div class=\"md-typeset__table\">",
        );
        result = result.replace("</div></div>", "</div>");

        // Strip Asciidoctor-specific classes from table elements so Material
        // styles apply cleanly. Remove class attributes entirely from table,
        // th, td elements (Asciidoctor classes like tableblock, frame-all,
        // grid-all, halign-left, valign-top fight Material's styling).
        let table_class_re =
            Regex::new(r#"<table\s+class="[^"]*tableblock[^"]*"([^>]*)>"#).expect("valid regex");
        result = table_class_re
            .replace_all(&result, "<table$1>")
            .into_owned();

        let th_class_re =
            Regex::new(r#"<th\s+class="[^"]*tableblock[^"]*">"#).expect("valid regex");
        result = th_class_re.replace_all(&result, "<th>").into_owned();

        let td_class_re =
            Regex::new(r#"<td\s+class="[^"]*tableblock[^"]*">"#).expect("valid regex");
        result = td_class_re.replace_all(&result, "<td>").into_owned();

        // Unwrap <p class="tableblock">...</p> inside cells — Material doesn't
        // expect paragraph wrappers inside table cells.
        let p_tableblock_re =
            Regex::new(r#"<p\s+class="tableblock">(.*?)</p>"#).expect("valid regex");
        result = p_tableblock_re.replace_all(&result, "$1").into_owned();

        // Remove colgroup/col elements — let Material CSS handle column widths.
        let colgroup_re = Regex::new(r#"(?is)<colgroup>.*?</colgroup>"#).expect("valid regex");
        result = colgroup_re.replace_all(&result, "").into_owned();

        // Convert <caption class="title">Table N. ...</caption> to plain <caption>.
        let caption_class_re =
            Regex::new(r#"<caption\s+class="title">(?:Table\s+\d+\.\s*)?(.*?)</caption>"#)
                .expect("valid regex");
        result = caption_class_re
            .replace_all(&result, "<caption>$1</caption>")
            .into_owned();

        result
    }

    // -- Figures (regex) ----------------------------------------------------

    fn transform_figures(&self, html: &str) -> String {
        let start_re = Regex::new(r#"<div\s+class="imageblock"[^>]*>"#).expect("valid regex");

        // Pre-compile regexes used inside the loop.
        let content_re =
            Regex::new(r#"(?is)<div\s+class="content"[^>]*>(.*?)</div>"#).expect("valid regex");
        let fig_title_re =
            Regex::new(r#"(?is)<div\s+class="title"[^>]*>(.*?)</div>"#).expect("valid regex");

        let mut result = html.to_string();

        loop {
            let m = start_re.find(&result);
            if m.is_none() {
                break;
            }
            let m = m.unwrap();
            let start_pos = m.start();
            let after_open = m.end();

            let end_pos = find_matching_close_div(&result, after_open);
            if end_pos.is_none() {
                break;
            }
            let end_pos = end_pos.unwrap();

            let block_html = result[start_pos..end_pos].to_string();

            // Extract content div's inner HTML.
            let content_inner = match content_re.captures(&block_html) {
                Some(c) => c[1].to_string(),
                None => {
                    // No content div, skip this block to prevent infinite loop.
                    break;
                }
            };

            // Extract title if present.
            let title_text = fig_title_re
                .captures(&block_html)
                .map(|c| strip_tags(&c[1]).trim().to_string());

            let figure = if let Some(ref title) = title_text {
                if title.is_empty() {
                    format!("<figure class=\"adoc-figure\">{content_inner}</figure>")
                } else {
                    format!(
                        "<figure class=\"adoc-figure\"><figcaption>{title}</figcaption>{content_inner}</figure>"
                    )
                }
            } else {
                format!("<figure class=\"adoc-figure\">{content_inner}</figure>")
            };

            result.replace_range(start_pos..end_pos, &figure);
        }

        result
    }

    // -- Xref URLs (regex) --------------------------------------------------

    fn fix_xref_urls(&self, html: &str, page_rel_path: Option<&str>) -> String {
        let re = Regex::new(r#"(<a\s[^>]*?)href="([^"]*)"([^>]*>)"#).expect("valid regex");

        re.replace_all(html, |caps: &regex::Captures| {
            let prefix = &caps[1];
            let href = &caps[2];
            let suffix = &caps[3];

            let new_href = rewrite_href(href, page_rel_path);
            format!("{prefix}href=\"{new_href}\"{suffix}")
        })
        .into_owned()
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Find the position immediately after the matching `</div>` for a `<div>` tag
/// whose content starts at `content_start`.
///
/// Returns `None` if no matching close is found.
fn find_matching_close_div(html: &str, content_start: usize) -> Option<usize> {
    let bytes = html.as_bytes();
    let mut depth = 1;
    let mut i = content_start;

    while i < bytes.len() && depth > 0 {
        if bytes[i] == b'<' {
            // Check for </div>
            if html[i..].starts_with("</div>") {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 6); // past </div>
                }
                i += 6;
                continue;
            }
            // Check for <div (self-closing or opening)
            if html[i..].starts_with("<div") {
                // Check it's actually a div tag (not <divx or similar)
                let after = html.as_bytes().get(i + 4).copied();
                if matches!(after, Some(b' ' | b'>' | b'/' | b'\n' | b'\r' | b'\t')) {
                    // Check if it's self-closing.
                    if let Some(close) = html[i..].find('>') {
                        let tag = &html[i..=(i + close)];
                        if !tag.ends_with("/>") {
                            depth += 1;
                        }
                    }
                }
            }
        }
        i += 1;
    }

    None
}

/// Navigate into a nested `TocEntry` tree using an index path.
fn get_entry_mut<'a>(items: &'a mut [TocEntry], path: &[usize]) -> &'a mut TocEntry {
    assert!(!path.is_empty());
    let mut current = &mut items[path[0]];
    for &idx in &path[1..] {
        current = &mut current.children[idx];
    }
    current
}

/// Capitalise the first letter of a string.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}

/// Rewrite a single href value for directory-URL routing.
fn rewrite_href(href: &str, page_rel_path: Option<&str>) -> String {
    if href.is_empty() {
        return href.to_string();
    }

    // Skip absolute URLs and fragment-only links.
    if href.starts_with('#')
        || href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("mailto:")
        || href.starts_with("tel:")
    {
        return href.to_string();
    }

    // Split off fragment.
    let (path_and_query, frag) = match href.find('#') {
        Some(i) => (&href[..i], Some(&href[i + 1..])),
        None => (href, None),
    };

    // Split off query string.
    let (path_only, query) = match path_and_query.find('?') {
        Some(i) => (&path_and_query[..i], Some(&path_and_query[i + 1..])),
        None => (path_and_query, None),
    };

    // Convert extension to trailing slash.
    let mut converted = if let Some(stripped) = path_only.strip_suffix("/index.html") {
        format!("{stripped}/")
    } else if let Some(stripped) = path_only.strip_suffix(".html") {
        format!("{stripped}/")
    } else if let Some(stripped) = path_only.strip_suffix(".adoc") {
        format!("{stripped}/")
    } else {
        path_only.to_string()
    };

    // Resolve relative paths when page_rel_path is provided.
    if let Some(prp) = page_rel_path {
        if !converted.starts_with('/') {
            let src_dir = parent_path(prp);
            let had_trailing_slash = converted.ends_with('/');

            let joined = if src_dir.is_empty() {
                converted.clone()
            } else {
                format!("{src_dir}/{converted}")
            };
            let mut abs_path = normalize_path(&joined);
            if had_trailing_slash && !abs_path.ends_with('/') {
                abs_path.push('/');
            }

            let page_stem = file_stem(prp);
            let page_url_dir = if page_stem == "index" {
                src_dir.to_string()
            } else if src_dir.is_empty() {
                page_stem.to_string()
            } else {
                format!("{src_dir}/{page_stem}")
            };

            let abs_clean = abs_path.trim_end_matches('/');
            converted = relative_path(abs_clean, &page_url_dir);
            if had_trailing_slash && !converted.ends_with('/') {
                converted.push('/');
            }
        }
    }

    // Reassemble with query and fragment.
    let mut out = converted;
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = frag {
        out.push('#');
        out.push_str(f);
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn processor() -> HtmlProcessor {
        HtmlProcessor::new()
    }

    // -- Metadata tests -----------------------------------------------------

    #[test]
    fn test_extract_meta_title() {
        let html = r#"<h1 class="sect0">My Document Title</h1><p>body</p>"#;
        let proc = processor();
        let result = proc.process(html, None);
        assert_eq!(result.meta.title, Some("My Document Title".to_string()));
    }

    #[test]
    fn test_extract_meta_description() {
        let html = r#"<meta name="description" content="A great doc"><h2 id="intro">Intro</h2>"#;
        let proc = processor();
        let result = proc.process(html, None);
        assert_eq!(result.meta.description, Some("A great doc".to_string()));
    }

    // -- TOC tests ----------------------------------------------------------

    #[test]
    fn test_toc_from_headings() {
        let html = r#"
            <h2 id="chapter-1">Chapter 1</h2>
            <h3 id="section-1-1">Section 1.1</h3>
            <h3 id="section-1-2">Section 1.2</h3>
            <h2 id="chapter-2">Chapter 2</h2>
        "#;
        let proc = processor();
        let result = proc.process(html, None);

        assert_eq!(result.toc.len(), 2);
        assert_eq!(result.toc[0].title, "Chapter 1");
        assert_eq!(result.toc[0].id, "chapter-1");
        assert_eq!(result.toc[0].level, 2);
        assert_eq!(result.toc[0].children.len(), 2);
        assert_eq!(result.toc[0].children[0].title, "Section 1.1");
        assert_eq!(result.toc[0].children[0].id, "section-1-1");
        assert_eq!(result.toc[0].children[0].level, 3);
        assert_eq!(result.toc[0].children[1].title, "Section 1.2");
        assert_eq!(result.toc[1].title, "Chapter 2");
        assert_eq!(result.toc[1].children.len(), 0);
    }

    #[test]
    fn test_toc_generates_id_when_missing() {
        let html = r#"<h2>My Heading Without ID</h2>"#;
        let proc = processor();
        let result = proc.process(html, None);

        assert_eq!(result.toc.len(), 1);
        assert_eq!(result.toc[0].id, "my-heading-without-id");
        assert_eq!(result.toc[0].title, "My Heading Without ID");
    }

    #[test]
    fn test_sect0_excluded_from_toc() {
        let html = r#"
            <h1 class="sect0">Document Title</h1>
            <h2 id="intro">Introduction</h2>
        "#;
        let proc = processor();
        let result = proc.process(html, None);

        assert_eq!(result.toc.len(), 1);
        assert_eq!(result.toc[0].title, "Introduction");
    }

    // -- Slugify tests ------------------------------------------------------

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("  My Heading  "), "my-heading");
        assert_eq!(slugify("CamelCase Title"), "camelcase-title");
        assert_eq!(slugify("with_underscores"), "with-underscores");
        assert_eq!(slugify("special!@#chars"), "specialchars");
        assert_eq!(slugify("multiple   spaces"), "multiple-spaces");
        assert_eq!(slugify("already-slugged"), "already-slugged");
    }

    // -- Admonition tests ---------------------------------------------------

    #[test]
    fn test_transform_admonition_note() {
        let html = r#"<div class="admonitionblock note">
<table><tbody><tr><td class="icon"><div class="title">Note</div></td>
<td class="content">This is a note.</td></tr></tbody></table></div>"#;

        let proc = processor();
        let result = proc.process(html, None);

        assert!(
            result.html.contains("class=\"admonition note\""),
            "Expected 'admonition note' class, got: {}",
            result.html
        );
        assert!(result.html.contains("class=\"admonition-title\""));
        assert!(result.html.contains("Note"));
        assert!(result.html.contains("This is a note."));
        assert!(!result.html.contains("admonitionblock"));
    }

    #[test]
    fn test_transform_admonition_caution_maps_to_warning() {
        let html = r#"<div class="admonitionblock caution">
<table><tbody><tr><td class="icon"><div class="title">Caution</div></td>
<td class="content">Be careful!</td></tr></tbody></table></div>"#;

        let proc = processor();
        let result = proc.process(html, None);

        assert!(result.html.contains("class=\"admonition warning\""));
        assert!(result.html.contains("Caution"));
    }

    #[test]
    fn test_transform_admonition_important_maps_to_danger() {
        let html = r#"<div class="admonitionblock important">
<table><tbody><tr><td class="icon"><div class="title">Important</div></td>
<td class="content">Critical info!</td></tr></tbody></table></div>"#;

        let proc = processor();
        let result = proc.process(html, None);

        assert!(result.html.contains("class=\"admonition danger\""));
        assert!(result.html.contains("Important"));
    }

    // -- Callout tests ------------------------------------------------------

    #[test]
    fn test_transform_callout_list() {
        let html = r#"<div class="colist"><table>
<tr><td>1</td><td>First item</td></tr>
<tr><td>2</td><td>Second item</td></tr>
</table></div>"#;

        let proc = processor();
        let result = proc.process(html, None);

        assert!(
            result.html.contains("<ol class=\"colist\">"),
            "Expected ol.colist, got: {}",
            result.html
        );
        assert!(result.html.contains("<li>First item</li>"));
        assert!(result.html.contains("<li>Second item</li>"));
        assert!(result.html.contains("</ol>"));
        assert!(!result.html.contains("<table>"));
    }

    #[test]
    fn test_clean_callout_markers() {
        let html = r#"<span class="conum" data-value="1"></span>(1)
<span class="conum" data-value="2"></span> (2)"#;

        let proc = processor();
        let result = proc.process(html, None);

        assert!(!result.html.contains("(1)"));
        assert!(!result.html.contains("(2)"));
        assert!(result.html.contains("conum"));
    }

    // -- Code block tests ---------------------------------------------------

    #[test]
    fn test_transform_code_block_rouge_to_zensical() {
        let html = r#"<pre class="rouge highlight"><code data-lang="rust"><span class="k">use</span> <span class="nn">std</span>;</code></pre>"#;
        let proc = processor();
        let result = proc.process(html, None);

        assert!(
            result.html.contains(r#"class="language-rust highlight""#),
            "Expected language-rust wrapper, got: {}",
            result.html
        );
        assert!(
            !result.html.contains(r#"class="rouge highlight""#),
            "Rouge wrapper should be removed"
        );
        assert!(
            result.html.contains(r#"<span class="k">use</span>"#),
            "Highlighted spans should be preserved"
        );
    }

    #[test]
    fn test_transform_code_block_preserves_non_rouge() {
        let html = r#"<pre><code>plain code</code></pre>"#;
        let proc = processor();
        let result = proc.process(html, None);

        assert!(
            result.html.contains("<pre><code>plain code</code></pre>"),
            "Non-Rouge code blocks should be unchanged"
        );
    }

    // -- Table tests --------------------------------------------------------

    #[test]
    fn test_transform_table_wraps_with_typeset() {
        let html = r#"<table class="tableblock"><tr><td>cell</td></tr></table>"#;

        let proc = processor();
        let result = proc.process(html, None);

        assert!(
            result.html.contains("class=\"md-typeset__table\""),
            "Expected md-typeset__table wrapper, got: {}",
            result.html
        );
        assert!(
            !result.html.contains("tableblock"),
            "Asciidoctor tableblock classes should be stripped"
        );
    }

    #[test]
    fn test_transform_table_moves_title_to_caption() {
        let html = r#"<div class="tableblock">
<div class="title">Table 1. My Table</div>
<table class="tableblock"><tr><td>data</td></tr></table>
</div>"#;

        let proc = processor();
        let result = proc.process(html, None);

        assert!(
            result.html.contains("<caption>Table 1. My Table</caption>"),
            "Expected caption, got: {}",
            result.html
        );
        assert!(result.html.contains("md-typeset__table"));
        assert!(
            !result
                .html
                .contains(r#"<div class="title">Table 1. My Table</div>"#)
        );
    }

    // -- Figure tests -------------------------------------------------------

    #[test]
    fn test_transform_figure() {
        let html = r#"<div class="imageblock">
<div class="content"><img src="image.png" alt="photo"></div>
<div class="title">Figure 1. A nice photo</div>
</div>"#;

        let proc = processor();
        let result = proc.process(html, None);

        assert!(
            result.html.contains("<figure class=\"adoc-figure\">"),
            "Expected figure.adoc-figure, got: {}",
            result.html
        );
        assert!(
            result
                .html
                .contains("<figcaption>Figure 1. A nice photo</figcaption>")
        );
        assert!(result.html.contains("<img"));
        assert!(!result.html.contains("imageblock"));
    }

    #[test]
    fn test_transform_figure_without_title() {
        let html = r#"<div class="imageblock">
<div class="content"><img src="image.png" alt="photo"></div>
</div>"#;

        let proc = processor();
        let result = proc.process(html, None);

        assert!(
            result.html.contains("<figure class=\"adoc-figure\">"),
            "Expected figure.adoc-figure, got: {}",
            result.html
        );
        assert!(!result.html.contains("<figcaption>"));
        assert!(result.html.contains("<img"));
    }

    // -- Xref URL tests -----------------------------------------------------

    #[test]
    fn test_xref_adoc_to_trailing_slash() {
        let html = r#"<a href="page.adoc">Link</a>"#;
        let proc = processor();
        let result = proc.process(html, None);

        assert!(result.html.contains(r#"href="page/""#));
    }

    #[test]
    fn test_xref_preserves_fragment() {
        let html = r#"<a href="page.adoc#section">Link</a>"#;
        let proc = processor();
        let result = proc.process(html, None);

        assert!(result.html.contains(r#"href="page/#section""#));
    }

    #[test]
    fn test_xref_absolute_url_unchanged() {
        let html = r#"<a href="https://example.com/page">Link</a>"#;
        let proc = processor();
        let result = proc.process(html, None);

        assert!(result.html.contains(r#"href="https://example.com/page""#));
    }

    #[test]
    fn test_xref_fragment_only_unchanged() {
        let html = r##"<a href="#section">Link</a>"##;
        let proc = processor();
        let result = proc.process(html, None);

        assert!(result.html.contains(r##"href="#section""##));
    }

    #[test]
    fn test_xref_html_extension_to_trailing_slash() {
        let html = r#"<a href="other.html">Link</a>"#;
        let proc = processor();
        let result = proc.process(html, None);

        assert!(result.html.contains(r#"href="other/""#));
    }

    // -- Helper function tests ----------------------------------------------

    #[test]
    fn test_parent_path() {
        assert_eq!(parent_path("a/b/c.txt"), "a/b");
        assert_eq!(parent_path("file.txt"), "");
        assert_eq!(parent_path(""), "");
    }

    #[test]
    fn test_file_stem() {
        assert_eq!(file_stem("a/b/page.adoc"), "page");
        assert_eq!(file_stem("index.html"), "index");
        assert_eq!(file_stem("noext"), "noext");
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("a/b/../c"), "a/c");
        assert_eq!(normalize_path("a/./b/c"), "a/b/c");
        assert_eq!(normalize_path("a/b/../../c"), "c");
        assert_eq!(normalize_path("a/b/c"), "a/b/c");
    }

    #[test]
    fn test_relative_path() {
        assert_eq!(relative_path("a/b", "a"), "b");
        assert_eq!(relative_path("a/b", "c"), "../a/b");
        assert_eq!(relative_path("a", "a/b"), "..");
        assert_eq!(relative_path("x", ""), "x");
    }
}
