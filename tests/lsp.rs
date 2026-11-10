use serde_json::{Value, json};
use std::io::{Read, Write};
use std::process::{Command, Stdio};

fn position(s: &str, needle: &str) -> Value {
    let n = s.find(needle).unwrap();
    let before = &s[..n];
    json!({"line":before.bytes().filter(|&b|b==b'\n').count(),"character":before.rsplit('\n').next().unwrap().encode_utf16().count()})
}
#[test]
fn unsaved_modules_share_checked_snapshots_and_utf16_definitions() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("main.walz");
    let dependency = dir.path().join("part.walz");
    std::fs::write(&root, "(module)").unwrap();
    std::fs::write(&dependency, "(module)").unwrap();
    let root_uri = walz::lsp::file_uri(&root).unwrap();
    let dep_uri = walz::lsp::file_uri(&dependency).unwrap();
    let mut s = walz::lsp::Session::new(Default::default());
    s.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":dep_uri,"version":1,"text":"(module (func $answer (result i32) 42) (@export (func $answer)))"}}}));
    let text = "(module (@use \"./part.walz\" (func $answer)) (func $main (result i32) (; 🦀 ;)\n (@let $x (type i32) 1 (; 🦀 ;) (+ $x (call $answer)))))";
    let out=s.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":root_uri,"version":1,"text":text}}}));
    assert!(
        out.iter()
            .any(|v| v["params"]["uri"] == root_uri && v["params"]["diagnostics"] == json!([])),
        "{out:?}"
    );
    let query = |method: &str, id: u32, p: Value| json!({"id":id,"method":method,"params":{"textDocument":{"uri":root_uri},"position":p}});
    let out = s.handle(query("textDocument/hover", 1, position(text, "$x (call")));
    assert!(
        out[0]["result"]["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("i32"),
        "{out:?}"
    );
    let out = s.handle(query(
        "textDocument/definition",
        2,
        position(text, "$answer)))))"),
    ));
    assert_eq!(out[0]["result"][0]["uri"], dep_uri, "{out:?}");
    let before = s.analysis_count();
    s.handle(query("textDocument/hover", 3, position(text, "$x (call")));
    assert_eq!(
        before,
        s.analysis_count(),
        "same revision must reuse checked IR"
    );
    let changed = text.replace("$x (call", "$missing (call");
    let out=s.handle(json!({"method":"textDocument/didChange","params":{"textDocument":{"uri":root_uri,"version":2},"contentChanges":[{"text":changed}]}}));
    assert!(
        out.iter().any(|v| v["params"]["uri"] == root_uri
            && !v["params"]["diagnostics"].as_array().unwrap().is_empty()),
        "{out:?}"
    );
    let newer = s.analysis_count();
    s.handle(json!({"method":"textDocument/didChange","params":{"textDocument":{"uri":root_uri,"version":1},"contentChanges":[{"text":text}]}}));
    assert_eq!(newer, s.analysis_count(), "stale edit ignored");
}
#[test]
fn stdio_framing_shutdown_and_cancellation_are_protocol_only() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_walz"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for message in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        json!({"jsonrpc":"2.0","id":9,"method":"textDocument/hover","params":{}}),
        json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":9}}),
        json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ] {
        let bytes = serde_json::to_vec(&message).unwrap();
        write!(input, "Content-Length: {}\r\n\r\n", bytes.len()).unwrap();
        input.write_all(&bytes).unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let mut reader = std::io::BufReader::new(output.stdout.as_slice());
    let mut responses = vec![];
    loop {
        use std::io::BufRead;
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap() == 0 {
            break;
        }
        let length: usize = line
            .trim()
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse()
            .unwrap();
        reader.read_line(&mut String::new()).unwrap();
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        responses.push(serde_json::from_slice::<Value>(&body).unwrap());
    }
    assert_eq!(responses.len(), 3);
    assert_eq!(
        responses[0]["result"]["capabilities"]["positionEncoding"],
        "utf-16"
    );
    assert!(
        matches!(
            responses[1]["error"]["code"].as_i64(),
            Some(-32800 | -32602)
        ),
        "cancellation may race with a completed request: {responses:?}"
    );
    assert_eq!(responses[2]["result"], Value::Null);
}

#[test]
fn utf16_positions_round_trip_and_reject_half_surrogates() {
    use walz::lsp::{Position, byte_offset};
    let s = "a🦀日本\r\nx";
    assert_eq!(
        byte_offset(
            s,
            Position {
                line: 0,
                character: 1
            }
        ),
        Some(1)
    );
    assert_eq!(
        byte_offset(
            s,
            Position {
                line: 0,
                character: 2
            }
        ),
        None
    );
    assert_eq!(
        byte_offset(
            s,
            Position {
                line: 0,
                character: 3
            }
        ),
        Some(5)
    );
    assert_eq!(
        byte_offset(
            s,
            Position {
                line: 0,
                character: 5
            }
        ),
        Some(11)
    );
    assert_eq!(
        byte_offset(
            s,
            Position {
                line: 1,
                character: 1
            }
        ),
        Some(s.len())
    );
    assert_eq!(
        byte_offset(
            s,
            Position {
                line: 2,
                character: 0
            }
        ),
        None
    );
}
#[test]
fn incremental_changes_use_utf16_and_invalidate_importers() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("new 日本.walz");
    let uri = walz::lsp::file_uri(&file).unwrap();
    assert!(uri.contains("%20"));
    let mut session = walz::lsp::Session::new(Default::default());
    let text = "(module (func $f (result i32) (; 🦀 ;) (i64.const 1)))";
    session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":text}}}));
    let start = position(text, "i64");
    let mut end = start.clone();
    end["character"] = json!(start["character"].as_u64().unwrap() + 3);
    let out=session.handle(json!({"method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"range":{"start":start,"end":end},"text":"i32"}]}}));
    assert_eq!(out[0]["params"]["diagnostics"], json!([]), "{out:?}");
    assert!(!file.exists(), "unsaved checking must not write a file");
}

#[test]
fn erroneous_current_revisions_keep_declarations_navigation_and_completion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.walz");
    let part = dir.path().join("part.walz");
    let uri = walz::lsp::file_uri(&path).unwrap();
    let dep = walz::lsp::file_uri(&part).unwrap();
    let mut s = walz::lsp::Session::new(Default::default());
    s.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":dep,"version":1,"text":"(module (@export (func $sum)) (func $sum (param $n i32) (result i32) $n))"}}}));
    let text = "(module (@use \"./part.walz\" (func $sum))\n(func $main (result i32) (; 😀 ;) (@let $stable (type i32) 41 (@let $answer (call $sum (i64.const 1)) (+ $stable $answer)))))";
    let out=s.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":text}}}));
    assert!(
        out.iter().any(|v| v["params"]["uri"] == uri
            && !v["params"]["diagnostics"].as_array().unwrap().is_empty())
    );
    for (needle, expected) in [("$stable $answer", "i32"), ("$answer)))", "binding")] {
        let p = position(text, needle);
        let hover=s.handle(json!({"id":1,"method":"textDocument/hover","params":{"textDocument":{"uri":uri},"position":p}}));
        let display = hover[0]["result"]["contents"]["value"]
            .as_str()
            .unwrap_or("");
        assert!(
            display.contains(expected) && display.contains("unverified"),
            "{hover:?}"
        );
    }
    let definition=s.handle(json!({"id":2,"method":"textDocument/definition","params":{"textDocument":{"uri":uri},"position":position(text,"$sum (i64")}}));
    assert_eq!(definition[0]["result"][0]["uri"], dep, "{definition:?}");
    let completion=s.handle(json!({"id":3,"method":"textDocument/completion","params":{"textDocument":{"uri":uri},"position":position(text,"$stable $answer")}}));
    assert!(
        completion[0]["result"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["label"] == "$stable"),
        "{completion:?}"
    );
    let changed = text.replace("(call $sum (i64.const 1))", "(call $sum 1)");
    let out=s.handle(json!({"method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":changed}]}}));
    assert!(
        out.iter()
            .any(|v| v["params"]["uri"] == uri && v["params"]["diagnostics"] == json!([]))
    );
}

#[test]
fn recovery_uses_current_positions_and_lexical_shadowing_without_old_types() {
    let dir = tempfile::tempdir().unwrap();
    let uri = walz::lsp::file_uri(&dir.path().join("main.walz")).unwrap();
    let mut s = walz::lsp::Session::new(Default::default());
    for (version, prefix) in [(1, ""), (2, "\n\n(; 😀 ;)\n")] {
        let text = format!(
            "{prefix}(module (func $bad (result i32) $missing) (func $f (result i32) (@let $x (type i32) 1 (block (result i32) (drop (@let $x (type i64) 2 $x)) $x))))"
        );
        if version == 1 {
            s.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":version,"text":text}}}));
        } else {
            s.handle(json!({"method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":version},"contentChanges":[{"text":text}]}}));
        }
        for (needle, ty, decl) in [
            ("$x)) $x", "i64", "$x (type i64)"),
            ("$x))))", "i32", "$x (type i32)"),
        ] {
            let hover=s.handle(json!({"id":4,"method":"textDocument/hover","params":{"textDocument":{"uri":uri},"position":position(&text,needle)}}));
            assert!(
                hover[0]["result"]["contents"]["value"]
                    .as_str()
                    .unwrap_or("")
                    .contains(ty),
                "{hover:?}"
            );
            let definition=s.handle(json!({"id":5,"method":"textDocument/definition","params":{"textDocument":{"uri":uri},"position":position(&text,needle)}}));
            assert_eq!(
                definition[0]["result"][0]["range"]["start"],
                position(&text, decl),
                "{definition:?}"
            );
        }
    }
}

#[test]
fn incomplete_edits_keep_current_declarations_and_offer_core_imports() {
    let dir = tempfile::tempdir().unwrap();
    let uri = walz::lsp::file_uri(&dir.path().join("main.walz")).unwrap();
    let mut s = walz::lsp::Session::new(Default::default());
    let text = "(module (@use \"walz:core\" (func $json.)) (func $f (param $x i32) (result i32) $x";
    assert!(walz::check(text).is_err());
    let notifications=s.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":text}}}));
    assert!(
        notifications
            .iter()
            .any(|n| !n["params"]["diagnostics"].as_array().unwrap().is_empty())
    );
    let p = position(text, "$x"); // parameter declaration itself
    let hover=s.handle(json!({"id":6,"method":"textDocument/hover","params":{"textDocument":{"uri":uri},"position":p}}));
    assert!(
        hover[0]["result"]["contents"]["value"]
            .as_str()
            .unwrap_or("")
            .contains("unverified"),
        "{hover:?}"
    );
    let completion=s.handle(json!({"id":7,"method":"textDocument/completion","params":{"textDocument":{"uri":uri},"position":position(text,"$json.")}}));
    assert!(
        completion[0]["result"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["label"] == "$json.parse"),
        "{completion:?}"
    );
}

#[test]
fn declaration_recovery_keeps_wat_type_function_and_local_names_separate() {
    let dir = tempfile::tempdir().unwrap();
    let uri = walz::lsp::file_uri(&dir.path().join("main.walz")).unwrap();
    let text = "(module (type $Same (struct (field i32))) (func $Same (result i32) 1) (func $bad (result i32) $missing) (func $run (param $Same i32) (result i32) (block (result i32) (drop (struct.new $Same 1)) (call $Same))))";
    let mut s = walz::lsp::Session::new(Default::default());
    s.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":text}}}));
    for (needle, target) in [
        ("$Same 1)", "$Same (struct"),
        ("$Same))))", "$Same (result"),
    ] {
        let output=s.handle(json!({"id":8,"method":"textDocument/definition","params":{"textDocument":{"uri":uri},"position":position(text,needle)}}));
        assert_eq!(
            output[0]["result"][0]["range"]["start"],
            position(text, target),
            "{output:?}"
        );
    }
}
