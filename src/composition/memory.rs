//! A memory index is local to a Core instance. Explicit imports/aliases retain
//! memory identity, while each instantiation allocates fresh defined memories.
//! This is an inspection graph, not a certificate for arbitrary host modules.
use super::*;
use std::sync::Arc;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct MemoryId(pub usize);
#[derive(Clone, Debug, Serialize)]
pub struct Memory {
    pub initial: u64,
    pub maximum: Option<u64>,
    pub shared: bool,
    pub memory64: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct MemoryUse {
    pub instance: String,
    pub index: usize,
    pub identity: MemoryId,
    pub memory: Memory,
}
#[derive(Clone, Debug, Serialize)]
pub struct CanonicalMemory {
    pub component: String,
    pub function: usize,
    pub memory: Option<MemoryId>,
}
#[derive(Default, Serialize)]
pub(super) struct Graph {
    pub uses: Vec<MemoryUse>,
    pub canonical: Vec<CanonicalMemory>,
    pub unresolved: Vec<String>,
    #[serde(skip)]
    next: usize,
    #[serde(skip)]
    instances: usize,
}
#[derive(Clone)]
struct Slot {
    ty: Memory,
    import: Option<(String, String)>,
}
#[derive(Clone)]
struct Module {
    memories: Vec<Slot>,
    exports: BTreeMap<String, usize>,
}
#[derive(Clone)]
struct Definition {
    syntax: Sexp,
    outer: Arc<Scope>,
}
type Instance = BTreeMap<String, MemoryUse>;
#[derive(Clone, Default)]
struct Scope {
    modules: Vec<Option<Module>>,
    module_names: BTreeMap<String, usize>,
    components: Vec<Option<Arc<Definition>>>,
    component_names: BTreeMap<String, usize>,
    instances: Vec<Instance>,
    instance_names: BTreeMap<String, usize>,
    memories: Vec<Option<MemoryUse>>,
    memory_names: BTreeMap<String, usize>,
    outer: Option<Arc<Scope>>,
}
fn second(n: &Sexp, name: &str) -> bool {
    n.head() == Some("core") && n.list().and_then(|p| p.get(1)).and_then(Sexp::atom) == Some(name)
}
fn index(name: &str, names: &BTreeMap<String, usize>) -> Option<usize> {
    name.parse().ok().or_else(|| names.get(name).copied())
}
fn local_name(parts: &[Sexp], at: usize) -> Option<String> {
    parts
        .get(at)
        .and_then(Sexp::atom)
        .filter(|n| n.starts_with('$'))
        .map(str::to_owned)
}
fn insert_name(names: &mut BTreeMap<String, usize>, parts: &[Sexp], at: usize, index: usize) {
    if let Some(name) = local_name(parts, at) {
        names.insert(name, index);
    }
}
fn memory(fields: &[Sexp]) -> Memory {
    let numbers = fields
        .iter()
        .filter_map(Sexp::atom)
        .filter_map(|s| s.replace('_', "").parse().ok())
        .collect::<Vec<u64>>();
    Memory {
        initial: numbers.first().copied().unwrap_or(0),
        maximum: numbers.get(1).copied(),
        shared: fields.iter().any(|n| n.atom() == Some("shared")),
        memory64: fields.iter().any(|n| n.atom() == Some("i64")),
    }
}
fn module(node: &Sexp) -> Module {
    let mut result = Module {
        memories: vec![],
        exports: Default::default(),
    };
    let mut names = BTreeMap::new();
    for n in node.list().unwrap().iter().skip(2) {
        let p = n.list().unwrap_or(&[]);
        if n.head() == Some("memory") {
            insert_name(&mut names, p, 1, result.memories.len());
            for e in &p[1..] {
                if e.head() == Some("export")
                    && let Some(name) = e.list().and_then(|p| p.get(1)).and_then(Sexp::text)
                {
                    result.exports.insert(name.into(), result.memories.len());
                }
            }
            result.memories.push(Slot {
                ty: memory(&p[1..]),
                import: None,
            });
        } else if n.head() == Some("import") {
            if let Some(m) = p.get(3).filter(|n| n.head() == Some("memory")) {
                let mp = m.list().unwrap();
                insert_name(&mut names, mp, 1, result.memories.len());
                result.memories.push(Slot {
                    ty: memory(&mp[1..]),
                    import: Some((
                        p[1].text().unwrap_or("").into(),
                        p[2].text().unwrap_or("").into(),
                    )),
                });
            }
        } else if n.head() == Some("export")
            && let Some(e) = p.get(2).filter(|n| n.head() == Some("memory"))
            && let (Some(name), Some(id)) = (
                p[1].text(),
                e.list()
                    .unwrap()
                    .get(1)
                    .and_then(Sexp::atom)
                    .and_then(|n| index(n, &names)),
            )
        {
            result.exports.insert(name.into(), id);
        }
    }
    result
}
impl Scope {
    fn module_ref(&self, n: &Sexp) -> Option<Module> {
        let p = n.list()?;
        let at = usize::from(n.head() == Some("core")) + 1;
        self.modules
            .get(index(p.get(at)?.atom()?, &self.module_names)?)?
            .clone()
    }
    fn memory_ref(&self, n: &Sexp) -> Option<MemoryUse> {
        let p = n.list()?;
        if p.len() == 3 {
            let instance = self
                .instances
                .get(index(p[1].atom()?, &self.instance_names)?)?;
            instance.get(p[2].text()?).cloned()
        } else {
            self.memories
                .get(index(p.get(1)?.atom()?, &self.memory_names)?)?
                .clone()
        }
    }
    fn instance_ref(&self, n: &Sexp) -> Instance {
        let Some(p) = n.list() else {
            return Default::default();
        };
        if let Some(id) = p
            .get(1)
            .and_then(Sexp::atom)
            .and_then(|n| index(n, &self.instance_names))
        {
            return self.instances.get(id).cloned().unwrap_or_default();
        }
        p.iter()
            .filter(|n| n.head() == Some("export"))
            .filter_map(|n| {
                let p = n.list()?;
                Some((p.get(1)?.text()?.into(), self.memory_ref(p.get(2)?)?))
            })
            .collect()
    }
}
pub(super) fn inspect(text: &str) -> Result<Graph, Error> {
    let root = crate::sexpr::parse(text)?;
    let mut graph = Graph::default();
    walk(
        &root,
        "root",
        Scope::default(),
        &BTreeMap::new(),
        &mut graph,
        0,
    )?;
    Ok(graph)
}
fn walk(
    root: &Sexp,
    path: &str,
    mut scope: Scope,
    arguments: &BTreeMap<String, Module>,
    graph: &mut Graph,
    depth: usize,
) -> Result<(), Error> {
    if depth > 64 || graph.instances > 4096 || graph.uses.len() > 16384 {
        return Err(error("component memory graph limit"));
    }
    let mut function = 0;
    for n in root.list().unwrap_or(&[]).iter().skip(1) {
        if graph.instances > 4096 || graph.uses.len() > 16384 {
            return Err(error("component memory graph limit"));
        }
        let p = n.list().unwrap_or(&[]);
        if second(n, "module") {
            insert_name(&mut scope.module_names, p, 2, scope.modules.len());
            scope.modules.push(Some(module(n)));
        } else if n.head() == Some("component") {
            insert_name(&mut scope.component_names, p, 1, scope.components.len());
            scope.components.push(Some(Arc::new(Definition {
                syntax: n.clone(),
                outer: Arc::new(scope.clone()),
            })));
        } else if n.head() == Some("import") {
            if let Some(d) = p.get(2).filter(|n| second(n, "module")) {
                let d = d.list().unwrap();
                insert_name(&mut scope.module_names, d, 2, scope.modules.len());
                scope
                    .modules
                    .push(p[1].text().and_then(|name| arguments.get(name)).cloned());
            } else if let Some(d) = p.get(2).filter(|n| n.head() == Some("component")) {
                insert_name(
                    &mut scope.component_names,
                    d.list().unwrap(),
                    1,
                    scope.components.len(),
                );
                scope.components.push(None);
            }
        } else if second(n, "instance") {
            let id = scope.instances.len();
            graph.instances += 1;
            insert_name(&mut scope.instance_names, p, 2, id);
            let mut instance = Instance::new();
            if let Some(inst) = p.iter().find(|n| n.head() == Some("instantiate")) {
                let ip = inst.list().unwrap();
                let target = ip
                    .get(1)
                    .and_then(Sexp::atom)
                    .and_then(|name| index(name, &scope.module_names))
                    .and_then(|i| scope.modules.get(i))
                    .and_then(Option::as_ref);
                if let Some(module) = target {
                    let args = ip
                        .iter()
                        .filter(|n| n.head() == Some("with"))
                        .filter_map(|n| {
                            let p = n.list()?;
                            Some((p.get(1)?.text()?.to_owned(), scope.instance_ref(p.get(2)?)))
                        })
                        .collect::<BTreeMap<_, _>>();
                    let mut uses = vec![];
                    for (index, slot) in module.memories.iter().enumerate() {
                        let imported = slot
                            .import
                            .as_ref()
                            .and_then(|(ns, name)| args.get(ns)?.get(name))
                            .cloned();
                        let identity = if slot.import.is_none() {
                            let id = MemoryId(graph.next);
                            graph.next += 1;
                            Some(id)
                        } else {
                            imported.as_ref().map(|m| m.identity)
                        };
                        if let Some(identity) = identity {
                            let usage = MemoryUse {
                                instance: format!("{path}/core-{id}"),
                                index,
                                identity,
                                memory: imported
                                    .map(|u| u.memory)
                                    .unwrap_or_else(|| slot.ty.clone()),
                            };
                            graph.uses.push(usage.clone());
                            uses.push(Some(usage));
                        } else {
                            graph
                                .unresolved
                                .push(format!("{path}/core-{id}/memory-{index}"));
                            uses.push(None);
                        }
                    }
                    for (name, index) in &module.exports {
                        if let Some(Some(m)) = uses.get(*index) {
                            instance.insert(name.clone(), m.clone());
                        }
                    }
                } else {
                    graph
                        .unresolved
                        .push(format!("{path}/core-{id}: imported module"));
                }
            } else {
                instance = scope.instance_ref(n);
            }
            scope.instances.push(instance);
        } else if n.head() == Some("instance") {
            if let Some(inst) = p.iter().find(|n| n.head() == Some("instantiate")) {
                graph.instances += 1;
                let ip = inst.list().unwrap();
                let target = ip
                    .get(1)
                    .and_then(Sexp::atom)
                    .and_then(|name| index(name, &scope.component_names))
                    .and_then(|i| scope.components.get(i))
                    .and_then(Option::as_ref)
                    .cloned();
                if let Some(d) = target {
                    let args = ip
                        .iter()
                        .filter(|n| n.head() == Some("with"))
                        .filter_map(|n| {
                            let p = n.list()?;
                            Some((p.get(1)?.text()?.into(), scope.module_ref(p.get(2)?)?))
                        })
                        .collect();
                    walk(
                        &d.syntax,
                        &format!("{path}/component-{}", graph.instances),
                        Scope {
                            outer: Some(d.outer.clone()),
                            ..Scope::default()
                        },
                        &args,
                        graph,
                        depth + 1,
                    )?;
                } else {
                    graph.unresolved.push(format!("{path}: imported component"));
                }
            }
        } else if n.head() == Some("alias") {
            if p.get(1).and_then(Sexp::atom) == Some("outer") {
                let count = p
                    .get(2)
                    .and_then(Sexp::atom)
                    .and_then(|n| n.parse::<usize>().ok())
                    .unwrap_or(0);
                let mut outer = Some(&scope);
                for _ in 0..count {
                    outer = outer.and_then(|s| s.outer.as_deref());
                }
                if let Some(d) = p.last().filter(|n| second(n, "module")) {
                    let target = outer.and_then(|s| {
                        s.module_ref(&Sexp {
                            kind: Kind::List(vec![p[0].clone(), p[3].clone()]),
                            span: n.span,
                            context: Default::default(),
                        })
                    });
                    insert_name(
                        &mut scope.module_names,
                        d.list().unwrap(),
                        2,
                        scope.modules.len(),
                    );
                    scope.modules.push(target);
                }
            } else if p.get(1).and_then(Sexp::atom) == Some("core")
                && p.get(2).and_then(Sexp::atom) == Some("export")
                && let Some(d) = p.last().filter(|n| second(n, "memory"))
            {
                let memory = p
                    .get(3)
                    .and_then(Sexp::atom)
                    .and_then(|n| index(n, &scope.instance_names))
                    .and_then(|i| scope.instances.get(i))
                    .and_then(|i| i.get(p.get(4)?.text()?))
                    .cloned();
                insert_name(
                    &mut scope.memory_names,
                    d.list().unwrap(),
                    2,
                    scope.memories.len(),
                );
                scope.memories.push(memory);
            }
        }
        if n.head() == Some("func") || second(n, "func") {
            if let Some(canon) = p.iter().find(|n| n.head() == Some("canon"))
                && let Some(m) = canon
                    .list()
                    .unwrap()
                    .iter()
                    .find(|n| n.head() == Some("memory"))
            {
                let memory = scope.memory_ref(m).map(|m| m.identity);
                if memory.is_none() {
                    graph
                        .unresolved
                        .push(format!("{path}/canonical-{function}: memory context"));
                }
                graph.canonical.push(CanonicalMemory {
                    component: path.into(),
                    function,
                    memory,
                });
            }
            function += 1;
        }
    }
    Ok(())
}
