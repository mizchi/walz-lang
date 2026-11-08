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
    /// Rewrite presentation only. Symbol identity, source spans and stable
    /// diagnostic codes never depend on these rendered names.
    pub(crate) fn rewrite_names(
        mut self,
        names: &std::collections::BTreeMap<String, String>,
    ) -> Self {
        let d = &mut self.diagnostic;
        d.message = display_names(&d.message, names);
        for label in d.primary.iter_mut().chain(&mut d.related) {
            label.message = display_names(&label.message, names);
        }
        for text in d.notes.iter_mut().chain(&mut d.help) {
            *text = display_names(text, names);
        }
        self
    }
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

fn display_names(text: &str, names: &std::collections::BTreeMap<String, String>) -> String {
    fn render(
        text: &str,
        names: &std::collections::BTreeMap<String, String>,
        active: &mut Vec<String>,
        out: &mut String,
        left: &mut usize,
        truncated: &mut bool,
    ) {
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if *left == 0 {
                *truncated = true;
                return;
            }
            let mut token = c.to_string();
            if c == '$' {
                while chars
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                {
                    token.push(chars.next().unwrap());
                }
                if let Some(replacement) = names.get(&token)
                    && active.len() < 64
                    && !active.contains(&token)
                {
                    active.push(token.clone());
                    render(replacement, names, active, out, left, truncated);
                    active.pop();
                    continue;
                }
            }
            for c in token.chars() {
                if c.len_utf8() > *left {
                    *left = 0;
                    *truncated = true;
                    return;
                }
                out.push(c);
                *left -= c.len_utf8();
            }
        }
    }
    if names.is_empty() {
        return text.into();
    }
    let mut out = String::new();
    let mut left = 4096;
    let mut truncated = false;
    render(
        text,
        names,
        &mut vec![],
        &mut out,
        &mut left,
        &mut truncated,
    );
    if truncated {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod name_tests {
    use super::display_names;
    use std::collections::BTreeMap;
    #[test]
    fn source_names_replace_whole_identifiers_and_bound_shared_expansion() {
        let mut names = BTreeMap::from([
            ("$A.mono1".into(), "$Box<i32>".into()),
            ("$A.mono10".into(), "$Box<i64>".into()),
        ]);
        assert_eq!(
            display_names("$A.mono1 / $A.mono10", &names),
            "$Box<i32> / $Box<i64>"
        );
        for i in 0..64 {
            names.insert(format!("$X{i}"), format!("$Pair<$X{}, $X{}>", i + 1, i + 1));
        }
        let text = display_names("$X0", &names);
        assert!(text.len() <= 4099);
        assert!(text.ends_with('…'));
        names.insert("$cycle".into(), "$cycle".into());
        assert_eq!(display_names("$cycle", &names), "$cycle");
    }
}
