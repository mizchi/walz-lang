//! A statically checked, expression-only WAT extension targeting Wasm GC.

mod abi_limits;
mod aliases;
pub mod analysis;
mod applications;
pub mod benchmark;
mod binary;
pub mod bindings;
mod bounds;
mod buffers;
mod byte_access;
pub mod component;
mod construction;
#[doc(hidden)]
pub mod distribution;
pub mod effects;
mod emit;
pub mod execution;
pub mod flow;
pub mod format;
mod frontend;
mod generics;
mod harness;
pub mod inspect;
pub mod interface;
pub mod ir;
mod link;
mod lists;
#[cfg(not(target_family = "wasm"))]
pub mod lsp;
pub mod macros;
mod modules;
mod naming;
pub mod native;
mod native_macros;
mod newtypes;
mod optimize;
pub mod ownership;
mod ownership_graph;
pub mod performance;
pub mod proof;
pub mod race;
pub mod readonly;
pub mod regions;
mod resources;
pub mod review;
pub mod runner;
mod scoped;
mod sexpr;
mod simd;
pub mod source;
mod standard;
#[cfg(test)]
mod standard_catalogue;
mod standard_sources;
mod syntax;
mod task_children;
pub mod threads;
mod type_syntax;
mod typeclasses;
pub mod typing;
pub mod verified;
pub mod wasm;
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
    #[serde(skip_serializing_if = "effects::is_false")]
    pub synchronizes: bool,
    #[serde(skip_serializing_if = "effects::is_false")]
    pub may_block: bool,
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
    /// Checked, monomorphized extension IR, before declaration reachability projection.
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
    check_observed(source, policy, |_, _| ()).map(|(module, ())| module)
}

pub(crate) fn check_observed<T>(
    source: &str,
    policy: &bindings::BindingPolicy,
    observe: impl FnOnce(&ir::Module, &generics::Expanded) -> T,
) -> Result<(ir::Module, T), Error> {
    let root = sexpr::parse(source)?;
    let standard = standard::prepare(source, &root)?;
    let syntax = standard.syntax(root)?;
    let source = standard.source(source);
    let mut syntax = syntax::normalize(source, &syntax)?;
    standard.rewrite(&mut syntax);
    let syntax = modules::expand(source, &syntax).map_err(|e| standard.annotate(e))?;
    let (macros, ordinary) = macros::Environment::prepare(source, &syntax.syntax)
        .map_err(|e| standard.annotate(syntax.annotate(e)))?;
    let (mut generics, mut specializations) =
        generics::start(source, &ordinary).map_err(|e| standard.annotate(syntax.annotate(e)))?;
    let mut observe = Some(observe);
    loop {
        if !macros.definitions.is_empty() || specializations.has_templates() {
            sexpr::identify(&mut generics.syntax);
            macros.clear_edits();
        }
        let inference = specializations.inference();
        let checked: Result<(ir::Module, T), Error> = (|| {
            let aliases = aliases::expand(source, &generics.syntax)?;
            let expanded = scoped::expand(source, &aliases.syntax)?;
            macros.charge_check(source, &expanded)?;
            specializations.charge_check(&expanded)?;
            let (mut module, bounds) = frontend::check(
                source,
                &expanded,
                &generics.bounds,
                &macros,
                &inference,
                (&syntax, &generics.origins),
                &aliases,
            )?;
            interface::validate(source, &mut module)?;
            policy.apply(&mut module);
            binary::apply(&mut module)?;
            bindings::validate(source, &module)?;
            frontend::validate_alias_targets(source, &module, &aliases.targets)?;
            syntax.validate(source, &module, &generics.origins)?;
            resources::validate(source, &module)?;
            bounds::check_effects(source, &module, &bounds)?;
            effects::validate(source, &module)?;
            task_children::validate(source, &module)?;
            ownership::validate(source, &module)?;
            race::validate(source, &module)?;
            benchmark::validate(source, &module)?;
            macros.apply_expansions(&mut generics.syntax);
            inference.apply_resolved(&mut generics);
            generics.macros = macros.report();
            let observation = observe.take().unwrap()(&module, &generics);
            Ok((module, observation))
        })();
        match checked {
            Err(error) if error.diagnostic().code == "E_GENERIC_PENDING" => {
                macros.apply_expansions(&mut generics.syntax);
                specializations
                    .resume_inference(&mut generics, &inference)
                    .map_err(|e| standard.annotate(syntax.annotate(macros.annotate(source, e))))?;
            }
            Err(error) if error.diagnostic().code == "E_MACRO_PENDING" => {
                macros.apply_expansions(&mut generics.syntax);
                let request = macros.take_pending().expect("pending expansion request");
                macros
                    .resume(source, &mut generics.syntax, request)
                    .and_then(|()| specializations.refresh(&mut generics))
                    .map_err(|e| standard.annotate(syntax.annotate(macros.annotate(source, e))))?;
            }
            result => {
                return result.map_err(|error| {
                    standard.annotate(
                        syntax.annotate(generics.annotate(source, macros.annotate(source, error))),
                    )
                });
            }
        }
    }
}

/// Resolve and check walz, infer effects, and emit a validated Wasm GC module.
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
    // Reserved Walz entries must not be discarded as opaque WAT annotations.
    // Otherwise native WAT takes precedence without rewriting its source.
    // The dedicated reader does not decide acceptance here (data strings,
    // stack syntax, numeric indices, and all standard instructions remain valid).
    if !syntax::requested(source)
        && let Ok(wasm) = wat::parse_str(source)
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
    let module = match check(source) {
        Ok(module) => module,
        Err(error) => {
            if error.diagnostic().code == "E_CHECK"
                && !macros::requested(source)
                && !race::requested(source)
                && let Some(wat) = native_macros::expand(source)?
            {
                let wasm = wat::parse_str(&wat).map_err(|e| {
                    Error::message(format!("expanded WAT: {e}")).with_code("E_NATIVE_WAT")
                })?;
                wasmparser::Validator::new()
                    .validate_all(&wasm)
                    .map_err(|e| {
                        Error::message(format!("expanded Wasm: {e}")).with_code("E_NATIVE_WAT")
                    })?;
                return Ok(Compilation {
                    wat,
                    wasm,
                    analysis: Analysis {
                        opaque: true,
                        module_effects: Effects::unknown(),
                        functions: vec![],
                    },
                    ir: None,
                });
            }
            return Err(error);
        }
    };
    if !options.optimize {
        if !module.laws.is_empty() || module.race_free {
            verified::verify(source, &module)?;
        }
        return emit_module(module, false);
    }
    compile_checked(source, module)
}

/// Validate either native WAT or the typed extension syntax.
pub fn validate(source: &str) -> Result<(), Error> {
    if let Ok(module) = check(source) {
        if !module.laws.is_empty() || module.race_free {
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
    if !ir.laws.is_empty() || ir.race_free {
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
    if task_children::enabled(&linked) {
        return Err(
            Error::message("scoped tasks require --target core-js or component".into())
                .with_code("E_TASK_BACKEND"),
        );
    }
    binary::core(&linked)?;
    let wat = emit::emit(&newtypes::lower(&linked));
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
