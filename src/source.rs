//! Filesystem loading is separate from pure checking. Includes are expanded
//! statically, preserving original file locations for compiler diagnostics.
use crate::sexpr::Sexp;
use crate::{Error, ir::Span};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MAX_SOURCE: usize = 16 * 1024 * 1024;
struct Chunk {
    start: usize,
    end: usize,
    offset: usize,
    path: PathBuf,
    original: Arc<str>,
}
pub struct Source {
    text: String,
    path: PathBuf,
    chunks: Vec<Chunk>,
}
impl Source {
    /// Expanded immutable text; IR spans refer to this compilation unit.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Translate a checker diagnostic on expanded text back to its input file.
    pub fn diagnostic(&self, mut error: Error) -> Error {
        if error.diagnostic.primary.is_none() {
            error.diagnostic.message =
                format!("{}:{}", self.path.display(), error.diagnostic.message);
        }
        for label in error
            .diagnostic
            .primary
            .iter_mut()
            .chain(&mut error.diagnostic.related)
        {
            // Already-localized loader errors must not be mapped a second time.
            if label.location.path.is_some() {
                continue;
            }
            let offset = label.span.start;
            let chunk = self
                .chunks
                .iter()
                .find(|c| c.start <= offset && offset < c.end)
                .or_else(|| self.chunks.last().filter(|c| offset == c.end));
            if let Some(chunk) = chunk {
                let start = chunk.offset + offset - chunk.start;
                // A label may cross an include boundary: retain its contiguous
                // portion in this file rather than pointing into another file.
                let end = chunk.offset + label.span.end.min(chunk.end) - chunk.start;
                *label = crate::diagnostic::Label::new(
                    &chunk.original,
                    Span { start, end },
                    &label.message,
                );
                label.location.path = Some(chunk.path.clone());
            }
        }
        error
    }
    fn append(
        &mut self,
        text: &str,
        offset: usize,
        path: &Path,
        original: &Arc<str>,
    ) -> Result<(), Error> {
        if self.text.len() + text.len() > MAX_SOURCE {
            return Err(Error::message("expanded source exceeds 16 MiB".into()));
        }
        let start = self.text.len();
        self.text.push_str(text);
        self.chunks.push(Chunk {
            start,
            end: self.text.len(),
            offset,
            path: path.into(),
            original: original.clone(),
        });
        Ok(())
    }
}
fn includes<'a>(node: &'a Sexp, output: &mut Vec<&'a Sexp>) {
    if node.head() == Some("include") {
        output.push(node);
    } else if matches!(node.head(), Some("module" | "namespace")) {
        for child in node.list().unwrap() {
            includes(child, output);
        }
    }
}
fn expand(
    path: &Path,
    body: bool,
    stack: &mut Vec<PathBuf>,
    output: &mut Source,
) -> Result<(), Error> {
    let canonical = path
        .canonicalize()
        .map_err(|e| Error::message(format!("{}: {e}", path.display())))?;
    if stack.contains(&canonical) {
        return Err(Error::message(format!(
            "include cycle: {}",
            stack
                .iter()
                .chain([&canonical])
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(" -> ")
        )));
    }
    if stack.len() >= 64 {
        return Err(Error::message("include nesting limit exceeded".into()));
    }
    let original: Arc<str> = std::fs::read_to_string(&canonical)
        .map_err(|e| Error::message(format!("{}: {e}", canonical.display())))?
        .into();
    if original.len() > MAX_SOURCE {
        return Err(Error::message("source exceeds 16 MiB".into()));
    }
    let syntax = match crate::sexpr::parse(&original) {
        Ok(syntax) => syntax,
        Err(error) if body => return Err(error.with_path(&canonical)),
        Err(_) => return output.append(&original, 0, &canonical, &original), // Native WAT has its own reader.
    };
    if body && syntax.head() != Some("module") {
        return Err(Error::message(format!(
            "{}: included file must contain (module ...)",
            canonical.display()
        )));
    }
    let range = if body {
        Span {
            start: syntax.list().unwrap()[0].span.end,
            end: syntax.span.end - 1,
        }
    } else {
        Span {
            start: 0,
            end: original.len(),
        }
    };
    let mut nodes = Vec::new();
    includes(&syntax, &mut nodes);
    let mut cursor = range.start;
    stack.push(canonical.clone());
    for node in nodes {
        let parts = node.list().unwrap();
        let Some(relative) = parts
            .get(1)
            .and_then(Sexp::text)
            .filter(|_| parts.len() == 2)
        else {
            return Err(
                Error::at(&original, node.span, "expected (include \"relative.gwat\")")
                    .with_path(&canonical),
            );
        };
        if Path::new(relative).is_absolute() {
            return Err(
                Error::at(&original, node.span, "include path must be relative")
                    .with_path(&canonical),
            );
        }
        output.append(
            &original[cursor..node.span.start],
            cursor,
            &canonical,
            &original,
        )?;
        // Whitespace prevents tokens/comments on either side from merging.
        output.append("\n", node.span.start, &canonical, &original)?;
        expand(
            &canonical.parent().unwrap().join(relative),
            true,
            stack,
            output,
        )?;
        output.append("\n", node.span.start, &canonical, &original)?;
        cursor = node.span.end;
    }
    output.append(&original[cursor..range.end], cursor, &canonical, &original)?;
    stack.pop();
    Ok(())
}
/// Load a compilation unit. String-based check/compile never read files.
pub fn load(path: impl AsRef<Path>) -> Result<Source, Error> {
    let path = path.as_ref();
    let mut source = Source {
        text: String::new(),
        path: path.into(),
        chunks: Vec::new(),
    };
    expand(path, false, &mut Vec::new(), &mut source)?;
    Ok(source)
}
