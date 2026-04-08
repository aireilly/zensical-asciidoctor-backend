//! Asciidoctor subprocess renderer.
//!
//! Builds CLI arguments from [`Config`] and executes the `asciidoctor`
//! binary, capturing the HTML output from stdout.

use std::process::Command;

use thiserror::Error;

use crate::config::Config;

/// Errors that can occur during rendering.
#[derive(Debug, Error)]
pub enum RendererError {
    /// The asciidoctor binary was not found on the system.
    #[error("asciidoctor not found: '{cmd}'. Install with: gem install asciidoctor")]
    NotFound { cmd: String },

    /// The asciidoctor process exited with a non-zero status.
    #[error("asciidoctor failed for {path}: {stderr}")]
    Failed { path: String, stderr: String },

    /// An I/O error occurred when spawning or communicating with the process.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Wraps configuration and renders AsciiDoc files via the `asciidoctor` CLI.
#[derive(Clone, Debug)]
pub struct Renderer {
    cmd: String,
    safe_mode: String,
    attributes: Vec<(String, String)>,
    requires: Vec<String>,
    fail_on_error: bool,
}

impl Renderer {
    /// Create a new `Renderer` from the given configuration.
    pub fn new(config: &Config) -> Self {
        let mut attributes: Vec<(String, String)> = config
            .attributes
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        // Sort for deterministic argument ordering in tests.
        attributes.sort_by(|a, b| a.0.cmp(&b.0));

        Self {
            cmd: config.asciidoctor_cmd.clone(),
            safe_mode: config.safe_mode.to_string(),
            attributes,
            requires: config.requires.clone(),
            fail_on_error: config.fail_on_error,
        }
    }

    /// Build the argument list for invoking asciidoctor on the given source file.
    ///
    /// The returned vector includes the command name as the first element,
    /// followed by all flags, options, and the file path.
    pub fn build_args(&self, src_path: &str) -> Vec<String> {
        let mut args = vec![
            self.cmd.clone(),
            String::from("-b"),
            String::from("html5"),
            String::from("-s"),
            String::from("-o"),
            String::from("-"),
            String::from("-S"),
            self.safe_mode.clone(),
        ];

        for lib in &self.requires {
            args.push(String::from("-r"));
            args.push(lib.clone());
        }

        for (key, value) in &self.attributes {
            args.push(String::from("-a"));
            if value.is_empty() {
                args.push(key.clone());
            } else {
                args.push(format!("{key}={value}"));
            }
        }

        args.push(src_path.to_string());
        args
    }

    /// Render the AsciiDoc file at `src_path` to HTML.
    ///
    /// On success, returns the HTML string produced by asciidoctor.
    ///
    /// When `fail_on_error` is false, errors are caught and returned as an
    /// inline HTML error block instead of propagating.
    pub fn render(&self, src_path: &str) -> Result<String, RendererError> {
        let args = self.build_args(src_path);

        let output = match Command::new(&args[0]).args(&args[1..]).output() {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if self.fail_on_error {
                    return Err(RendererError::NotFound {
                        cmd: self.cmd.clone(),
                    });
                }
                return Ok(format!(
                    "<div class=\"admonition failure\">\
                     <p class=\"admonition-title\">AsciiDoc Error</p>\
                     <p>asciidoctor not found: '{}'. \
                     Install with: <code>gem install asciidoctor</code></p>\
                     </div>",
                    html_escape(&self.cmd),
                ));
            }
            Err(e) => return Err(RendererError::Io(e)),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if self.fail_on_error {
                return Err(RendererError::Failed {
                    path: src_path.to_string(),
                    stderr,
                });
            }
            return Ok(format!(
                "<div class=\"admonition failure\">\
                 <p class=\"admonition-title\">AsciiDoc Error</p>\
                 <pre><code>{}</code></pre>\
                 </div>",
                html_escape(&stderr),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Escape special HTML characters in a string.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, SafeMode};
    use std::collections::HashMap;

    #[test]
    fn test_build_args_default_config() {
        let config = Config::default();
        let renderer = Renderer::new(&config);
        let args = renderer.build_args("test.adoc");

        assert_eq!(args[0], "asciidoctor");
        assert_eq!(args[1], "-b");
        assert_eq!(args[2], "html5");
        assert_eq!(args[3], "-s");
        assert_eq!(args[4], "-o");
        assert_eq!(args[5], "-");
        assert_eq!(args[6], "-S");
        assert_eq!(args[7], "safe");
        // The last argument must always be the file path.
        assert_eq!(args.last().unwrap(), "test.adoc");
    }

    #[test]
    fn test_build_args_custom_safe_mode() {
        let config = Config {
            safe_mode: SafeMode::Unsafe,
            ..Config::default()
        };
        let renderer = Renderer::new(&config);
        let args = renderer.build_args("test.adoc");

        assert_eq!(args[7], "unsafe");
    }

    #[test]
    fn test_build_args_with_attributes() {
        let config = Config {
            attributes: HashMap::from([(
                String::from("imagesdir"),
                String::from("images"),
            )]),
            ..Config::default()
        };
        let renderer = Renderer::new(&config);
        let args = renderer.build_args("test.adoc");

        assert!(
            args.windows(2)
                .any(|w| w[0] == "-a" && w[1] == "imagesdir=images"),
            "expected -a imagesdir=images in args: {args:?}"
        );
    }

    #[test]
    fn test_build_args_with_requires() {
        let config = Config {
            requires: vec![String::from("asciidoctor-diagram")],
            ..Config::default()
        };
        let renderer = Renderer::new(&config);
        let args = renderer.build_args("test.adoc");

        assert!(
            args.windows(2)
                .any(|w| w[0] == "-r" && w[1] == "asciidoctor-diagram"),
            "expected -r asciidoctor-diagram in args: {args:?}"
        );
    }

    #[test]
    fn test_build_args_empty_attribute_value() {
        let config = Config {
            attributes: HashMap::from([(String::from("showtitle"), String::new())]),
            ..Config::default()
        };
        let renderer = Renderer::new(&config);
        let args = renderer.build_args("test.adoc");

        assert!(
            args.windows(2)
                .any(|w| w[0] == "-a" && w[1] == "showtitle"),
            "expected -a showtitle (without =) in args: {args:?}"
        );
        // Make sure it does NOT contain "showtitle=".
        assert!(
            !args.iter().any(|a| a == "showtitle="),
            "empty-valued attribute must not produce 'showtitle='"
        );
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(
            html_escape("<script>alert(\"xss\")&</script>"),
            "&lt;script&gt;alert(&quot;xss&quot;)&amp;&lt;/script&gt;"
        );
    }
}
