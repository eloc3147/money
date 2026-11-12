//! Structured, colorless error reports.
//!
//! Errors leaving background work (such as the [`loader`](crate::loader) thread) are converted
//! into an [`ErrorReport`]: plain data holding the message, the chain of causes, the source
//! location where the error was raised and the stack frames. Nothing in it contains terminal
//! color codes, so it can be displayed by the GUI, logged, or serialized as-is.
//!
//! Locations and stack frames are captured by the [`eyre`] hook installed by [`install_hook`].
//! Reports created by another handler still convert, just without a location or frames.

use std::fmt;
use std::panic::Location;

use color_eyre::eyre::{self, EyreHandler};
use serde::{Deserialize, Serialize};

/// Crate name prefix of application functions in stack frames.
const APP_PREFIX: &str = "money::";

/// Eyre handler which records where an error was raised and an (unresolved) backtrace.
struct Handler {
    /// Where the report was constructed.
    location: Option<&'static Location<'static>>,
    /// Stack at construction, resolved lazily when the report is converted.
    backtrace: backtrace::Backtrace,
}

impl EyreHandler for Handler {
    fn debug(
        &self,
        error: &(dyn std::error::Error + 'static),
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        let report = ErrorReport::from_parts(error, self.location, Some(&self.backtrace));
        fmt::Display::fmt(&report, f)
    }

    fn track_caller(&mut self, location: &'static Location<'static>) {
        self.location = Some(location);
    }
}

/// Install the plain-text eyre hook (and color-eyre's panic hook).
///
/// Safe to call more than once; only the first call has an effect. Reports created before the
/// first call use eyre's default handler and convert without a location or frames.
pub fn install_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let (panic_hook, _) = color_eyre::config::HookBuilder::default().into_hooks();
        // Leave panics in tests to the test harness.
        if !cfg!(test) {
            panic_hook.install();
        }
        // Fails only if a hook was already set, in which case that one is kept.
        let _ = eyre::set_hook(Box::new(|_| {
            Box::new(Handler {
                location: None,
                backtrace: backtrace::Backtrace::new_unresolved(),
            })
        }));
    });
}

/// A position in a source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    /// File path as recorded by the compiler.
    pub file: String,
    /// 1-based line.
    pub line: u32,
    /// 1-based column.
    pub column: Option<u32>,
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file, self.line)?;
        if let Some(column) = self.column {
            write!(f, ":{column}")?;
        }
        Ok(())
    }
}

/// One stack frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    /// Demangled function name, without the hash suffix.
    pub function: String,
    /// Where the frame's code is, if debug info is available.
    pub location: Option<SourceLocation>,
}

impl Frame {
    /// Whether this frame is in application code (rather than std or a dependency).
    #[must_use]
    pub fn is_app(&self) -> bool {
        let name = self.function.trim_start_matches('<');
        name.starts_with(APP_PREFIX) && !name.starts_with("money::error::")
    }
}

/// A complete, colorless description of an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorReport {
    /// The outermost error message.
    pub message: String,
    /// Underlying causes, outermost first.
    pub causes: Vec<String>,
    /// Where the error was raised.
    pub location: Option<SourceLocation>,
    /// The stack when the error was raised, innermost first. Empty when not captured.
    pub frames: Vec<Frame>,
}

impl ErrorReport {
    /// A report holding only a message.
    #[must_use]
    pub fn msg(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            causes: Vec::new(),
            location: None,
            frames: Vec::new(),
        }
    }

    /// Build a report from an error chain plus optional location and backtrace.
    fn from_parts(
        error: &(dyn std::error::Error + 'static),
        location: Option<&'static Location<'static>>,
        backtrace: Option<&backtrace::Backtrace>,
    ) -> Self {
        let mut chain = std::iter::successors(Some(error), |e| e.source()).map(ToString::to_string);
        let message = chain.next().unwrap_or_default();
        Self {
            message,
            causes: chain.collect(),
            location: location.map(|l| SourceLocation {
                file: l.file().to_string(),
                line: l.line(),
                column: Some(l.column()),
            }),
            frames: backtrace.map(frames).unwrap_or_default(),
        }
    }

    /// Frames in application code.
    pub fn app_frames(&self) -> impl Iterator<Item = &Frame> {
        self.frames.iter().filter(|f| f.is_app())
    }
}

/// Resolve a backtrace into frames, dropping the frames of error construction itself.
fn frames(backtrace: &backtrace::Backtrace) -> Vec<Frame> {
    let mut backtrace = backtrace.clone();
    backtrace.resolve();
    let frames: Vec<Frame> = backtrace
        .frames()
        .iter()
        .flat_map(backtrace::BacktraceFrame::symbols)
        .map(|symbol| Frame {
            function: symbol
                .name()
                .map_or_else(|| "<unknown>".to_string(), |n| format!("{n:#}")),
            location: symbol
                .filename()
                .zip(symbol.lineno())
                .map(|(file, line)| SourceLocation {
                    file: file.display().to_string(),
                    line,
                    column: symbol.colno(),
                }),
        })
        .collect();
    // Leading frames up to the last one that builds the report (backtrace capture, eyre
    // conversion/wrapping, this module) are report construction, not where the error happened.
    let is_construction = |f: &Frame| {
        let name = f.function.trim_start_matches('<');
        name.starts_with("backtrace::")
            || name.starts_with("eyre::")
            || name.starts_with("money::error::")
            || name.contains(" as eyre::")
    };
    let start = frames
        .iter()
        .take_while(|f| !f.is_app() || is_construction(f))
        .enumerate()
        .filter(|(_, f)| is_construction(f))
        .last()
        .map_or(0, |(idx, _)| idx + 1);
    frames[start..].to_vec()
}

impl From<&eyre::Report> for ErrorReport {
    fn from(report: &eyre::Report) -> Self {
        let handler = report.handler().downcast_ref::<Handler>();
        Self::from_parts(
            report.as_ref(),
            handler.and_then(|h| h.location),
            handler.map(|h| &h.backtrace),
        )
    }
}

impl From<eyre::Report> for ErrorReport {
    fn from(report: eyre::Report) -> Self {
        Self::from(&report)
    }
}

impl fmt::Display for ErrorReport {
    /// Plain text: message, numbered causes, location and application frames.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if !self.causes.is_empty() {
            write!(f, "\n\nCaused by:")?;
            for (idx, cause) in self.causes.iter().enumerate() {
                write!(f, "\n  {idx}: {cause}")?;
            }
        }
        if let Some(location) = &self.location {
            write!(f, "\n\nLocation: {location}")?;
        }
        let mut app = self.app_frames().peekable();
        if app.peek().is_some() {
            write!(f, "\n\nStack (application frames):")?;
            for frame in app {
                write!(f, "\n  {}", frame.function)?;
                if let Some(location) = &frame.location {
                    write!(f, "\n      at {location}")?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use color_eyre::eyre::{WrapErr, eyre};

    use super::*;

    #[test]
    fn converts_chain_and_formats_plain_text() {
        // Location and frames depend on the global hook, which other unit tests in this binary
        // may beat us to; they are covered by `tests/error_report.rs`.
        let report = ErrorReport::from(
            Err::<(), _>(eyre!("inner problem"))
                .wrap_err("middle")
                .wrap_err("outer")
                .unwrap_err(),
        );
        assert_eq!(report.message, "outer");
        assert_eq!(report.causes, vec!["middle", "inner problem"]);
        let text = report.to_string();
        assert!(text.starts_with("outer\n\nCaused by:\n  0: middle\n  1: inner problem"));
        assert!(!text.contains('\u{1b}'), "no ANSI escapes");
    }

    #[test]
    fn plain_message_report() {
        let report = ErrorReport::msg("boom");
        assert_eq!(report.to_string(), "boom");
        assert_eq!(report.app_frames().count(), 0);
    }

    #[test]
    fn frame_classification() {
        let frame = |function: &str| Frame {
            function: function.into(),
            location: None,
        };
        assert!(frame("money::loader::load").is_app());
        assert!(frame("<money::Foo as Bar>::baz").is_app());
        assert!(!frame("money::error::frames").is_app());
        assert!(!frame("tokio::runtime::park").is_app());
        let location = SourceLocation {
            file: "a.rs".into(),
            line: 3,
            column: None,
        };
        assert_eq!(location.to_string(), "a.rs:3");
    }
}
