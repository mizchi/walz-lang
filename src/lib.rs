//! A statically checked, expression-only WAT extension targeting Wasm GC.

mod aliases;
pub mod analysis;
pub mod bindings;
mod buffers;
pub mod component;
pub mod effects;
mod emit;
pub mod flow;
pub mod format;
mod frontend;
pub mod ir;
mod link;
mod modules;
pub mod native;
mod optimize;
pub mod ownership;
pub mod proof;
pub mod regions;
mod resources;
pub mod runner;
mod scoped;
mod sexpr;
pub mod source;
pub mod typing;
pub mod verified;
pub mod wit;
pub mod wit_bindgen;

use serde::Serialize;
pub mod diagnostic;
pub use diagnostic::Error;

/// Conservative possible effects, excluding external resource limits.
/// Immutable GC field reads are not mutable-memory effects.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct Effects {
    pub reads: bool,
    pub writes: bool,
    pub allocates: bool,
    pub may_trap: bool,
    /// May execute a throw, including exceptions caught within this function.
    pub may_throw: bool,
    /// Logical suspension/reentrancy, distinct from GC pauses or OS scheduling.
    pub may_suspend: bool,
    pub may_diverge: bool,
    pub host: bool,
}

/// One function or nominal closure's captures, direct calls, and transitive effects.
#[derive(Debug, Serialize)]
pub struct FunctionSummary {
    pub name: String,
    pub captures: Vec<String>,
    pub calls: Vec<String>,
    pub effects: Effects,
    pub semantic_effects: effects::EffectSummary,
    pub declared_effects: Option<effects::EffectSummary>,
    pub memory: regions::MemorySummary,
    pub facts: Vec<flow::ExpressionFacts>,
}

#[derive(Debug, Serialize)]
pub struct Analysis {
    /// Full structured facts are unavailable for arbitrary native WAT.
    pub opaque: bool,
    pub module_effects: Effects,
    pub functions: Vec<FunctionSummary>,
}

/// A checked module and the analysis and executable formats derived from it.
pub struct Compilation {
    pub wat: String,
    pub wasm: Vec<u8>,
    pub analysis: Analysis,
    /// Complete checked extension IR, before declaration reachability projection.
    /// Native WAT outside that subset is validated but has no source expression IR.
    pub ir: Option<ir::Module>,
}

/// Optimization switches for differential testing; source validation is unchanged.
#[derive(Clone, Copy, Debug)]
pub struct CompileOptions {
    pub optimize: bool,
}
impl Default for CompileOptions {
    fn default() -> Self {
        Self { optimize: true }
    }
}

/// Resolve names and check types and effect contracts, returning typed IR.
/// Does not lower the checked IR or invoke a Wasm runtime.
pub fn check(source: &str) -> Result<ir::Module, Error> {
    check_with_policy(source, &bindings::BindingPolicy::default())
}

pub fn check_with_policy(
    source: &str,
    policy: &bindings::BindingPolicy,
) -> Result<ir::Module, Error> {
    let syntax = sexpr::parse(source)?;
    let syntax = modules::expand(source, &syntax)?;
    let aliases = aliases::expand(source, &syntax.syntax)?;
    let expanded = scoped::expand(source, &aliases.syntax)?;
    let mut module = frontend::check(source, &expanded)?;
    policy.apply(&mut module);
    frontend::validate_alias_targets(source, &module, &aliases.targets)?;
    syntax.validate(source, &module)?;
    bindings::validate(source, &module)?;
    resources::validate(source, &module)?;
    effects::validate(source, &module)?;
    ownership::validate(source, &module)?;
    Ok(module)
}

/// Resolve and check wlt, infer effects, and emit a validated Wasm GC module.
pub fn compile(source: &str) -> Result<Compilation, Error> {
    compile_with_options(source, CompileOptions::default())
}

pub fn compile_with_policy(
    source: &str,
    policy: &bindings::BindingPolicy,
) -> Result<Compilation, Error> {
    if policy.imports.is_empty() {
        return compile(source);
    }
    compile_checked(source, check_with_policy(source, policy)?)
}

pub fn compile_with_options(source: &str, options: CompileOptions) -> Result<Compilation, Error> {
    // Native WAT takes precedence: do not reinterpret or rewrite valid WAT.
    // The dedicated reader does not decide acceptance here (data strings,
    // stack syntax, numeric indices, and all standard instructions remain valid).
    if let Ok(wasm) = wat::parse_str(source)
        && wasmparser::Parser::is_core_wasm(&wasm)
        && wasmparser::Validator::new().validate_all(&wasm).is_ok()
    {
        let ir = check(source).ok();
        let analysis = ir
            .as_ref()
            .map(analysis::analyze)
            .unwrap_or_else(|| Analysis {
                opaque: true,
                module_effects: Effects::unknown(),
                functions: vec![],
            });
        return Ok(Compilation {
            wat: source.to_owned(),
            wasm,
            analysis,
            ir,
        });
    }
    let module = check(source)?;
    if !options.optimize {
        if !module.laws.is_empty() {
            verified::verify(source, &module)?;
        }
        return emit_module(module, false);
    }
    compile_checked(source, module)
}

/// Validate either native WAT or the typed extension syntax.
pub fn validate(source: &str) -> Result<(), Error> {
    if let Ok(module) = check(source) {
        if !module.laws.is_empty() {
            verified::verify(source, &module)?;
        }
        return Ok(());
    }
    compile(source).map(|_| ())
}

/// Expand extensions into standard WAT; native WAT is returned unchanged.
pub fn expand(source: &str) -> Result<String, Error> {
    Ok(compile(source)?.wat)
}

pub(crate) fn compile_checked(source: &str, ir: ir::Module) -> Result<Compilation, Error> {
    if !ir.laws.is_empty() {
        verified::verify(source, &ir)?;
    }
    emit_checked(ir)
}

pub(crate) fn emit_checked(ir: ir::Module) -> Result<Compilation, Error> {
    emit_module(ir, true)
}

fn emit_module(ir: ir::Module, optimize: bool) -> Result<Compilation, Error> {
    let analysis = analysis::analyze(&ir);
    let linked = if optimize {
        link::project(&optimize::simplify(&ir))
    } else {
        ir.clone()
    };
    let wat = emit::emit(&linked);
    let wasm =
        wat::parse_str(&wat).map_err(|error| Error::message(format!("generated WAT: {error}")))?;
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .map_err(|error| Error::message(format!("generated Wasm: {error}")))?;
    Ok(Compilation {
        wat,
        wasm,
        analysis,
        ir: Some(ir),
    })
}
