//! Thin LSP state machine. Checked snapshots and source maps remain compiler-owned.
mod protocol;
use crate::{bindings::BindingPolicy, diagnostic::Label, inspect::Snapshot, source};
pub use protocol::serve_stdio;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Position {
    pub line: usize,
    pub character: usize,
}
pub fn byte_offset(text: &str, position: Position) -> Option<usize> {
    let mut start = 0;
    for (line, row) in text.split('\n').enumerate() {
        if line == position.line {
            let row = row.strip_suffix('\r').unwrap_or(row);
            let mut units = 0;
            for (byte, c) in row.char_indices() {
                if units == position.character {
                    return Some(start + byte);
                }
                units += c.len_utf16();
                if units > position.character {
                    return None;
                }
            }
            return (units == position.character).then_some(start + row.len());
        }
        start += row.len() + 1;
    }
    None
}
fn position(text: &str, byte: usize) -> Position {
    let mut end = byte.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let before = &text[..end];
    Position {
        line: before.bytes().filter(|&b| b == b'\n').count(),
        character: before
            .rsplit('\n')
            .next()
            .unwrap_or("")
            .encode_utf16()
            .count(),
    }
}
pub fn file_uri(path: &Path) -> Result<String, String> {
    let path = source::normalize_path(path).map_err(|e| e.to_string())?;
    let path = path.to_str().ok_or("path must be UTF-8")?;
    let mut uri = "file://".to_owned();
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~:".contains(&b) {
            uri.push(char::from(b))
        } else {
            uri += &format!("%{b:02X}");
        }
    }
    Ok(uri)
}
fn uri_path(uri: &str) -> Result<PathBuf, String> {
    let tail = uri
        .strip_prefix("file://")
        .ok_or("only file: URIs are supported")?;
    let tail = tail
        .strip_prefix("localhost/")
        .map(|s| format!("/{s}"))
        .unwrap_or_else(|| tail.into());
    if !tail.starts_with('/') || tail.contains(['?', '#']) {
        return Err("invalid file URI".into());
    }
    let mut bytes = Vec::new();
    let mut iter = tail.bytes();
    while let Some(b) = iter.next() {
        if b == b'%' {
            let a = iter
                .next()
                .and_then(|b| char::from(b).to_digit(16))
                .ok_or("invalid URI escape")?;
            let b = iter
                .next()
                .and_then(|b| char::from(b).to_digit(16))
                .ok_or("invalid URI escape")?;
            bytes.push((a * 16 + b) as u8);
        } else {
            bytes.push(b)
        }
    }
    let path = String::from_utf8(bytes).map_err(|_| "URI must be UTF-8")?;
    if path.contains('\0') {
        return Err("invalid file URI".into());
    }
    source::normalize_path(Path::new(&path)).map_err(|e| e.to_string())
}
struct Document {
    uri: String,
    version: i64,
    text: std::sync::Arc<str>,
}
struct Cached {
    used: usize,
    snapshot: Result<Snapshot, crate::Error>,
}
pub struct Session {
    policy: BindingPolicy,
    documents: BTreeMap<PathBuf, Document>,
    cache: BTreeMap<PathBuf, Cached>,
    clock: usize,
    analyses: usize,
    pub(crate) shutdown: bool,
}
impl Session {
    pub fn new(policy: BindingPolicy) -> Self {
        Self {
            policy,
            documents: BTreeMap::new(),
            cache: BTreeMap::new(),
            analyses: 0,
            clock: 0,
            shutdown: false,
        }
    }
    pub fn analysis_count(&self) -> usize {
        self.analyses
    }
    fn snapshot(&mut self, path: &Path) -> &Result<Snapshot, crate::Error> {
        self.clock = self.clock.saturating_add(1);
        if !self.cache.contains_key(path) {
            if self.cache.len() >= 8
                && let Some(oldest) = self
                    .cache
                    .iter()
                    .min_by_key(|(_, s)| s.used)
                    .map(|(p, _)| p.clone())
            {
                self.cache.remove(&oldest);
            }
            let overlays = self
                .documents
                .iter()
                .map(|(p, d)| (p.clone(), d.text.clone()))
                .collect();
            let snapshot = source::load_with_overlays(path, &overlays)
                .and_then(|s| Snapshot::new(s, &self.policy))
                .or_else(|error| {
                    if let Some(doc) = self.documents.get(path) {
                        Snapshot::unresolved(
                            source::Source::unresolved(path.into(), doc.text.clone()),
                            error,
                        )
                    } else {
                        Err(error)
                    }
                });
            self.cache.insert(
                path.into(),
                Cached {
                    used: self.clock,
                    snapshot,
                },
            );
            self.analyses += 1;
        }
        let cached = self.cache.get_mut(path).unwrap();
        cached.used = self.clock;
        &cached.snapshot
    }
    fn text(&self, path: &Path) -> String {
        self.documents
            .get(path)
            .map(|d| d.text.to_string())
            .or_else(|| {
                self.cache
                    .values()
                    .filter_map(|s| s.snapshot.as_ref().ok())
                    .find_map(|s| s.source.original(path).map(str::to_owned))
            })
            .or_else(|| std::fs::read_to_string(path).ok())
            .unwrap_or_default()
    }
    fn range(&self, label: &Label) -> Value {
        let text = label
            .location
            .path
            .as_deref()
            .map(|p| self.text(p))
            .unwrap_or_default();
        json!({"start":position(&text,label.span.start),"end":position(&text,label.span.end)})
    }
    fn publish(&mut self) -> Vec<Value> {
        let paths = self.documents.keys().cloned().collect::<Vec<_>>();
        let mut output = vec![];
        for path in paths {
            let error = match self.snapshot(&path) {
                Ok(snapshot) => snapshot.error.clone(),
                Err(error) => Some(error.clone()),
            };
            let doc = &self.documents[&path];
            let mut diagnostics = vec![];
            if let Some(e) = error {
                let d = e.diagnostic();
                let primary = d.primary.as_ref();
                let in_root = primary
                    .and_then(|l| l.location.path.as_deref())
                    .is_none_or(|p| p == path);
                let range = if in_root {
                    primary.map(|l| self.range(l)).unwrap_or(
                        json!({"start":{"line":0,"character":0},"end":{"line":0,"character":0}}),
                    )
                } else {
                    json!({"start":{"line":0,"character":0},"end":{"line":0,"character":0}})
                };
                let related=d.related.iter().chain(primary.filter(|_|!in_root)).filter_map(|l|Some(json!({"location":{"uri":file_uri(l.location.path.as_deref()?).ok()?,"range":self.range(l)},"message":if l.message.is_empty(){d.message.as_str()}else{&l.message}}))).collect::<Vec<_>>();
                diagnostics.push(json!({"range":range,"severity":1,"code":d.code,"source":"wlt","message":std::iter::once(d.message.clone()).chain(d.notes.iter().cloned()).chain(d.help.iter().map(|h|format!("help: {h}"))).collect::<Vec<_>>().join("\n"),"relatedInformation":related}));
            }
            output.push(json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":doc.uri,"version":doc.version,"diagnostics":diagnostics}}));
        }
        output
    }
    fn edit(&mut self, method: &str, p: &Value) -> Result<Vec<Value>, String> {
        let td = &p["textDocument"];
        let uri = td["uri"].as_str().ok_or("missing document URI")?;
        let path = uri_path(uri)?;
        match method {
            "textDocument/didOpen" => {
                if self.documents.len() >= 128 && !self.documents.contains_key(&path) {
                    return Err("too many open documents".into());
                }
                let text = td["text"].as_str().ok_or("missing document text")?;
                if text.len() > 16 * 1024 * 1024 {
                    return Err("source exceeds 16 MiB".into());
                }
                let version = td["version"].as_i64().ok_or("missing document version")?;
                if self
                    .documents
                    .iter()
                    .filter(|(p, _)| **p != path)
                    .map(|(_, d)| d.text.len())
                    .sum::<usize>()
                    + text.len()
                    > 64 * 1024 * 1024
                {
                    return Err("open documents exceed 64 MiB".into());
                }
                self.documents.insert(
                    path,
                    Document {
                        uri: uri.into(),
                        version,
                        text: text.into(),
                    },
                );
            }
            "textDocument/didChange" => {
                let version = td["version"].as_i64().ok_or("missing document version")?;
                let doc = self
                    .documents
                    .get_mut(&path)
                    .ok_or("document is not open")?;
                if version <= doc.version {
                    return Ok(vec![]);
                }
                let mut text = doc.text.to_string();
                for change in p["contentChanges"]
                    .as_array()
                    .ok_or("missing content changes")?
                {
                    let replacement = change["text"].as_str().ok_or("missing changed text")?;
                    if let Some(range) = change.get("range") {
                        let start: Position = serde_json::from_value(range["start"].clone())
                            .map_err(|_| "invalid start position")?;
                        let end: Position = serde_json::from_value(range["end"].clone())
                            .map_err(|_| "invalid end position")?;
                        let start = byte_offset(&text, start).ok_or("position outside document")?;
                        let end = byte_offset(&text, end).ok_or("position outside document")?;
                        if start > end {
                            return Err("reversed change range".into());
                        }
                        text.replace_range(start..end, replacement);
                    } else {
                        text = replacement.into();
                    }
                    if text.len() > 16 * 1024 * 1024 {
                        return Err("source exceeds 16 MiB".into());
                    }
                }
                if self
                    .documents
                    .iter()
                    .filter(|(p, _)| **p != path)
                    .map(|(_, d)| d.text.len())
                    .sum::<usize>()
                    + text.len()
                    > 64 * 1024 * 1024
                {
                    return Err("open documents exceed 64 MiB".into());
                }
                let doc = self.documents.get_mut(&path).unwrap();
                doc.text = text.into();
                doc.version = version;
            }
            "textDocument/didClose" => {
                self.documents.remove(&path);
                self.cache.clear();
                let mut out = vec![
                    json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":[]}}),
                ];
                out.extend(self.publish());
                return Ok(out);
            }
            _ => {}
        }
        self.cache.clear();
        Ok(self.publish())
    }
    fn query(&mut self, method: &str, p: &Value) -> Result<Value, String> {
        let path = uri_path(
            p["textDocument"]["uri"]
                .as_str()
                .ok_or("missing document URI")?,
        )?;
        let pos: Position =
            serde_json::from_value(p["position"].clone()).map_err(|_| "invalid position")?;
        let text = self.text(&path);
        let Some(byte) = byte_offset(&text, pos) else {
            return Ok(Value::Null);
        };
        let scalar = position(&text, byte);
        let column = text[..byte]
            .rsplit('\n')
            .next()
            .unwrap_or("")
            .chars()
            .count()
            + 1;
        let Ok(snapshot) = self.snapshot(&path) else {
            return Ok(Value::Null);
        };
        if method == "textDocument/completion" {
            return Ok(json!(
                snapshot
                    .completions(&path, scalar.line + 1, column)
                    .into_iter()
                    .map(|(label, detail, kind)| json!({"label":label,"detail":detail,"kind":kind}))
                    .collect::<Vec<_>>()
            ));
        }
        let q = snapshot
            .query(&path, scalar.line + 1, column)
            .map_err(|e| e.to_string())?;
        let link = snapshot
            .link(&path, scalar.line + 1, column)
            .map(|(l, t)| (l.clone(), t.to_owned()));
        if method == "textDocument/definition" {
            let local = q.matches.iter().find_map(|m| m.declaration.clone());
            if let Some(label) = local.or_else(|| link.map(|(l, _)| l)).or_else(|| {
                snapshot
                    .declaration(&path, scalar.line + 1, column)
                    .cloned()
            }) && let Some(path) = &label.location.path
            {
                return Ok(json!([{"uri":file_uri(path)?,"range":self.range(&label)}]));
            }
            Ok(Value::Null)
        } else {
            let values = if let Some((_, display)) = link {
                vec![display]
            } else {
                q.matches
                    .iter()
                    .map(|m| {
                        format!(
                            "{}: {}",
                            m.specialization.as_deref().unwrap_or(&m.function),
                            m.ty
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let values = if values.is_empty() {
                snapshot
                    .declaration_hover(&path, scalar.line + 1, column)
                    .into_iter()
                    .collect()
            } else {
                values
            };
            if values.is_empty() {
                return Ok(Value::Null);
            }
            Ok(json!({"contents":{"kind":"plaintext","value":values.join("\n")}}))
        }
    }
    pub fn handle(&mut self, message: Value) -> Vec<Value> {
        let Some(method) = message["method"].as_str() else {
            return vec![];
        };
        let params = &message["params"];
        let id = message.get("id").cloned();
        if self.shutdown && method != "exit" {
            return id.map(|id|vec![json!({"jsonrpc":"2.0","id":id,"error":{"code":-32600,"message":"server has shut down"}})]).unwrap_or_default();
        }
        let result=match method {
            "initialize"=>Ok(json!({"capabilities":{"positionEncoding":"utf-16","textDocumentSync":{"openClose":true,"change":2,"save":true},"hoverProvider":true,"definitionProvider":true,"completionProvider":{"triggerCharacters":["$","."]}},"serverInfo":{"name":"wlt","version":env!("CARGO_PKG_VERSION")}})),
            "shutdown"=>{self.shutdown=true;Ok(Value::Null)},
            "textDocument/hover"|"textDocument/definition"|"textDocument/completion"=>self.query(method,params),
            "textDocument/didOpen"|"textDocument/didChange"|"textDocument/didClose"|"textDocument/didSave"=>return self.edit(method,params).unwrap_or_else(|message|vec![json!({"jsonrpc":"2.0","method":"window/logMessage","params":{"type":1,"message":message}})]),
            "workspace/didChangeWatchedFiles"=>{self.cache.clear();return self.publish()},
            "initialized"|"$/cancelRequest"|"exit"=>return vec![],
            _=>return id.map(|id|vec![json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"method not found"}})]).unwrap_or_default(),
        };
        id.map(|id| {
            vec![match result {
                Ok(value) => json!({"jsonrpc":"2.0","id":id,"result":value}),
                Err(message) => {
                    json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":message}})
                }
            }]
        })
        .unwrap_or_default()
    }
}

#[cfg(test)]
mod memory_contracts {
    use super::*;
    #[test]
    fn unsaved_source_snapshots_share_the_immutable_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.wlt");
        let text: std::sync::Arc<str> = "(module)".into();
        let overlays = BTreeMap::from([(path.clone(), text.clone())]);
        let loaded = source::load_with_overlays(&path, &overlays).unwrap();
        assert_eq!(
            loaded
                .original(&source::normalize_path(&path).unwrap())
                .unwrap()
                .as_ptr(),
            text.as_ptr()
        );
    }
    #[test]
    fn editor_snapshots_are_bounded_and_closed_documents_retire() {
        let dir = tempfile::tempdir().unwrap();
        let mut server = Session::new(Default::default());
        for n in 0..10 {
            let uri = file_uri(&dir.path().join(format!("{n}.wlt"))).unwrap();
            server.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":"(module)"}}}));
            assert!(
                server.cache.len() <= 8,
                "snapshots grew with document history"
            );
        }
        for n in 0..10 {
            let uri = file_uri(&dir.path().join(format!("{n}.wlt"))).unwrap();
            server.handle(
                json!({"method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}}),
            );
        }
        assert!(server.documents.is_empty());
        assert!(server.cache.is_empty());
    }
}
