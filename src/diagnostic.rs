//! Structured checker diagnostics. Rendering and filesystem source mapping are
//! consumers of this data; checks never recover locations from rendered text.
use crate::ir::Span;
use serde::Serialize;
use std::{fmt, path::PathBuf};

#[derive(Clone, Debug, Serialize)]
pub struct Location {
    pub path: Option<PathBuf>,
    /// One-based line and Unicode scalar column, not UTF-8 byte offsets.
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Label {
    /// Half-open byte range, relative to `location.path` after source mapping,
    /// or to the string passed to check/compile before source mapping.
    pub span: Span,
    pub location: Location,
    pub message: String,
}

impl Label {
    pub(crate) fn new(source: &str, span: Span, message: impl ToString) -> Self {
        let prefix = &source[..span.start];
        Self {
            span,
            location: Location {
                path: None,
                line: prefix.bytes().filter(|&b| b == b'\n').count() + 1,
                column: prefix.rsplit('\n').next().unwrap().chars().count() + 1,
            },
            message: message.to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    /// Stable machine-readable category. E_CHECK is a general checker failure;
    /// E_TOOL covers failures without an extension source span.
    pub code: &'static str,
    pub message: String,
    pub primary: Option<Label>,
    pub related: Vec<Label>,
    pub notes: Vec<String>,
    pub help: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Error {
    pub(crate) diagnostic: Box<Diagnostic>,
}

impl Error {
    pub(crate) fn message(message: String) -> Self {
        Self {
            diagnostic: Box::new(Diagnostic {
                code: "E_TOOL",
                message,
                primary: None,
                related: vec![],
                notes: vec![],
                help: vec![],
            }),
        }
    }

    pub(crate) fn at(source: &str, span: Span, message: impl fmt::Display) -> Self {
        let mut error = Self::message(message.to_string());
        error.diagnostic.code = "E_CHECK";
        error.diagnostic.primary = Some(Label::new(source, span, ""));
        error
    }

    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub(crate) fn with_code(mut self, code: &'static str) -> Self {
        self.diagnostic.code = code;
        self
    }

    pub(crate) fn related(mut self, source: &str, span: Span, message: impl ToString) -> Self {
        self.diagnostic
            .related
            .push(Label::new(source, span, message));
        self
    }

    pub(crate) fn note(mut self, message: impl ToString) -> Self {
        self.diagnostic.notes.push(message.to_string());
        self
    }

    pub(crate) fn help(mut self, message: impl ToString) -> Self {
        self.diagnostic.help.push(message.to_string());
        self
    }

    /// An inner expression may already explain its own failed expectation.
    /// Do not attribute that error to an unrelated outer call/result bound.
    pub(crate) fn expected_at(self, source: &str, span: Span, message: impl ToString) -> Self {
        if self.diagnostic.code == "E_TYPE_MISMATCH" && self.diagnostic.related.is_empty() {
            self.related(source, span, message)
        } else {
            self
        }
    }

    pub(crate) fn with_path(mut self, path: &std::path::Path) -> Self {
        for label in self
            .diagnostic
            .primary
            .iter_mut()
            .chain(&mut self.diagnostic.related)
        {
            label.location.path = Some(path.into());
        }
        self
    }
}

fn location(f: &mut fmt::Formatter<'_>, label: &Label) -> fmt::Result {
    if let Some(path) = &label.location.path {
        write!(f, "{}:", path.display())?;
    }
    write!(f, "{}:{}", label.location.line, label.location.column)
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let diagnostic = &self.diagnostic;
        if let Some(primary) = &diagnostic.primary {
            location(f, primary)?;
            f.write_str(": ")?;
        }
        f.write_str(&diagnostic.message)?;
        for label in &diagnostic.related {
            f.write_str("\n  note: ")?;
            location(f, label)?;
            write!(f, ": {}", label.message)?;
        }
        for note in &diagnostic.notes {
            write!(f, "\n  note: {note}")?;
        }
        for help in &diagnostic.help {
            write!(f, "\n  help: {help}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}
