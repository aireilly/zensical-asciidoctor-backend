//! Renders the demo AsciiDoc files through the full pipeline.
//!
//! Usage: cargo run --example render_asciidoc
//!
//! This example processes all .adoc files in demo/docs/,
//! renders them through Asciidoctor + HTML post-processing, and
//! writes the output HTML to demo/site/ alongside the Zensical-built
//! Markdown pages.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use zensical_asciidoctor_backend::config::Config;
use zensical_asciidoctor_backend::html::{HtmlProcessor, TocEntry};
use zensical_asciidoctor_backend::renderer::Renderer;

/// A page we've rendered, ready for HTML generation.
struct RenderedPage {
    file_stem: String,
    title: String,
    toc: Vec<TocEntry>,
    content: String,
}

/// A nav entry for the left sidebar.
struct NavEntry {
    title: String,
    href: String,
    icon_svg: Option<String>,
    is_active: bool,
}

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let docs_dir = manifest_dir.join("demo/docs");
    let site_dir = manifest_dir.join("demo/site");

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
    let processor = HtmlProcessor::new();

    // Find all .adoc files in docs/
    let entries: Vec<_> = fs::read_dir(&docs_dir)
        .expect("failed to read docs directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "adoc"))
        .collect();

    println!("Rendering {} AsciiDoc files...\n", entries.len());

    // Render all pages
    let mut pages: Vec<RenderedPage> = Vec::new();
    for entry in &entries {
        let src_path = entry.path();
        let file_stem = src_path.file_stem().unwrap().to_string_lossy().to_string();
        let rel_path = format!("{}", src_path.file_name().unwrap().to_string_lossy());

        print!("  {} ... ", src_path.file_name().unwrap().to_string_lossy());

        let raw_html = match renderer.render(src_path.to_str().unwrap()) {
            Ok(html) => html,
            Err(err) => {
                println!("FAILED: {err}");
                continue;
            }
        };

        let processed = processor.process(&raw_html, Some(&rel_path));
        let title = processed
            .meta
            .title
            .unwrap_or_else(|| title_case(&file_stem));

        println!("OK (title: {title}, toc: {} entries)", processed.toc.len());

        pages.push(RenderedPage {
            file_stem,
            title,
            toc: processed.toc,
            content: processed.html,
        });
    }

    // Write HTML — pages go directly into site/<stem>/index.html
    for (i, page) in pages.iter().enumerate() {
        let full_html = build_full_page(page, &pages, i);

        let out_dir = site_dir.join(&page.file_stem);
        fs::create_dir_all(&out_dir).expect("failed to create output dir");
        let out_path = out_dir.join("index.html");
        fs::write(&out_path, &full_html).expect("failed to write HTML");

        // Copy images directory into each page's output directory so that
        // relative image paths (e.g. images/pipeline.svg) resolve correctly.
        let images_src = docs_dir.join("images");
        if images_src.is_dir() {
            let images_dst = out_dir.join("images");
            fs::create_dir_all(&images_dst).expect("failed to create images dir");
            for img in fs::read_dir(&images_src).unwrap().filter_map(|e| e.ok()) {
                let dest = images_dst.join(img.file_name());
                fs::copy(img.path(), dest).expect("failed to copy image");
            }
        }
    }

    // Append AsciiDoc pages to the search index
    update_search_index(&site_dir, &pages);

    // Update Markdown pages' navigation to include AsciiDoc pages
    update_markdown_nav(&site_dir, &pages);

    println!("\nDone! Output written to demo/site/");
}

fn title_case(s: &str) -> String {
    s.split(['_', '-'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build the complete HTML page matching Zensical's output exactly.
fn build_full_page(page: &RenderedPage, all_pages: &[RenderedPage], current_idx: usize) -> String {
    // AsciiDoc pages are at site/<stem>/, so assets are at ../
    let base_path = "..";
    let title = &page.title;

    // Build navigation entries — Markdown pages first, then AsciiDoc pages
    let mut nav_entries: Vec<NavEntry> = vec![
        NavEntry {
            title: "Get started".to_string(),
            href: format!("{base_path}/."),
            icon_svg: Some(ICON_ROCKET.to_string()),
            is_active: false,
        },
        NavEntry {
            title: "Markdown in 5min".to_string(),
            href: format!("{base_path}/markdown/"),
            icon_svg: Some(ICON_MARKDOWN.to_string()),
            is_active: false,
        },
    ];
    for (i, pg) in all_pages.iter().enumerate() {
        nav_entries.push(NavEntry {
            title: pg.title.clone(),
            href: format!("{base_path}/{}/", pg.file_stem),
            icon_svg: None,
            is_active: i == current_idx,
        });
    }

    let left_nav_html = build_left_nav(&nav_entries, &page.toc);
    let right_toc_html = build_right_toc(&page.toc);
    let footer_nav_html = build_footer_nav(all_pages, current_idx, base_path);

    format!(
        r##"<!doctype html>
<html lang="en" class="no-js">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width,initial-scale=1">
    <meta name="description" content="A new project generated from the default template project.">
    <meta name="author" content="<your name here>">
    <link rel="icon" href="{base_path}/assets/images/favicon.png">
    <meta name="generator" content="zensical-0.0.23">
    <title>{title} - Documentation</title>
    <link rel="stylesheet" href="{base_path}/assets/stylesheets/modern/main.1e989742.min.css">
    <link rel="stylesheet" href="{base_path}/assets/stylesheets/modern/palette.dfe2e883.min.css">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link rel="stylesheet" href="https://fonts.googleapis.com/css?family=Inter:300,300i,400,400i,500,500i,700,700i%7CJetBrains+Mono:400,400i,700,700i&display=fallback">
    <style>:root{{--md-text-font:"Inter";--md-code-font:"JetBrains Mono"}}</style>
    <script>__md_scope=new URL(".",location),__md_hash=e=>[...e].reduce(((e,t)=>(e<<5)-e+t.charCodeAt(0)),0),__md_get=(e,t=localStorage,a=__md_scope)=>JSON.parse(t.getItem(a.pathname+"."+e)),__md_set=(e,t,a=localStorage,_=__md_scope)=>{{try{{a.setItem(_.pathname+"."+e,JSON.stringify(t))}}catch(e){{}}}},document.documentElement.setAttribute("data-platform",navigator.platform)</script>
  </head>
  <body dir="ltr" data-md-color-scheme="default" data-md-color-primary="indigo" data-md-color-accent="indigo">
    <input class="md-toggle" data-md-toggle="drawer" type="checkbox" id="__drawer" autocomplete="off">
    <input class="md-toggle" data-md-toggle="search" type="checkbox" id="__search" autocomplete="off">
    <label class="md-overlay" for="__drawer"></label>
    <div data-md-component="skip">
      <a href="#content" class="md-skip">Skip to content</a>
    </div>
    <div data-md-component="announce"></div>

    <header class="md-header md-header--shadow" data-md-component="header">
      <nav class="md-header__inner md-grid" aria-label="Header">
        <a href="{base_path}/." title="Documentation" class="md-header__button md-logo" aria-label="Documentation" data-md-component="logo">
          {ICON_BOOK}
        </a>
        <label class="md-header__button md-icon" for="__drawer">
          {ICON_MENU}
        </label>
        <div class="md-header__title" data-md-component="header-title">
          <div class="md-header__ellipsis">
            <div class="md-header__topic">
              <span class="md-ellipsis">Documentation</span>
            </div>
            <div class="md-header__topic" data-md-component="header-topic">
              <span class="md-ellipsis">{title}</span>
            </div>
          </div>
        </div>
        <form class="md-header__option" data-md-component="palette">
          <input class="md-option" data-md-color-media="none" data-md-color-scheme="default" data-md-color-primary="indigo" data-md-color-accent="indigo" aria-label="Switch to dark mode" type="radio" name="__palette" id="__palette_0">
          <label class="md-header__button md-icon" title="Switch to dark mode" for="__palette_1" hidden>
            {ICON_SUN}
          </label>
          <input class="md-option" data-md-color-media="none" data-md-color-scheme="slate" data-md-color-primary="indigo" data-md-color-accent="indigo" aria-label="Switch to light mode" type="radio" name="__palette" id="__palette_1">
          <label class="md-header__button md-icon" title="Switch to light mode" for="__palette_0" hidden>
            {ICON_MOON}
          </label>
        </form>
        <script>var palette=__md_get("__palette");if(palette&&palette.color){{if("(prefers-color-scheme)"===palette.color.media){{var media=matchMedia("(prefers-color-scheme: light)"),input=document.querySelector(media.matches?"[data-md-color-media='(prefers-color-scheme: light)']":"[data-md-color-media='(prefers-color-scheme: dark)']");palette.color.media=input.getAttribute("data-md-color-media"),palette.color.scheme=input.getAttribute("data-md-color-scheme"),palette.color.primary=input.getAttribute("data-md-color-primary"),palette.color.accent=input.getAttribute("data-md-color-accent")}}for(var[key,value]of Object.entries(palette.color))document.body.setAttribute("data-md-color-"+key,value)}}</script>
        <label class="md-header__button md-icon" for="__search">
          {ICON_SEARCH}
        </label>
        <div class="md-search" data-md-component="search" role="dialog" aria-label="Search">
          <button type="button" class="md-search__button">Search</button>
        </div>
        <div class="md-header__source"></div>
      </nav>
    </header>

    <div class="md-container" data-md-component="container">
      <main class="md-main" data-md-component="main">
        <div class="md-main__inner md-grid">
          <div class="md-sidebar md-sidebar--primary" data-md-component="sidebar" data-md-type="navigation">
            <div class="md-sidebar__scrollwrap">
              <div class="md-sidebar__inner">
{left_nav_html}
              </div>
            </div>
          </div>

          <div class="md-sidebar md-sidebar--secondary" data-md-component="sidebar" data-md-type="toc">
            <div class="md-sidebar__scrollwrap">
              <div class="md-sidebar__inner">
                <nav class="md-nav md-nav--secondary" aria-label="On this page">
                  <label class="md-nav__title" for="__toc">
                    <span class="md-nav__icon md-icon"></span>
                    On this page
                  </label>
                  <ul class="md-nav__list" data-md-component="toc" data-md-scrollfix>
{right_toc_html}
                  </ul>
                </nav>
              </div>
            </div>
          </div>

          <div class="md-content" data-md-component="content">
            <article class="md-content__inner md-typeset" id="content">
{content}
            </article>
          </div>
        </div>

        <button type="button" class="md-top md-icon" data-md-component="top" hidden>
          {ICON_ARROW_UP}
          Back to top
        </button>
      </main>

      <footer class="md-footer">
{footer_nav_html}
        <div class="md-footer-meta md-typeset">
          <div class="md-footer-meta__inner md-grid">
            <div class="md-copyright">
              <div class="md-copyright__highlight">
                Copyright &copy; 2026 The authors
              </div>
              Made with
              <a href="https://zensical.org/" target="_blank" rel="noopener">Zensical</a>
            </div>
          </div>
        </div>
      </footer>
    </div>

    <div class="md-dialog" data-md-component="dialog">
      <div class="md-dialog__inner md-typeset"></div>
    </div>

    <script id="__config" type="application/json">{{"annotate":null,"base":"{base_path}","features":["announce.dismiss","content.code.annotate","content.code.copy","content.code.select","content.footnote.tooltips","content.tabs.link","content.tooltips","navigation.footer","navigation.indexes","navigation.instant","navigation.instant.prefetch","navigation.path","navigation.sections","navigation.top","navigation.tracking","search.highlight"],"search":"{base_path}/assets/javascripts/workers/search.e2d2d235.min.js","tags":null,"translations":{{"clipboard.copied":"Copied to clipboard","clipboard.copy":"Copy to clipboard","search.result.more.one":"1 more on this page","search.result.more.other":"# more on this page","search.result.none":"No matching documents","search.result.one":"1 matching document","search.result.other":"# matching documents","search.result.placeholder":"Type to start searching","search.result.term.missing":"Missing","select.version":"Select version"}},"version":null}}</script>
    <script src="{base_path}/assets/javascripts/bundle.5fcf0de6.min.js"></script>
  </body>
</html>"##,
        title = title,
        base_path = base_path,
        left_nav_html = left_nav_html,
        right_toc_html = right_toc_html,
        content = page.content,
        footer_nav_html = footer_nav_html,
        ICON_BOOK = ICON_BOOK,
        ICON_MENU = ICON_MENU,
        ICON_SUN = ICON_SUN,
        ICON_MOON = ICON_MOON,
        ICON_SEARCH = ICON_SEARCH,
        ICON_ARROW_UP = ICON_ARROW_UP,
    )
}

/// Build the left sidebar navigation matching Zensical's structure exactly.
fn build_left_nav(nav_entries: &[NavEntry], current_toc: &[TocEntry]) -> String {
    let mut html = String::new();
    html.push_str(r#"<nav class="md-nav md-nav--primary" aria-label="Navigation" data-md-level="0">
                  <label class="md-nav__title" for="__drawer">
                    <a href="../." title="Documentation" class="md-nav__button md-logo" aria-label="Documentation" data-md-component="logo">
"#);
    html.push_str("                      ");
    html.push_str(ICON_BOOK);
    html.push('\n');
    html.push_str(
        r#"                    </a>
                    Documentation
                  </label>
                  <ul class="md-nav__list" data-md-scrollfix>
"#,
    );

    for entry in nav_entries {
        if entry.is_active {
            // Active page gets the inline TOC
            html.push_str(
                r#"                    <li class="md-nav__item md-nav__item--active">
                      <input class="md-nav__toggle md-toggle" type="checkbox" id="__toc">
                      <label class="md-nav__link md-nav__link--active" for="__toc">
"#,
            );
            if let Some(svg) = &entry.icon_svg {
                html.push_str("                        ");
                html.push_str(svg);
                html.push('\n');
            }
            html.push_str(&format!(
                r#"                        <span class="md-ellipsis">{title}</span>
                        <span class="md-nav__icon md-icon"></span>
                      </label>
                      <a href="{href}" class="md-nav__link md-nav__link--active">
"#,
                title = entry.title,
                href = entry.href,
            ));
            if let Some(svg) = &entry.icon_svg {
                html.push_str("                        ");
                html.push_str(svg);
                html.push('\n');
            }
            html.push_str(&format!(
                r#"                        <span class="md-ellipsis">{title}</span>
                      </a>
"#,
                title = entry.title,
            ));

            // Inline TOC for the active page
            if !current_toc.is_empty() {
                html.push_str(r#"                      <nav class="md-nav md-nav--secondary" aria-label="On this page">
                        <label class="md-nav__title" for="__toc">
                          <span class="md-nav__icon md-icon"></span>
                          On this page
                        </label>
                        <ul class="md-nav__list" data-md-component="toc" data-md-scrollfix>
"#);
                build_toc_items(current_toc, &mut html, 26);
                html.push_str(
                    r#"                        </ul>
                      </nav>
"#,
                );
            }

            html.push_str("                    </li>\n");
        } else {
            // Non-active page
            html.push_str(&format!(
                r#"                    <li class="md-nav__item">
                      <a href="{href}" class="md-nav__link">
"#,
                href = entry.href,
            ));
            if let Some(svg) = &entry.icon_svg {
                html.push_str("                        ");
                html.push_str(svg);
                html.push('\n');
            }
            html.push_str(&format!(
                r#"                        <span class="md-ellipsis">{title}</span>
                      </a>
                    </li>
"#,
                title = entry.title,
            ));
        }
    }

    html.push_str(
        r#"                  </ul>
                </nav>"#,
    );
    html
}

/// Build right-side "On this page" TOC using Material nav structure.
fn build_right_toc(entries: &[TocEntry]) -> String {
    let mut html = String::new();
    build_toc_items(entries, &mut html, 20);
    html
}

fn build_toc_items(entries: &[TocEntry], html: &mut String, indent: usize) {
    let pad = " ".repeat(indent);
    for entry in entries {
        let href = format!("#{}", entry.id);
        if entry.children.is_empty() {
            html.push_str(&format!(
                "{pad}<li class=\"md-nav__item\">\n\
                 {pad}  <a href=\"{href}\" class=\"md-nav__link\">\n\
                 {pad}    <span class=\"md-ellipsis\">{title}</span>\n\
                 {pad}  </a>\n\
                 {pad}</li>\n",
                pad = pad,
                href = href,
                title = entry.title,
            ));
        } else {
            html.push_str(&format!(
                "{pad}<li class=\"md-nav__item\">\n\
                 {pad}  <a href=\"{href}\" class=\"md-nav__link\">\n\
                 {pad}    <span class=\"md-ellipsis\">{title}</span>\n\
                 {pad}  </a>\n\
                 {pad}  <nav class=\"md-nav\" aria-label=\"{title}\">\n\
                 {pad}    <ul class=\"md-nav__list\">\n",
                pad = pad,
                href = href,
                title = entry.title,
            ));
            build_toc_items(&entry.children, html, indent + 6);
            html.push_str(&format!(
                "{pad}    </ul>\n\
                 {pad}  </nav>\n\
                 {pad}</li>\n",
                pad = pad,
            ));
        }
    }
}

/// Build footer prev/next navigation.
fn build_footer_nav(all_pages: &[RenderedPage], current_idx: usize, base_path: &str) -> String {
    let prev = if current_idx > 0 {
        Some(&all_pages[current_idx - 1])
    } else {
        None
    };
    let next = if current_idx + 1 < all_pages.len() {
        Some(&all_pages[current_idx + 1])
    } else {
        None
    };

    if prev.is_none() && next.is_none() {
        return String::new();
    }

    let mut html =
        String::from("        <nav class=\"md-footer__inner md-grid\" aria-label=\"Footer\">\n");

    if let Some(pg) = prev {
        html.push_str(&format!(
            r#"          <a href="{base_path}/{stem}/" class="md-footer__link md-footer__link--prev" aria-label="Previous: {title}">
            <div class="md-footer__button md-icon">
              {ICON_ARROW_LEFT}
            </div>
            <div class="md-footer__title">
              <span class="md-footer__direction">Previous</span>
              <div class="md-ellipsis">{title}</div>
            </div>
          </a>
"#,
            base_path = base_path,
            stem = pg.file_stem,
            title = pg.title,
            ICON_ARROW_LEFT = ICON_ARROW_LEFT,
        ));
    }

    if let Some(pg) = next {
        html.push_str(&format!(
            r#"          <a href="{base_path}/{stem}/" class="md-footer__link md-footer__link--next" aria-label="Next: {title}">
            <div class="md-footer__title">
              <span class="md-footer__direction">Next</span>
              <div class="md-ellipsis">{title}</div>
            </div>
            <div class="md-footer__button md-icon">
              {ICON_ARROW_RIGHT}
            </div>
          </a>
"#,
            base_path = base_path,
            stem = pg.file_stem,
            title = pg.title,
            ICON_ARROW_RIGHT = ICON_ARROW_RIGHT,
        ));
    }

    html.push_str("        </nav>\n");
    html
}

/// Add AsciiDoc pages to the Zensical search index.
fn update_search_index(site_dir: &Path, pages: &[RenderedPage]) {
    let search_path = site_dir.join("search.json");
    let Ok(content) = fs::read_to_string(&search_path) else {
        println!("  Warning: search.json not found, skipping search index update");
        return;
    };

    // Simple JSON manipulation — parse, add items, write back
    // The search index has {"config": {...}, "items": [...]}
    let Ok(mut search): Result<serde_json::Value, _> = serde_json::from_str(&content) else {
        println!("  Warning: could not parse search.json");
        return;
    };

    let items = search["items"].as_array_mut().unwrap();

    // Remove any previously added AsciiDoc entries (re-run safe)
    items.retain(|item| {
        let loc = item["location"].as_str().unwrap_or("");
        !pages
            .iter()
            .any(|pg| loc.starts_with(&format!("{}/", pg.file_stem)))
    });

    // Add entries for each AsciiDoc page
    for page in pages {
        // Page-level entry
        items.push(serde_json::json!({
            "location": format!("{}/", page.file_stem),
            "level": 1,
            "title": page.title,
            "text": "",
            "path": [page.title],
            "tags": []
        }));

        // Section entries from TOC
        add_toc_search_entries(items, &page.toc, &page.file_stem, &page.title);
    }

    let json = serde_json::to_string(&search).unwrap();
    fs::write(&search_path, json).expect("failed to write search.json");
    println!("  Updated search.json with AsciiDoc pages");
}

fn add_toc_search_entries(
    items: &mut Vec<serde_json::Value>,
    entries: &[TocEntry],
    file_stem: &str,
    page_title: &str,
) {
    for entry in entries {
        items.push(serde_json::json!({
            "location": format!("{}/#{}",  file_stem, entry.id),
            "level": entry.level,
            "title": entry.title,
            "text": "",
            "path": [page_title],
            "tags": []
        }));
        add_toc_search_entries(items, &entry.children, file_stem, page_title);
    }
}

/// Update the Markdown pages' navigation to include AsciiDoc page links.
fn update_markdown_nav(site_dir: &Path, pages: &[RenderedPage]) {
    // Find all existing HTML files generated by Zensical
    let md_pages = ["index.html", "markdown/index.html"];

    for md_page in &md_pages {
        let path = site_dir.join(md_page);
        let Ok(mut html) = fs::read_to_string(&path) else {
            continue;
        };

        // Add AsciiDoc nav entries before the closing </ul> of the primary nav
        // Find the last </ul> before </nav> in the primary nav
        let nav_entries: String = pages
            .iter()
            .map(|pg| {
                format!(
                    r#"
    <li class="md-nav__item">
      <a href="../{stem}/" class="md-nav__link">
        <span class="md-ellipsis">{title}</span>
      </a>
    </li>"#,
                    stem = pg.file_stem,
                    title = pg.title,
                )
            })
            .collect();

        // Insert before the closing </ul> of the nav list (data-md-scrollfix)
        // We look for the pattern that closes the nav list
        if let Some(pos) = html.find("</ul>\n</nav>\n                  </div>") {
            html.insert_str(pos, &nav_entries);
            fs::write(&path, &html).expect("failed to update markdown page nav");
            println!("  Updated nav in {md_page}");
        }
    }
}

// ---------------------------------------------------------------------------
// SVG Icons (matching Zensical's lucide icons exactly)
// ---------------------------------------------------------------------------

const ICON_BOOK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" class="lucide lucide-book-open" viewBox="0 0 24 24"><path d="M12 7v14M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z"/></svg>"#;

const ICON_MENU: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" class="lucide lucide-menu" viewBox="0 0 24 24"><path d="M4 5h16M4 12h16M4 19h16"/></svg>"#;

const ICON_SUN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" class="lucide lucide-sun" viewBox="0 0 24 24"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/></svg>"#;

const ICON_MOON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" class="lucide lucide-moon" viewBox="0 0 24 24"><path d="M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401"/></svg>"#;

const ICON_SEARCH: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" class="lucide lucide-search" viewBox="0 0 24 24"><path d="m21 21-4.34-4.34"/><circle cx="11" cy="11" r="8"/></svg>"#;

const ICON_ROCKET: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" class="lucide lucide-rocket" viewBox="0 0 24 24"><path d="M4.5 16.5c-1.5 1.26-2 5-2 5s3.74-.5 5-2c.71-.84.7-2.13-.09-2.91a2.18 2.18 0 0 0-2.91-.09M12 15l-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.4 22.4 0 0 1-4 2"/><path d="M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5"/></svg>"#;

const ICON_MARKDOWN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M22.27 19.385H1.73A1.73 1.73 0 0 1 0 17.655V6.345a1.73 1.73 0 0 1 1.73-1.73h20.54A1.73 1.73 0 0 1 24 6.345v11.308a1.73 1.73 0 0 1-1.73 1.731zM5.769 15.923v-4.5l2.308 2.885 2.307-2.885v4.5h2.308V8.078h-2.308l-2.307 2.885-2.308-2.885H3.46v7.847zM21.232 12h-2.309V8.077h-2.307V12h-2.308l3.461 4.039z"/></svg>"#;

const ICON_ARROW_UP: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" class="lucide lucide-circle-arrow-up" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"/><path d="m16 12-4-4-4 4M12 16V8"/></svg>"#;

const ICON_ARROW_LEFT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" class="lucide lucide-arrow-left" viewBox="0 0 24 24"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>"#;

const ICON_ARROW_RIGHT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" class="lucide lucide-arrow-right" viewBox="0 0 24 24"><path d="M5 12h14M12 5l7 7-7 7"/></svg>"#;
