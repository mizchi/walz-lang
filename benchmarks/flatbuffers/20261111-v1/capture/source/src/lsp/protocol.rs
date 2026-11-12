//! Bounded JSON-RPC framing; a reader observes cancellation while checking runs.
use super::Session;
use crate::bindings::BindingPolicy;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, BufRead, Write},
    sync::{Arc, Mutex, mpsc},
};
fn frame(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut length = None;
    let mut header_bytes = 0;
    loop {
        let mut line = String::new();
        let n =
            std::io::Read::take(&mut *reader, (8193 - header_bytes) as u64).read_line(&mut line)?;
        if n == 0 {
            return if header_bytes == 0 {
                Ok(None)
            } else {
                Err(io::Error::other("truncated LSP header"))
            };
        }
        header_bytes += n;
        if header_bytes > 8192 {
            return Err(io::Error::other("LSP header too large"));
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(io::Error::other("invalid LSP header"));
        };
        if key.eq_ignore_ascii_case("Content-Length") {
            if length.is_some() {
                return Err(io::Error::other("duplicate Content-Length"));
            }
            length = Some(value.trim().parse::<usize>().map_err(io::Error::other)?);
        }
    }
    let length = length
        .filter(|n| *n <= 16 * 1024 * 1024)
        .ok_or_else(|| io::Error::other("invalid Content-Length"))?;
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(io::Error::other)
}
fn write_frame(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", bytes.len())?;
    writer.write_all(&bytes)?;
    writer.flush()
}
#[derive(Default)]
struct Incoming {
    cancelled: BTreeSet<String>,
    pending: BTreeSet<String>,
    versions: BTreeMap<std::path::PathBuf, Option<i64>>,
}
impl Incoming {
    fn cancel(&mut self, id: &Value) {
        let key = id.to_string();
        if self.pending.contains(&key) {
            self.cancelled.insert(key);
        }
    }
    fn finish(&mut self, id: &Value) -> bool {
        let key = id.to_string();
        self.pending.remove(&key);
        self.cancelled.remove(&key)
    }
}
pub fn serve_stdio(policy: BindingPolicy) -> io::Result<bool> {
    let incoming = Arc::new(Mutex::new(Incoming::default()));
    let state = incoming.clone();
    let (send, receive) = mpsc::sync_channel(8);
    std::thread::spawn(move || {
        let mut reader = io::BufReader::new(io::stdin());
        loop {
            match frame(&mut reader) {
                Ok(Some(message)) => {
                    {
                        let mut state = state.lock().unwrap();
                        let method = message["method"].as_str().unwrap_or("");
                        if let Some(id) = message.get("id") {
                            state.pending.insert(id.to_string());
                        }
                        if method == "$/cancelRequest" {
                            state.cancel(&message["params"]["id"]);
                        }
                        if let Some(path) = message["params"]["textDocument"]["uri"]
                            .as_str()
                            .and_then(|uri| super::uri_path(uri).ok())
                        {
                            if matches!(method, "textDocument/didOpen" | "textDocument/didChange") {
                                let version = message["params"]["textDocument"]["version"].as_i64();
                                if state
                                    .versions
                                    .get(&path)
                                    .and_then(|v| *v)
                                    .zip(version)
                                    .is_none_or(|(old, new)| new > old)
                                {
                                    state.versions.insert(path, version);
                                }
                            } else if method == "textDocument/didClose" {
                                state.versions.insert(path, None);
                            }
                        }
                    }
                    if send.send(Ok(message)).is_err() {
                        return;
                    }
                }
                Ok(None) => return,
                Err(e) => {
                    let _ = send.send(Err(e));
                    return;
                }
            }
        }
    });
    let mut writer = io::BufWriter::new(io::stdout());
    let mut session = Session::new(policy);
    for message in receive {
        let message = match message {
            Ok(m) => m,
            Err(e) => {
                write_frame(
                    &mut writer,
                    &json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":e.to_string()}}),
                )?;
                return Ok(false);
            }
        };
        if message["method"] == "exit" {
            return Ok(session.shutdown);
        }
        let closed = (message["method"] == "textDocument/didClose")
            .then(|| {
                message["params"]["textDocument"]["uri"]
                    .as_str()
                    .and_then(|uri| super::uri_path(uri).ok())
            })
            .flatten();
        let id = message.get("id").cloned();
        let before = id
            .as_ref()
            .is_some_and(|id| incoming.lock().unwrap().cancelled.contains(&id.to_string()));
        let mut output = if before {
            vec![]
        } else {
            session.handle(message)
        };
        let cancelled = id
            .as_ref()
            .is_some_and(|id| incoming.lock().unwrap().finish(id));
        if cancelled {
            output = vec![
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32800,"message":"request cancelled"}}),
            ]
        }
        for response in output {
            if response["method"] == "textDocument/publishDiagnostics" {
                let uri = response["params"]["uri"].as_str().unwrap_or("");
                let current = super::uri_path(uri)
                    .ok()
                    .and_then(|path| incoming.lock().unwrap().versions.get(&path).cloned());
                if let Some(version) = current
                    && response["params"]["version"].as_i64() != version
                {
                    continue;
                }
            }
            write_frame(&mut writer, &response)?;
        }
        if let Some(path) = closed {
            let mut state = incoming.lock().unwrap();
            if state.versions.get(&path) == Some(&None) {
                state.versions.remove(&path);
            }
        }
    }
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_tracks_only_live_requests() {
        let mut state = Incoming::default();
        for id in 0..1000 {
            state.cancel(&json!(id));
        }
        assert!(state.cancelled.is_empty());
        state.pending.insert("9".into());
        state.cancel(&json!(9));
        assert!(state.finish(&json!(9)));
        assert!(state.pending.is_empty());
        state.cancel(&json!(9));
        assert!(state.cancelled.is_empty());
    }
    #[test]
    fn framing_rejects_truncation_and_oversized_allocations() {
        for data in [
            "Content-Length: 999999999\r\n\r\n",
            "Content-Length: 1\r\nContent-Length: 1\r\n\r\n0",
            "Content-Length: 10\r\n\r\n{}",
        ] {
            assert!(frame(&mut io::Cursor::new(data)).is_err());
        }
    }
}
