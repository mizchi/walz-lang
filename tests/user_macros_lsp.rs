use serde_json::{Value, json};

fn position(text: &str, needle: &str) -> Value {
    let prefix = &text[..text.find(needle).unwrap()];
    json!({"line":prefix.bytes().filter(|&b|b==b'\n').count(),"character":prefix.rsplit('\n').next().unwrap().encode_utf16().count()})
}

#[test]
fn macros_offer_signature_definition_expansion_and_binding_completion_in_current_revision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.wlt");
    let uri = wlt::lsp::file_uri(&path).unwrap();
    let text = "(module\n (%macro $with\n  (rule (?name:ident ?value:expr ?body:expr)\n   (bind ?name ?body) (%let ?name ?value ?body)))\n (func $f (result i32) (; 🦀 ;) (%$with $n (i32.const 42) (+ $n 0))))";
    let mut session = wlt::lsp::Session::new(Default::default());
    let open=session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"text":text,"version":1}}}));
    assert!(
        open.iter().any(|v| v["params"]["diagnostics"] == json!([])),
        "{open:?}"
    );
    let request = |method: &str, needle: &str| json!({"id":1,"method":method,"params":{"textDocument":{"uri":uri},"position":position(text,needle)}});
    let out = session.handle(request("textDocument/hover", "%$with"));
    let value = out[0]["result"]["contents"]["value"].as_str().unwrap();
    assert!(
        value.contains("macro") && value.contains("ident"),
        "{out:?}"
    );
    let out = session.handle(request("textDocument/definition", "%$with"));
    assert_eq!(out[0]["result"][0]["range"]["start"]["line"], 1, "{out:?}");
    let out = session.handle(request("wlt/expandMacro", "%$with"));
    assert!(
        out[0]["result"]["expansions"][0]["expanded"]
            .as_str()
            .unwrap()
            .contains("let"),
        "{out:?}"
    );
    let out = session.handle(request("textDocument/definition", "$n 0"));
    assert_eq!(
        out[0]["result"][0]["range"]["start"],
        position(text, "$n (i32"),
        "{out:?}"
    );

    let broken = text.replace("(+ $n 0)", "(+ $n $missing)");
    session.handle(json!({"method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":broken}]}}));
    let out=session.handle(json!({"id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":uri},"position":position(&broken,"$missing")}}));
    assert!(
        out[0]["result"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["label"] == "$n"),
        "{out:?}"
    );
    let out=session.handle(json!({"id":3,"method":"textDocument/hover","params":{"textDocument":{"uri":uri},"position":position(&broken,"$n $missing")}}));
    let hover = out[0]["result"]["contents"]["value"].as_str().unwrap();
    assert!(hover.contains("unverified"), "{out:?}");
}

#[test]
fn generated_temporary_ranges_do_not_hide_argument_types() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.wlt");
    let uri = wlt::lsp::file_uri(&path).unwrap();
    let text = "(module (%macro $convert (rule (?a:expr ?b:expr) (%let $tmp ?a (i32.wrap_i64 ?b)))) (func $f (param $a i32) (param $b i64) (result i32) (%$convert $a $b)))";
    let mut session = wlt::lsp::Session::new(Default::default());
    session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"text":text,"version":1}}}));
    let out=session.handle(json!({"id":1,"method":"textDocument/hover","params":{"textDocument":{"uri":uri},"position":position(text,"$b)))")}}));
    assert!(
        out[0]["result"]["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("i64"),
        "{out:?}"
    );
}

#[test]
fn erased_arguments_do_not_receive_the_macro_results_checked_type() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.wlt");
    let uri = wlt::lsp::file_uri(&path).unwrap();
    let text = "(module (%macro $keep (rule (?yes:expr ?no:expr) (%let $saved ?yes $saved))) (func $f (result i32) (%$keep (i32.const 42) (i64.const 99))))";
    let mut session = wlt::lsp::Session::new(Default::default());
    session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"text":text,"version":1}}}));
    let out=session.handle(json!({"id":1,"method":"textDocument/hover","params":{"textDocument":{"uri":uri},"position":position(text,"i64.const 99")}}));
    assert!(out[0]["result"].is_null(), "{out:?}");
}

#[test]
fn macro_completion_uses_its_own_namespace_and_imported_definitions_map_to_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.wlt");
    let library = dir.path().join("syntax.wlt");
    let uri = wlt::lsp::file_uri(&main).unwrap();
    let library_uri = wlt::lsp::file_uri(&library).unwrap();
    let mut session = wlt::lsp::Session::new(Default::default());
    let lib = "(module (%macro $id (rule (?x:expr) ?x)) (%export (macro $id)))";
    session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":library_uri,"text":lib,"version":1}}}));
    let text = "(module (%use \"./syntax.wlt\" (macro $id $id)) (func $id (result i32) 9) (func $f (result i32) (%$id (i32.const 42))))";
    let open=session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"text":text,"version":1}}}));
    assert!(
        open.iter()
            .any(|v| v["params"]["uri"] == uri && v["params"]["diagnostics"] == json!([])),
        "{open:?}"
    );
    let request = |method: &str| json!({"id":1,"method":method,"params":{"textDocument":{"uri":uri},"position":position(text,"%$id")}});
    let out = session.handle(request("textDocument/completion"));
    let values = out[0]["result"].as_array().unwrap();
    assert!(values.iter().any(|v| v["label"] == "%$id"), "{out:?}");
    assert!(values.iter().any(|v| v["label"] == "$id"), "{out:?}");
    let out = session.handle(request("textDocument/definition"));
    assert_eq!(out[0]["result"][0]["uri"], library_uri, "{out:?}");
}

#[test]
fn incomplete_imported_macro_bodies_recover_current_bindings_and_source_locations() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.wlt");
    let library = dir.path().join("syntax.wlt");
    let uri = wlt::lsp::file_uri(&main).unwrap();
    let library_uri = wlt::lsp::file_uri(&library).unwrap();
    let lib = "(module (%macro $with (rule (?n:ident ?v:expr ?body:expr) (bind ?n ?body) (%let ?n ?v ?body))) (%export (macro $with)))";
    let text = "(module\n (%use \"./syntax.wlt\" (macro $with $use))\n (func $f (param $outer i64) (result i64) (; 🦀 ;)\n   (%$use $amount (i64.const 42) (+ $amount $am";
    let mut session = wlt::lsp::Session::new(Default::default());
    session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":library_uri,"text":lib,"version":1}}}));
    let diagnostics=session.handle(json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"text":text,"version":1}}}));
    assert!(
        diagnostics.iter().any(|v| v["params"]["uri"] == uri
            && !v["params"]["diagnostics"].as_array().unwrap().is_empty())
    );
    let request = |method: &str, needle: &str| json!({"id":1,"method":method,"params":{"textDocument":{"uri":uri},"position":if needle=="$am" {let prefix=&text[..text.rfind(needle).unwrap()];json!({"line":prefix.bytes().filter(|b|*b==b'\n').count(),"character":prefix.rsplit('\n').next().unwrap().encode_utf16().count()})} else {position(text,needle)}}});
    let out = session.handle(request("textDocument/completion", "$am"));
    assert!(
        out[0]["result"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["label"] == "$amount"),
        "{out:?}"
    );
    let out = session.handle(request("textDocument/definition", "$amount $am"));
    assert_eq!(
        out[0]["result"][0]["range"]["start"],
        position(text, "$amount (i64"),
        "{out:?}"
    );
    let out = session.handle(request("textDocument/hover", "$amount $am"));
    assert!(
        out[0]["result"]["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("unverified"),
        "{out:?}"
    );
    let out = session.handle(request("textDocument/definition", "%$use"));
    assert_eq!(out[0]["result"][0]["uri"], library_uri, "{out:?}");
    let out = session.handle(request("wlt/expandMacro", "%$use"));
    assert_eq!(out[0]["result"]["expansions"], json!([]), "{out:?}");
    session.handle(json!({"method":"textDocument/didChange","params":{"textDocument":{"uri":library_uri,"version":2},"contentChanges":[{"text":"(module)"}]}}));
    let out = session.handle(request("textDocument/completion", "$am"));
    assert!(
        !out[0]["result"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["label"] == "$amount"),
        "stale scope: {out:?}"
    );
}
