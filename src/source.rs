//! Filesystem loading is separate from pure checking. Includes are expanded
//! statically, preserving original file locations for compiler diagnostics.
use crate::sexpr::Sexp;
use crate::{Error, ir::Span};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MAX_SOURCE: usize = 16 * 1024 * 1024;
struct Chunk {
    start: usize,
    end: usize,
    offset: usize,
    path: PathBuf,
    original: Arc<str>,
    generated: bool,
}
pub struct Source {
    text: String,
    path: PathBuf,
    chunks: Vec<Chunk>,
    migration: Option<crate::syntax::Migration>,
}
impl Source {
    /// A failed editor load still has a current root buffer to index. This
    /// constructor does not resolve imports or make the source compilable.
    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn unresolved(path: PathBuf, text: Arc<str>) -> Self {
        Self {
            text: text.to_string(),
            path: path.clone(),
            chunks: vec![Chunk {
                start: 0,
                end: text.len(),
                offset: 0,
                path,
                original: text,
                generated: false,
            }],
            migration: None,
        }
    }
    /// Expanded immutable text; IR spans refer to this compilation unit.
    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn original(&self, path: &Path) -> Option<&str> {
        self.chunks
            .iter()
            .find(|c| c.path == path)
            .map(|c| c.original.as_ref())
    }
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Files represented by this loaded snapshot, including linked modules.
    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.path.as_path()).chain(self.chunks.iter().map(|c| c.path.as_path()))
    }

    pub(crate) fn label(&self, span: Span, message: &str) -> Option<crate::diagnostic::Label> {
        if span.start > span.end
            || span.end > self.text.len()
            || !self.text.is_char_boundary(span.start)
            || !self.text.is_char_boundary(span.end)
        {
            return None;
        }
        self.diagnostic(Error::at(&self.text, span, message))
            .diagnostic
            .primary
    }

    /// Inverse source map for an immutable loaded snapshot. Generated wrappers
    /// are excluded; repeated includes may have more than one expanded offset.
    pub(crate) fn offsets(
        &self,
        path: Option<&Path>,
        line: usize,
        column: usize,
    ) -> Result<Vec<usize>, Error> {
        let error = || {
            Error::message("position is outside the loaded source files".into())
                .with_code("E_SOURCE_POSITION")
        };
        let path = if let Some(path) = path {
            let absolute = if path.is_absolute() {
                path.to_owned()
            } else {
                std::env::current_dir().map_err(|_| error())?.join(path)
            };
            if self.chunks.iter().any(|c| c.path == absolute) {
                absolute
            } else if let Ok(canonical) = path.canonicalize() {
                canonical
            } else {
                // The file may have disappeared while its parent (including
                // symlinks such as macOS /var) still identifies the snapshot.
                let name = path.file_name().ok_or_else(error)?;
                path.parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."))
                    .canonicalize()
                    .map_err(|_| error())?
                    .join(name)
            }
        } else {
            self.chunks.first().ok_or_else(error)?.path.clone()
        };
        let original = &self
            .chunks
            .iter()
            .find(|c| c.path == path)
            .ok_or_else(error)?
            .original;
        let row = original
            .split('\n')
            .nth(line.checked_sub(1).ok_or_else(error)?)
            .ok_or_else(error)?;
        let column = column.checked_sub(1).ok_or_else(error)?;
        let byte = row
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(row.len()))
            .nth(column)
            .ok_or_else(error)?;
        let start = original
            .split_inclusive('\n')
            .take(line - 1)
            .map(str::len)
            .sum::<usize>()
            + byte;
        let mut offsets = self
            .chunks
            .iter()
            .filter(|c| {
                c.path == path
                    && !c.generated
                    && c.offset <= start
                    && start < c.offset + c.end - c.start
            })
            .map(|c| c.start + start - c.offset)
            .collect::<Vec<_>>();
        if start == original.len()
            && let Some(c) = self
                .chunks
                .iter()
                .rev()
                .find(|c| c.path == path && !c.generated && c.offset + c.end - c.start == start)
        {
            offsets.push(c.end);
        }
        offsets.sort_unstable();
        offsets.dedup();
        // A valid coordinate can belong to an import/include that was removed.
        Ok(offsets)
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
            if let Some(migration) = &self.migration {
                let span = Span {
                    start: migration.original_offset(label.span.start),
                    end: migration.original_offset(label.span.end),
                };
                *label = crate::diagnostic::Label::new(migration.original(), span, &label.message);
            }
            let offset = label.span.start;
            let chunk = self
                .chunks
                .iter()
                .find(|c| c.start <= offset && offset < c.end)
                .or_else(|| self.chunks.last().filter(|c| offset == c.end));
            if let Some(chunk) = chunk {
                let start = (chunk.offset + offset - chunk.start).min(chunk.original.len());
                // A label may cross an include boundary: retain its contiguous
                // portion in this file rather than pointing into another file.
                let end = (chunk.offset + label.span.end.min(chunk.end) - chunk.start)
                    .min(chunk.original.len());
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
            generated: false,
        });
        Ok(())
    }
}
fn file_import(node: &Sexp) -> bool {
    matches!(node.head(), Some("import" | "%import" | "use" | "%use"))
        && node
            .list()
            .and_then(|p| p.get(1))
            .and_then(Sexp::text)
            .is_some()
        && node
            .list()
            .and_then(|p| p.get(2))
            .and_then(Sexp::text)
            .is_none()
}
fn includes<'a>(
    source: &str,
    node: &'a Sexp,
    output: &mut Vec<&'a Sexp>,
    legacy: bool,
) -> Result<(), Error> {
    if !legacy
        && matches!(
            node.head(),
            Some(
                "namespace"
                    | "%namespace"
                    | "%import"
                    | "%file.namespace"
                    | "%file.use"
                    | "%legacy.namespace"
            )
        )
    {
        return Err(Error::at(
            source,
            node.span,
            "use files as modules and select their exports with %use",
        )
        .with_code("E_WLT_EXTENSION"));
    }
    if !legacy && node.head() == Some("%use") {
        let path = node.list().and_then(|p| p.get(1)).and_then(Sexp::text);
        if !path.is_some_and(|path| {
            path.starts_with("wlt:") || path.starts_with("./") || path.starts_with("../")
        }) {
            return Err(Error::at(
                source,
                node.span,
                "%use requires a package name or an explicit relative path such as ./module.wlt",
            )
            .with_code("E_USE_PATH"));
        }
    }
    if !legacy
        && (node.head() == Some("include")
            || matches!(node.head(), Some("import" | "use")) && file_import(node))
    {
        return Err(
            Error::at(source, node.span, "file linking requires %include or %use")
                .with_code("E_WLT_PREFIX"),
        );
    }
    if (node.head() == Some("%use") || legacy && file_import(node))
        && node
            .list()
            .and_then(|p| p.get(1))
            .and_then(Sexp::text)
            .is_some_and(|s| s.starts_with("wlt:"))
    {
        return Ok(());
    }
    if matches!(node.head(), Some("include" | "%include")) || file_import(node) {
        output.push(node);
    } else if matches!(node.head(), Some("module" | "namespace" | "%namespace")) {
        for child in node.list().unwrap() {
            includes(source, child, output, legacy)?;
        }
    }
    Ok(())
}
#[derive(Default)]
struct Modules {
    names: std::collections::BTreeMap<PathBuf, String>,
    units: Vec<(String, Source)>,
    unavailable: BTreeMap<PathBuf, Vec<crate::binary::Unavailable>>,
    size: usize,
}
fn expand(
    path: &Path,
    body: bool,
    stack: &mut Vec<PathBuf>,
    output: &mut Source,
    modules: &mut Modules,
    legacy: bool,
    overlays: &BTreeMap<PathBuf, Arc<str>>,
) -> Result<(), Error> {
    let canonical =
        normalize_path(path).map_err(|e| Error::message(format!("{}: {e}", path.display())))?;
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
    let original: Arc<str> = if let Some(text) = overlays.get(&canonical) {
        text.clone()
    } else {
        std::fs::read_to_string(&canonical)
            .map_err(|e| Error::message(format!("{}: {e}", canonical.display())))?
            .into()
    };
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
    includes(&original, &syntax, &mut nodes, legacy).map_err(|e| e.with_path(&canonical))?;
    let mut cursor = range.start;
    stack.push(canonical.clone());
    for node in nodes {
        let parts = node.list().unwrap();
        let import = file_import(node);
        let Some(relative) = parts.get(1).and_then(Sexp::text).filter(|_| {
            if import {
                parts.len() >= 3
            } else {
                parts.len() == 2
            }
        }) else {
            return Err(Error::at(
                &original,
                node.span,
                if import {
                    "expected (import \"relative.wlt\" (func/type/tag $member [$alias])...)"
                } else {
                    "expected (include \"relative.wlt\")"
                },
            )
            .with_path(&canonical));
        };
        if relative.starts_with("wlt:") {
            return Err(Error::at(
                &original,
                node.span,
                "standard modules require %use, not %include",
            )
            .with_code("E_CORE_MODULE")
            .with_path(&canonical));
        }
        if Path::new(relative).is_absolute() {
            return Err(
                Error::at(&original, node.span, "include/import path must be relative")
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
        let target = canonical.parent().unwrap().join(relative);
        if import {
            let target = normalize_path(&target).map_err(|e| {
                Error::at(&original, node.span, format!("{}: {e}", target.display()))
                    .with_path(&canonical)
            })?;
            if stack.contains(&target) {
                return Err(
                    Error::at(&original, node.span, "include/import cycle").with_path(&canonical)
                );
            }
            let name = if let Some(name) = modules.names.get(&target) {
                name.clone()
            } else {
                let name = format!("$__file{}", modules.names.len());
                modules.names.insert(target.clone(), name.clone());
                let mut unit = Source {
                    text: String::new(),
                    path: target.clone(),
                    chunks: vec![],
                    migration: None,
                };
                if target.extension().is_some_and(|ext| ext == "wasm") {
                    let load = || -> Result<(String, Vec<crate::binary::Unavailable>), Error> {
                        let length = std::fs::metadata(&target)
                            .map_err(|e| Error::message(e.to_string()))?
                            .len();
                        if length > crate::binary::MAX_BINARY as u64 {
                            return Err(Error::message("Wasm binary exceeds 4 MiB".into()));
                        }
                        let bytes =
                            std::fs::read(&target).map_err(|e| Error::message(e.to_string()))?;
                        crate::binary::source(&bytes, &name)
                    };
                    let (generated, unavailable) = load().map_err(|e| {
                        Error::at(&original, node.span, &e.diagnostic().message)
                            .with_code("E_WASM_USE")
                            .with_path(&canonical)
                    })?;
                    modules.unavailable.insert(target.clone(), unavailable);
                    let original = Arc::from(format!("(module\n{generated})"));
                    unit.append(&generated, "(module\n".len(), &target, &original)?;
                    unit.chunks.last_mut().unwrap().generated = true;
                } else {
                    expand(&target, true, stack, &mut unit, modules, legacy, overlays)?;
                }
                modules.size += unit.text.len() + 64;
                if modules.size > MAX_SOURCE {
                    return Err(Error::message("expanded source exceeds 16 MiB".into()));
                }
                modules.units.push((name.clone(), unit));
                name
            };
            if let Some(unavailable) = modules.unavailable.get(&target) {
                for binding in &parts[2..] {
                    let selector = binding.list().and_then(|p| p.get(1)).and_then(Sexp::atom);
                    if let Some(export) = unavailable
                        .iter()
                        .find(|e| Some(e.selector.as_str()) == selector)
                    {
                        return Err(Error::at(
                            &original,
                            binding.span,
                            format!("{}: {}", export.selector, export.reason),
                        )
                        .with_code("E_WASM_USE")
                        .with_path(&canonical));
                    }
                }
            }
            output.append(
                &format!("(%file.use {name} "),
                node.span.start,
                &canonical,
                &original,
            )?;
            output.chunks.last_mut().unwrap().generated = true;
            let bindings = parts[2].span.start..parts.last().unwrap().span.end;
            output.append(
                &original[bindings.clone()],
                bindings.start,
                &canonical,
                &original,
            )?;
            output.append(")", node.span.end - 1, &canonical, &original)?;
        } else {
            if target.extension().is_some_and(|ext| ext == "wasm") {
                return Err(Error::at(
                    &original,
                    node.span,
                    "binary modules require %use, not %include",
                )
                .with_code("E_WASM_USE")
                .with_path(&canonical));
            }
            expand(&target, true, stack, output, modules, legacy, overlays)?;
        }
        output.append("\n", node.span.start, &canonical, &original)?;
        cursor = node.span.end;
    }
    output.append(&original[cursor..range.end], cursor, &canonical, &original)?;
    stack.pop();
    Ok(())
}
/// Load a compilation unit. String-based check/compile never read files.
pub fn load(path: impl AsRef<Path>) -> Result<Source, Error> {
    load_mode(path.as_ref(), false, &BTreeMap::new())
}

/// Internal harness entry for frozen historical evaluation inputs.
#[doc(hidden)]
pub fn load_legacy(path: impl AsRef<Path>) -> Result<Source, Error> {
    let mut source = load_mode(path.as_ref(), true, &BTreeMap::new())?;
    let migration =
        crate::syntax::convert_legacy(&source.text).map_err(|e| source.diagnostic(e))?;
    source.text = migration.text().into();
    source.migration = Some(migration);
    Ok(source)
}

/// Load immutable unsaved buffers through the ordinary static module resolver.
/// Keys may name files not yet written to disk. No source file is modified.
pub fn load_with_overlays(
    path: impl AsRef<Path>,
    overlays: &BTreeMap<PathBuf, Arc<str>>,
) -> Result<Source, Error> {
    let overlays = overlays
        .iter()
        .map(|(p, t)| normalize_path(p).map(|p| (p, t.clone())))
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(|e| Error::message(e.to_string()))?;
    load_mode(path.as_ref(), false, &overlays)
}

/// Read declarations from current editor buffers with missing closing parens.
/// This snapshot is used exclusively by unverified editor indexing.
#[cfg(not(target_family = "wasm"))]
pub(crate) fn load_editor(
    path: &Path,
    overlays: &BTreeMap<PathBuf, Arc<str>>,
) -> Result<Source, Error> {
    let mut repaired = overlays.clone();
    let mut originals = BTreeMap::new();
    for (path, text) in overlays {
        if crate::sexpr::parse(text).is_err()
            && let Some(closed) = crate::sexpr::editor_closed(text)
        {
            let path = normalize_path(path).map_err(|e| Error::message(e.to_string()))?;
            repaired.insert(path.clone(), Arc::from(closed));
            originals.insert(path, text.clone());
        }
    }
    let mut source = load_with_overlays(path, &repaired)?;
    for chunk in &mut source.chunks {
        if let Some(original) = originals.get(&chunk.path) {
            chunk.original = original.clone();
            chunk.offset = chunk.offset.min(original.len());
        }
    }
    Ok(source)
}

pub(crate) fn normalize_path(path: &Path) -> std::io::Result<PathBuf> {
    if let Ok(path) = path.canonicalize() {
        return Ok(path);
    }
    let path = std::path::absolute(path)?;
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            p => normalized.push(p.as_os_str()),
        }
    }
    if let Some(parent) = normalized.parent()
        && parent != normalized
        && let Some(name) = normalized.file_name()
    {
        return Ok(normalize_path(parent)?.join(name));
    }
    Ok(normalized)
}

fn load_mode(
    path: &Path,
    legacy: bool,
    overlays: &BTreeMap<PathBuf, Arc<str>>,
) -> Result<Source, Error> {
    let mut source = Source {
        text: String::new(),
        path: path.into(),
        chunks: Vec::new(),
        migration: None,
    };
    let mut modules = Modules::default();
    expand(
        path,
        false,
        &mut Vec::new(),
        &mut source,
        &mut modules,
        legacy,
        overlays,
    )?;
    if modules.units.is_empty() {
        return Ok(source);
    }
    // Reserve fresh, ordinary identifiers across every original file. Rewrite
    // only generated chunks, never strings, comments, or user identifiers.
    let mut atoms = std::collections::BTreeSet::new();
    let mut seen = std::collections::BTreeSet::new();
    for unit in std::iter::once(&source).chain(modules.units.iter().map(|(_, s)| s)) {
        for chunk in &unit.chunks {
            if !seen.insert(chunk.path.clone()) {
                continue;
            }
            let syntax = crate::sexpr::parse(&chunk.original)?;
            let mut pending = vec![&syntax];
            while let Some(node) = pending.pop() {
                if let Some(atom) = node.atom() {
                    atoms.insert(atom.to_owned());
                }
                if let Some(items) = node.list() {
                    pending.extend(items);
                }
            }
        }
    }
    let mut renames = Vec::new();
    let mut next = 0;
    for (name, _) in &mut modules.units {
        let fresh = loop {
            let name = format!("$wlt-file-{next}");
            next += 1;
            if !atoms
                .iter()
                .any(|atom| atom == &name || atom.starts_with(&format!("{name}.")))
            {
                break name;
            }
        };
        renames.push((name.clone(), fresh.clone()));
        *name = fresh;
    }
    renames.sort_by_key(|(old, _)| std::cmp::Reverse(old.len()));
    rename_generated(&mut source, &renames)?;
    for (_, unit) in &mut modules.units {
        rename_generated(unit, &renames)?;
    }
    let syntax = crate::sexpr::parse(&source.text)?;
    let end = syntax.span.end - 1;
    let mut linked = Source {
        text: String::new(),
        path: source.path.clone(),
        chunks: vec![],
        migration: None,
    };
    append_range(&mut linked, &source, 0..end)?;
    for (name, unit) in modules.units {
        let original = unit
            .chunks
            .first()
            .expect("module body source")
            .original
            .clone();
        linked.append(
            &format!("\n(%file.namespace {name}\n"),
            0,
            &unit.path,
            &original,
        )?;
        append_range(&mut linked, &unit, 0..unit.text.len())?;
        linked.append("\n)\n", 0, &unit.path, &original)?;
    }
    append_range(&mut linked, &source, end..source.text.len())?;
    Ok(linked)
}

fn append_range(
    output: &mut Source,
    input: &Source,
    range: std::ops::Range<usize>,
) -> Result<(), Error> {
    for chunk in &input.chunks {
        let start = chunk.start.max(range.start);
        let end = chunk.end.min(range.end);
        if start < end {
            output.append(
                &input.text[start..end],
                chunk.offset + start - chunk.start,
                &chunk.path,
                &chunk.original,
            )?;
            output.chunks.last_mut().unwrap().generated = chunk.generated;
        }
    }
    Ok(())
}

fn rename_generated(source: &mut Source, renames: &[(String, String)]) -> Result<(), Error> {
    let mut output = Source {
        text: String::new(),
        path: source.path.clone(),
        chunks: vec![],
        migration: None,
    };
    for chunk in &source.chunks {
        let mut text = source.text[chunk.start..chunk.end].to_owned();
        if chunk.generated {
            for (old, new) in renames {
                text = text.replace(old, new);
            }
        }
        output.append(&text, chunk.offset, &chunk.path, &chunk.original)?;
        output.chunks.last_mut().unwrap().generated = chunk.generated;
    }
    *source = output;
    Ok(())
}
