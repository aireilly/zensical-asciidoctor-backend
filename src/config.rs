//! Configuration for the `AsciiDoc` module.

use std::collections::HashMap;
use std::fmt;

/// Configuration for the `AsciiDoc` module.
#[derive(Clone, Debug)]
pub struct Config {
    /// Path to asciidoctor binary.
    pub asciidoctor_cmd: String,
    /// Safe mode for asciidoctor.
    pub safe_mode: SafeMode,
    /// Asciidoctor attributes passed as `-a key=value`.
    pub attributes: HashMap<String, String>,
    /// Ruby libraries to require via `-r`.
    pub requires: Vec<String>,
    /// Whether to fail the build on render errors.
    pub fail_on_error: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            asciidoctor_cmd: String::from("asciidoctor"),
            safe_mode: SafeMode::default(),
            attributes: HashMap::from([
                (String::from("showtitle"), String::new()),
                (String::from("sectanchors"), String::new()),
                (String::from("source-highlighter"), String::from("rouge")),
            ]),
            requires: Vec::new(),
            fail_on_error: true,
        }
    }
}

/// Asciidoctor safe mode.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SafeMode {
    Unsafe,
    #[default]
    Safe,
    Server,
    Secure,
}

impl fmt::Display for SafeMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SafeMode::Unsafe => write!(f, "unsafe"),
            SafeMode::Safe => write!(f, "safe"),
            SafeMode::Server => write!(f, "server"),
            SafeMode::Secure => write!(f, "secure"),
        }
    }
}
