use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Const(i32),
    Trace(i32),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Pair(Box<Expr>, Box<Expr>),
    Result(Box<Expr>, bool),
    Choose(Box<Expr>, Box<Expr>, Box<Expr>),
}
impl Expr {
    pub fn has_trace(&self) -> bool {
        matches!(self, Self::Trace(_)) || self.children().iter().any(|e| e.has_trace())
    }
    fn children(&self) -> Vec<Expr> {
        match self {
            Self::Add(a, b) | Self::Mul(a, b) | Self::Pair(a, b) => {
                vec![(**a).clone(), (**b).clone()]
            }
            Self::Result(a, _) => vec![(**a).clone()],
            Self::Choose(a, b, c) => vec![(**a).clone(), (**b).clone(), (**c).clone()],
            _ => vec![],
        }
    }
    fn eval(&self, trace: &mut i32) -> i32 {
        match self {
            Self::Const(n) => *n,
            Self::Trace(n) => {
                *trace = trace.wrapping_mul(31).wrapping_add(*n);
                *n
            }
            Self::Add(a, b) => a.eval(trace).wrapping_add(b.eval(trace)),
            Self::Mul(a, b) => a.eval(trace).wrapping_mul(b.eval(trace)),
            Self::Pair(a, b) => a.eval(trace).wrapping_sub(b.eval(trace)),
            Self::Result(a, _) => a.eval(trace),
            Self::Choose(a, b, c) => {
                if a.eval(trace) & 1 == 0 {
                    b.eval(trace)
                } else {
                    c.eval(trace)
                }
            }
        }
    }
    fn render(&self, short: bool, next: &mut usize) -> String {
        match self {
            Self::Const(n) => format!("(i32.const {n})"),
            Self::Trace(n) => format!("(call $event (i32.const {n}))"),
            Self::Add(a, b) | Self::Mul(a, b) => format!(
                "({} {} {})",
                if matches!(self, Self::Add(..)) {
                    "i32.add"
                } else {
                    "i32.mul"
                },
                a.render(short, next),
                b.render(short, next)
            ),
            Self::Pair(a, b) => {
                let id = *next;
                *next += 1;
                let a = a.render(short, next);
                let b = b.render(short, next);
                let value = format!("(pair.new i32 i32 {a} {b})");
                let body = format!("(i32.sub (local.get $a{id}) (local.get $b{id}))");
                if short {
                    format!("(let ($a{id} $b{id}) {value} {body})")
                } else {
                    format!(
                        "(match (pair i32 i32) (result i32) {value} (case pair ($a{id} $b{id}) {body}))"
                    )
                }
            }
            Self::Result(a, ok) => {
                let id = *next;
                *next += 1;
                format!(
                    "(match (result i32 i32) (result i32) (result.{} i32 i32 {}) (case ok ($r{id}) (local.get $r{id})) (case err ($e{id}) (local.get $e{id})))",
                    if *ok { "ok" } else { "err" },
                    a.render(short, next)
                )
            }
            Self::Choose(a, b, c) => format!(
                "(if (result i32) (i32.eqz (i32.and {} (i32.const 1))) (then {}) (else {}))",
                a.render(short, next),
                b.render(short, next),
                c.render(short, next)
            ),
        }
    }
    fn source(&self, short: bool) -> String {
        let body = self.render(short, &mut 0);
        format!(
            r#"(module (type.alias $Count i32)
          (import "trace" "event" (func $event (param $n i32) (result i32)))
          (import "trace" "get" (func $get (result i32)))
          (func $dead (result (option i64)) (option.none i64))
          (func $main (export "main") (result i64)
            (let $value {body}
              (i64.or (i64.shl (i64.extend_i32_u (call $get)) (i64.const 32))
                (i64.extend_i32_u (local.get $value))))))"#
        )
    }
}
fn random(state: &mut u64) -> u32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*state >> 32) as u32
}
fn generate(state: &mut u64, depth: u32) -> Expr {
    let choice = random(state) % 7;
    if depth == 0 || choice < 2 {
        return if choice == 1 {
            Expr::Trace(random(state) as i32)
        } else {
            Expr::Const(random(state) as i32)
        };
    }
    let a = Box::new(generate(state, depth - 1));
    let b = Box::new(generate(state, depth - 1));
    match choice {
        2 => Expr::Add(a, b),
        3 => Expr::Mul(a, b),
        4 => Expr::Pair(a, b),
        5 => Expr::Choose(a, b, Box::new(generate(state, depth - 1))),
        _ => Expr::Result(a, random(state) % 2 == 0),
    }
}
pub fn reduce(mut input: Expr, mut fails: impl FnMut(&Expr) -> bool) -> Expr {
    loop {
        let mut candidates = vec![Expr::Const(0)];
        candidates.extend(input.children());
        if let Some(next) = candidates.into_iter().find(|e| e != &input && fails(e)) {
            input = next;
        } else {
            return input;
        }
    }
}
fn execute(expr: &Expr, directory: &Path) -> Result<(), String> {
    let mut trace = 0;
    let value = expr.eval(&mut trace);
    let expected = (((trace as u32 as u64) << 32) | value as u32 as u64) as i64;
    for (index, (short, optimize)) in [(true, true), (true, false), (false, true)]
        .into_iter()
        .enumerate()
    {
        let compiled =
            wlt::compile_with_options(&expr.source(short), wlt::CompileOptions { optimize })
                .map_err(|e| format!("compile:{e}"))?;
        let path = directory.join(format!("{index}.wasm"));
        std::fs::write(&path, compiled.wasm).unwrap();
        let out = Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,function-references=y,exceptions=y",
                "--preload",
            ])
            .arg(format!("trace={}", directory.join("trace.wasm").display()))
            .args(["--invoke", "main"])
            .arg(&path)
            .output()
            .unwrap();
        if !out.status.success()
            || String::from_utf8_lossy(&out.stdout).trim() != expected.to_string()
        {
            return Err(format!(
                "wasmtime-{index}:{}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        let out = Command::new("node")
            .arg(directory.join("run.mjs"))
            .arg(&path)
            .output()
            .unwrap();
        if !out.status.success()
            || String::from_utf8_lossy(&out.stdout).trim() != expected.to_string()
        {
            return Err(format!(
                "node-{index}:{}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    Ok(())
}
pub fn run() {
    let seed = std::env::var("WLT_FUZZ_SEED")
        .ok()
        .map(|s| s.parse::<u64>().unwrap())
        .unwrap_or(42);
    let count = std::env::var("WLT_FUZZ_CASES")
        .ok()
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(24);
    assert!((1..=10000).contains(&count));
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("trace.wasm"),wat::parse_str(r#"(module
      (global $n (mut i32) (i32.const 0))
      (func (export "event") (param $v i32) (result i32)
        (global.set $n (i32.add (i32.mul (global.get $n) (i32.const 31)) (local.get $v))) (local.get $v))
      (func (export "get") (result i32) (global.get $n)))"#).unwrap()).unwrap();
    std::fs::write(dir.path().join("run.mjs"),r#"import {readFile} from 'node:fs/promises';
      let hash=0; const {instance}=await WebAssembly.instantiate(await readFile(process.argv[2]),{trace:{event:n=>{hash=(Math.imul(hash,31)+n)|0;return n},get:()=>hash}});
      console.log(instance.exports.main().toString());"#).unwrap();
    let mut state = seed;
    let replay = std::env::var("WLT_FUZZ_REPLAY").ok();
    let cases = if let Some(path) = replay {
        vec![serde_json::from_slice::<Expr>(&std::fs::read(path).unwrap()).unwrap()]
    } else {
        (0..count).map(|_| generate(&mut state, 4)).collect()
    };
    for (index, expr) in cases.into_iter().enumerate() {
        if let Err(error) = execute(&expr, dir.path()) {
            let signature = error.split(':').next().unwrap().to_owned();
            let reduced = reduce(expr, |e| {
                execute(e, dir.path())
                    .is_err_and(|e| e.split(':').next() == Some(signature.as_str()))
            });
            let folder = Path::new("target/fuzz/failures");
            std::fs::create_dir_all(folder).unwrap();
            let path = folder.join(format!(
                "seed-{seed}-case-{index}-{}.json",
                std::process::id()
            ));
            std::fs::write(&path, serde_json::to_vec_pretty(&reduced).unwrap()).unwrap();
            std::fs::write(path.with_extension("meta.json"), serde_json::to_vec_pretty(&serde_json::json!({"seed":seed,"case":index,"failure":error,"signature":signature})).unwrap()).unwrap();
            panic!(
                "{error}; seed {seed}, case {index}; replay {}",
                path.display()
            );
        }
        let text = expr.source(true);
        for _ in 0..8 {
            let mut bytes = text.as_bytes().to_vec();
            let index = random(&mut state) as usize % bytes.len();
            bytes[index] = b"()$;\"012 xyz"[random(&mut state) as usize % 12];
            let source = String::from_utf8(bytes).unwrap();
            assert!(
                std::panic::catch_unwind(|| wlt::compile(&source)).is_ok(),
                "parser panic: seed {seed}: {source}"
            );
        }
    }
}
