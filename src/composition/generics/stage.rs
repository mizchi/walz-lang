//! A type query may inspect a preceding, validated native graph. Compilation
//! never instantiates that graph, runs a start function or executes metadata.
use super::*;
use crate::component::bound_imports::BoundImports;
pub(super) struct Context<'a> {
    pub source: &'a str,
    pub root: &'a Sexp,
    pub policy: &'a BindingPolicy,
    pub depth: usize,
}
impl Context<'_> {
    pub(super) fn connections(
        &self,
        request: &Sexp,
        preceding: &[Sexp],
        namespaces: &BTreeSet<String>,
        budget: &mut Budget,
    ) -> Result<BoundImports, Error> {
        let Self {
            source,
            root,
            policy,
            depth,
        } = self;
        let mut binds = vec![];
        let mut instances = vec![];
        // Parse all declarations through the same contract as final wiring, even
        // though this probe exports only the namespaces the arguments depend on.
        for n in root
            .list()
            .unwrap()
            .iter()
            .filter(|n| n.head() == Some("@bind"))
        {
            super::super::bindings::declaration(n, &mut binds, &mut instances)?;
        }
        binds.retain(|b| namespaces.contains(&b.module));
        instances.retain(|b| namespaces.contains(&b.module));
        if binds.is_empty() && instances.is_empty() {
            return Ok(BoundImports {
                resolve: Default::default(),
                interfaces: Default::default(),
                functions: Default::default(),
                types: Default::default(),
            });
        }
        let raw = preceding
            .iter()
            .skip(1)
            .filter(|n| {
                !matches!(
                    n.head(),
                    Some(
                        "@lib"
                            | "@use"
                            | "@file.use"
                            | "@file.namespace"
                            | "@world"
                            | "@bind"
                            | "@expose"
                            | "export"
                    )
                )
            })
            .map(render)
            .collect::<Vec<_>>()
            .join("\n");
        let phase_error = |e: Error| {
            Error::at(source, request.span,
        format!("constructor type query requires a preceding, independently valid connection; forward or cyclic dependencies are not available: {}", e.diagnostic().message))
        .with_code("E_COMPONENT_TYPE_PHASE")
        };
        let compiled =
            super::super::compile_inner(&format!("(component {raw})"), policy, budget, depth + 1)
                .map_err(phase_error)?;
        let root = crate::sexpr::parse(&compiled.wat)?;
        let raw = root.list().unwrap()[1..]
            .iter()
            .map(render)
            .collect::<Vec<_>>();
        super::super::bindings::resolve(&raw, &Default::default(), &mut binds, &instances)
            .map_err(phase_error)
    }
}
