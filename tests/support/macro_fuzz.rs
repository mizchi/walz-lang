//! Reproducible differential corpus. The model never calls the compiler, and
//! manual syntax is rendered separately from user macro invocations.
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Const(i64),
    Trace(i64),
    Local,
    Add(Box<Self>, Box<Self>),
    Twice(Box<Self>),
    Dup(Box<Self>),
    Discard(Box<Self>),
    Choose(Box<Self>, Box<Self>, Box<Self>),
    Raise(Box<Self>),
    Caught(Box<Self>),
    Own(Box<Self>),
    Generic(Box<Self>),
    Sequence(Box<Self>, Box<Self>),
    Bindings(Box<Self>, Box<Self>),
}
impl Expr {
    fn children(&self) -> Vec<&Self> {
        match self {
            Self::Add(a, b) | Self::Sequence(a, b) | Self::Bindings(a, b) => vec![a, b],
            Self::Choose(a, b, c) => vec![a, b, c],
            Self::Twice(a)
            | Self::Dup(a)
            | Self::Discard(a)
            | Self::Raise(a)
            | Self::Caught(a)
            | Self::Own(a)
            | Self::Generic(a) => vec![a],
            _ => vec![],
        }
    }
    fn has_trace(&self) -> bool {
        matches!(self, Self::Trace(_)) || self.children().iter().any(|e| e.has_trace())
    }
    pub fn duplicates_trace(&self) -> bool {
        matches!(self,Self::Dup(a) if a.has_trace())
            || self.children().iter().any(|e| e.duplicates_trace())
    }
    fn smaller(&self) -> Vec<Self> {
        let mut out = vec![Self::Const(0)];
        out.extend(self.children().into_iter().cloned());
        for (i, child) in self.children().iter().enumerate() {
            for replacement in child.smaller() {
                let mut candidate = self.clone();
                let target = match &mut candidate {
                    Self::Add(a, b) | Self::Sequence(a, b) | Self::Bindings(a, b) => {
                        if i == 0 {
                            a
                        } else {
                            b
                        }
                    }
                    Self::Choose(a, b, c) => match i {
                        0 => a,
                        1 => b,
                        _ => c,
                    },
                    Self::Twice(a)
                    | Self::Dup(a)
                    | Self::Discard(a)
                    | Self::Raise(a)
                    | Self::Caught(a)
                    | Self::Own(a)
                    | Self::Generic(a) => a,
                    _ => unreachable!(),
                };
                **target = replacement;
                out.push(candidate);
            }
        }
        out
    }
    fn eval(&self, trace: &mut i64) -> Result<i64, i64> {
        Ok(match self {
            Self::Const(n) => *n,
            Self::Local => 21,
            Self::Trace(n) => {
                *trace = trace.wrapping_mul(31).wrapping_add(*n);
                *n
            }
            Self::Add(a, b) | Self::Bindings(a, b) => a.eval(trace)?.wrapping_add(b.eval(trace)?),
            Self::Twice(a) => a.eval(trace)?.wrapping_mul(2),
            Self::Dup(a) => a.eval(trace)?.wrapping_add(a.eval(trace)?),
            Self::Discard(_) => 0,
            Self::Choose(a, b, c) => {
                if a.eval(trace)? & 1 == 0 {
                    b.eval(trace)?
                } else {
                    c.eval(trace)?
                }
            }
            Self::Raise(a) => return Err(a.eval(trace)?),
            Self::Caught(a) => a.eval(trace).unwrap_or_else(|n| n),
            Self::Own(a) | Self::Generic(a) => a.eval(trace)?,
            Self::Sequence(a, b) => {
                a.eval(trace)?;
                b.eval(trace)?;
                0
            }
        })
    }
    fn render(&self, macros: bool, next: &mut usize) -> String {
        let id = *next;
        *next += 1;
        let children = self
            .children()
            .iter()
            .map(|e| e.render(macros, next))
            .collect::<Vec<_>>();
        if macros && matches!(self, Self::Bindings(..)) {
            return format!(
                "(%$delegated (($tmp {}) ($second {})) (i64.add $tmp $second))",
                children[0], children[1]
            );
        }
        if macros {
            let name = match self {
                Self::Add(..) => "add",
                Self::Twice(..) => "twice",
                Self::Dup(..) => "dup",
                Self::Discard(..) => "discard",
                Self::Choose(..) => "choose",
                Self::Raise(..) => "raise",
                Self::Caught(..) => "caught",
                Self::Own(..) => "own",
                Self::Generic(..) => "generic",
                Self::Sequence(..) => "sequence",
                _ => "",
            };
            if !name.is_empty() {
                return format!("(%${name} {})", children.join(" "));
            }
        }
        let a = children.first().map(String::as_str).unwrap_or("");
        let b = children.get(1).map(String::as_str).unwrap_or("");
        match self {
            Self::Const(n) => format!("(i64.const {n})"),
            Self::Trace(n) => format!("(call $event $counter (i64.const {n}))"),
            Self::Local => "$tmp".into(),
            Self::Add(..) => format!("(i64.add {a} {b})"),
            Self::Twice(..) => {
                format!("(%let $manual{id} (type i64) {a} (i64.add $manual{id} $manual{id}))")
            }
            Self::Dup(..) => format!("(i64.add {a} {a})"),
            Self::Discard(..) => "(i64.const 0)".into(),
            Self::Choose(..) => format!(
                "(if (result i64) (i64.eqz (i64.and {a} (i64.const 1))) (then {b}) (else {}))",
                children[2]
            ),
            Self::Raise(..) => format!("(throw $Stop {a})"),
            Self::Caught(..) => {
                format!("(%try (result i64) (do {a}) (catch $Stop ($manual{id}) $manual{id}))")
            }
            Self::Own(..) => format!(
                "(%match (%box i64) (result i64) (%box.new i64 {a}) (case own ($manual{id}) $manual{id}))"
            ),
            Self::Generic(..) => format!("(call (%inst $id i64) {a})"),
            Self::Bindings(..) => format!(
                "(%let $first{id} (type i64) {a} (%let $second{id} (type i64) {b} (i64.add $first{id} $second{id})))"
            ),
            Self::Sequence(..) => {
                format!("(block (result i64) (drop {a}) (drop {b}) (i64.const 0))")
            }
        }
    }
    fn source(&self, macros: bool) -> String {
        let definitions = if macros {
            r#"
          (%macro $bindings (rule (((?n:ident ?v:expr) ...) ?body:expr) (bind ?n ?body) (%let ((?n (type i64) ?v) ...) ?body)))
          (%macro $delegated (rule (((?n:ident ?v:expr) ...) ?body:expr) (bind ?n ?body) (%$bindings ((?n ?v) ...) ?body)))
          (%macro $add (rule (?a:expr ?b:expr) (i64.add ?a ?b)))
          (%macro $twice (rule (?x:expr) (%let $tmp (type i64) ?x (i64.add $tmp $tmp))))
          (%macro $dup (rule (?x:expr) (i64.add ?x ?x)))
          (%macro $discard (rule (?x:expr) (i64.const 0)))
          (%macro $choose (rule (?c:expr ?a:expr ?b:expr) (if (result i64) (i64.eqz (i64.and ?c (i64.const 1))) (then ?a) (else ?b))))
          (%macro $raise (rule (?x:expr) (throw $Stop ?x)))
          (%macro $caught (rule (?x:expr) (%try (result i64) (do ?x) (catch $Stop ($tmp) $tmp))))
          (%macro $own (rule (?x:expr) (%match (%box i64) (result i64) (%box.new i64 ?x) (case own ($tmp) $tmp))))
          (%macro $generic (rule (?x:expr) (call (%inst $id i64) ?x)))
          (%macro $sequence (rule (?x:expr ...) (block (result i64) (block (drop ?x) ...) (i64.const 0))))
        "#
        } else {
            ""
        };
        let body = self.render(macros, &mut 0);
        format!(
            r#"(module
          (type $Counter (struct (field $n (mut i64)))) (tag $Stop (param i64))
          (func $event (param $c (ref $Counter)) (param $n i64) (result i64)
            (block (result i64) (struct.set $Counter $n $c (i64.add (i64.mul (struct.get $Counter $n $c) (i64.const 31)) $n)) $n))
          (func $id (%type.params $T) (param $x $T) (result $T) (%effect pure) $x)
          {definitions}
          (func $run (result (%tuple i64 i64)) (%effect reads writes allocates)
            (%let (($counter (struct.new $Counter 0)) ($tmp (type i64) 21))
              (%let $value (%try (result i64) (do {body}) (catch $Stop ($caught) $caught))
                (%tuple.new i64 i64 $value (struct.get $Counter $n $counter)))))
          (func $main (export "main") (result i64) (%effect reads writes allocates)
            (%match (%tuple i64 i64) (result i64) (call $run) (case pair ($value $trace) $value)))
          (func $trace (export "trace") (result i64) (%effect reads writes allocates)
            (%match (%tuple i64 i64) (result i64) (call $run) (case pair ($value $trace) $trace))))"#
        )
    }
}
fn random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}
fn generate(state: &mut u64, depth: usize) -> Expr {
    let choice = (random(state) >> 32) % 14;
    if depth == 0 || choice < 3 {
        return match choice % 3 {
            0 => Expr::Const(random(state) as i64),
            1 => Expr::Trace(random(state) as i64),
            _ => Expr::Local,
        };
    }
    let a = Box::new(generate(state, depth - 1));
    match choice {
        3 => Expr::Add(a, Box::new(generate(state, depth - 1))),
        4 => Expr::Twice(a),
        5 => Expr::Dup(a),
        6 => Expr::Discard(a),
        7 => Expr::Choose(
            a,
            Box::new(generate(state, depth - 1)),
            Box::new(generate(state, depth - 1)),
        ),
        8 => Expr::Caught(Box::new(Expr::Raise(a))),
        9 => Expr::Caught(a),
        10 => Expr::Own(a),
        11 => Expr::Generic(a),
        12 => Expr::Sequence(a, Box::new(generate(state, depth - 1))),
        _ => Expr::Bindings(a, Box::new(generate(state, depth - 1))),
    }
}
pub fn reduce(mut input: Expr, mut fails: impl FnMut(&Expr) -> bool) -> Expr {
    while let Some(next) = input
        .smaller()
        .into_iter()
        .find(|e| e != &input && fails(e))
    {
        input = next;
    }
    input
}
fn execute(expr: &Expr, directory: &Path) -> Result<(), String> {
    let mut trace = 0;
    let value = expr.eval(&mut trace).unwrap_or_else(|n| n);
    let expected = format!("{value}\n{trace}");
    let mut effects = None;
    for (i, (macros, optimize)) in [(true, false), (false, false), (true, true), (false, true)]
        .into_iter()
        .enumerate()
    {
        let compiled = std::panic::catch_unwind(|| {
            wlt::compile_with_options(&expr.source(macros), wlt::CompileOptions { optimize })
        })
        .map_err(|_| "panic:compiler".to_string())?
        .map_err(|e| format!("compile-{}:{e}", e.diagnostic().code))?;
        let actual = serde_json::to_value(compiled.analysis.module_effects).unwrap();
        if let Some(ref required) = effects {
            if *required != actual {
                return Err(format!("effects:{required} != {actual}"));
            }
        } else {
            effects = Some(actual);
        }
        let path = directory.join(format!("{i}.wasm"));
        std::fs::write(&path, compiled.wasm).unwrap();
        let mut results = vec![];
        for export in ["main", "trace"] {
            let out = Command::new("wasmtime")
                .args([
                    "run",
                    "-W",
                    "gc=y,function-references=y,exceptions=y",
                    "--invoke",
                    export,
                ])
                .arg(&path)
                .output()
                .unwrap();
            if !out.status.success() {
                return Err(format!(
                    "wasmtime-runtime:{i}:{export}:{}",
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            results.push(String::from_utf8_lossy(&out.stdout).trim().to_owned());
        }
        if results.join("\n") != expected {
            return Err(format!(
                "wasmtime-value:{i}:expected {expected:?}; got {results:?}"
            ));
        }
        let out = Command::new("node")
            .arg(directory.join("run.mjs"))
            .arg(&path)
            .output()
            .unwrap();
        if !out.status.success() {
            return Err(format!(
                "v8-runtime:{i}:{}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        if String::from_utf8_lossy(&out.stdout).trim() != expected {
            return Err(format!(
                "v8-value:{i}:expected {expected:?}; got {}",
                String::from_utf8_lossy(&out.stdout)
            ));
        }
    }
    Ok(())
}
pub fn run() {
    let seed = std::env::var("WLT_MACRO_FUZZ_SEED")
        .map(|s| s.parse().unwrap())
        .unwrap_or(42);
    let count = std::env::var("WLT_MACRO_FUZZ_CASES")
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(16);
    assert!((1..=10000).contains(&count));
    let mut state = seed;
    let cases = if let Ok(path) = std::env::var("WLT_MACRO_FUZZ_REPLAY") {
        vec![serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()]
    } else {
        // Always exercise each semantic family, regardless of the random seed.
        let leaf = || Box::new(Expr::Trace(i64::MAX));
        let mut cases = vec![
            Expr::Const(i64::MIN),
            Expr::Local,
            Expr::Trace(-1),
            Expr::Add(leaf(), Box::new(Expr::Const(1))),
            Expr::Twice(leaf()),
            Expr::Dup(leaf()),
            Expr::Discard(leaf()),
            Expr::Choose(Box::new(Expr::Trace(0)), leaf(), Box::new(Expr::Trace(42))),
            Expr::Caught(Box::new(Expr::Raise(leaf()))),
            Expr::Own(leaf()),
            Expr::Generic(leaf()),
            Expr::Sequence(leaf(), leaf()),
            Expr::Bindings(Box::new(Expr::Local), leaf()),
        ];
        cases.extend((0..count).map(|_| generate(&mut state, 4)));
        cases
    };
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("run.mjs"),"import {readFile} from 'node:fs/promises'; const {instance}=await WebAssembly.instantiate(await readFile(process.argv[2]));console.log(instance.exports.main().toString());console.log(instance.exports.trace().toString());").unwrap();
    for (index, expr) in cases.into_iter().enumerate() {
        if let Err(error) = execute(&expr, directory.path()) {
            let signature = error.split(':').next().unwrap().to_owned();
            let reduced = reduce(expr.clone(), |e| {
                execute(e, directory.path()).is_err_and(|e| e.split(':').next() == Some(&signature))
            });
            let root = Path::new("target/macro-fuzz/failures");
            std::fs::create_dir_all(root).unwrap();
            let folder = root.join(format!("seed-{seed}-case-{index}-{}", std::process::id()));
            std::fs::create_dir(&folder).unwrap();
            for (name, e) in [("original", &expr), ("reduced", &reduced)] {
                std::fs::write(
                    folder.join(format!("{name}.json")),
                    serde_json::to_vec_pretty(e).unwrap(),
                )
                .unwrap();
                std::fs::write(folder.join(format!("{name}.macro.wlt")), e.source(true)).unwrap();
                std::fs::write(folder.join(format!("{name}.manual.wlt")), e.source(false)).unwrap();
            }
            std::fs::write(folder.join("failure.json"),serde_json::to_vec_pretty(&serde_json::json!({"seed":seed,"case":index,"failure":error,"signature":signature})).unwrap()).unwrap();
            panic!(
                "{error}; replay: just fuzz-user-macros-replay {}/reduced.json",
                folder.display()
            );
        }
    }
}
pub fn static_obligations() {
    for n in [0, 1, 42, i64::MAX, i64::MIN] {
        for (definition, call, manual, body) in [
            (
                "(%macro $m (rule (?x:expr) (block ?x ?x)))",
                "(%$m (%box.close (%move $owner)))",
                "(block (%box.close (%move $owner)) (%box.close (%move $owner)))",
                format!("(%let $owner (%box.new i64 (i64.const {n})) BODY)"),
            ),
            (
                "(%macro $m (rule (?x:expr) (nop)))",
                "(%$m (%box.close (%move $owner)))",
                "(nop)",
                format!("(%let $owner (%box.new i64 (i64.const {n})) BODY)"),
            ),
            (
                "(%macro $m (rule (?x:expr) (drop (call $host ?x))))",
                "(%$m (i64.const 1))",
                "(drop (call $host (i64.const 1)))",
                "BODY".into(),
            ),
            (
                "(%macro $m (rule (?x:expr) (drop (i32.add ?x (i32.const 0)))))",
                "(%$m (i64.const 1))",
                "(drop (i32.add (i64.const 1) (i32.const 0)))",
                "BODY".into(),
            ),
            (
                "(%macro $m (rule (?x:expr) (throw $Stop ?x)))",
                "(%$m (i64.const 1))",
                "(throw $Stop (i64.const 1))",
                "BODY".into(),
            ),
        ] {
            let source = |macros| {
                format!(
                    "(module (tag $Stop (param i64)) (import \"host\" \"f\" (func $host (param $n i64) (result i64))) {} (func $f (%effect pure) {}))",
                    if macros { definition } else { "" },
                    body.replace("BODY", if macros { call } else { manual })
                )
            };
            let a = wlt::check(&source(true)).unwrap_err();
            let b = wlt::check(&source(false)).unwrap_err();
            assert_eq!(
                a.diagnostic().code,
                b.diagnostic().code,
                "macro: {a}\nmanual: {b}"
            );
        }
    }
}
