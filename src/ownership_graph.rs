//! Availability certificates for structured jumps and exceptional edges.
//! The graph is checked again by the independent Lean kernel. IR-to-graph
//! extraction, exceptional effect inference and Wasm lowering remain TCB.
use crate::{Error, ir::*, ownership::Kinds};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Step {
    Nop,
    Bind { id: LocalId },
    Take { id: LocalId },
    Absent { id: LocalId },
    Finish,
}
#[derive(Clone, Debug, Serialize)]
pub struct Node {
    pub span: Span,
    #[serde(flatten)]
    pub step: Step,
    pub next: Vec<usize>,
    pub context: Option<Vec<LocalId>>,
    #[serde(skip)]
    loop_entry: bool,
}
#[derive(Debug, Serialize)]
pub struct Graph {
    pub entry: usize,
    pub nodes: Vec<Node>,
}
#[derive(Clone)]
struct Target {
    node: usize,
    scopes: Vec<LocalId>,
}
#[derive(Clone, Default)]
struct Env {
    scopes: Vec<LocalId>,
    labels: BTreeMap<LoopId, Target>,
    handlers: Vec<(Option<TagId>, Target)>,
}
struct Builder<'a> {
    source: &'a str,
    function: &'a Function,
    kinds: &'a Kinds,
    effects: &'a [crate::effects::EffectSet],
    nodes: Vec<Node>,
    next_slot: usize,
    temporaries: BTreeMap<LocalId, Span>,
}
impl Builder<'_> {
    fn node(&mut self, span: Span, step: Step, next: Vec<usize>) -> usize {
        let id = self.nodes.len();
        self.nodes.push(Node {
            span,
            step,
            next,
            context: None,
            loop_entry: false,
        });
        id
    }
    fn edge(&mut self, span: Span, env: &Env, target: &Target) -> usize {
        let mut next = target.node;
        for id in env
            .scopes
            .iter()
            .rev()
            .filter(|id| !target.scopes.contains(id))
        {
            next = self.node(span, Step::Absent { id: *id }, vec![next]);
        }
        next
    }
    fn raising(&mut self, span: Span, env: &Env, tag: Option<TagId>) -> usize {
        let mut targets = vec![];
        let mut seen = BTreeSet::new();
        let mut caught = false;
        for (handled, target) in &env.handlers {
            if tag.is_some() {
                if handled.is_none() || *handled == tag {
                    targets.push(target.clone());
                    caught = true;
                    break;
                }
            } else if let Some(tag) = handled {
                if seen.insert(*tag) {
                    targets.push(target.clone());
                }
            } else {
                targets.push(target.clone());
                caught = true;
                break;
            }
        }
        let mut edges = targets
            .iter()
            .map(|t| self.edge(span, env, t))
            .collect::<Vec<_>>();
        if !caught {
            edges.push(self.node(span, Step::Finish, vec![]));
        }
        self.node(span, Step::Nop, edges)
    }
    // Owned operands stay live in temporary slots until every operand has been
    // evaluated. A later operand throwing must not abandon an earlier move.
    fn operands(
        &mut self,
        children: Vec<&Expr>,
        env: &Env,
        commit: impl FnOnce(&mut Self, &Env) -> usize,
    ) -> Result<usize, Error> {
        let mut environments = vec![];
        let mut slots = vec![];
        let mut active = env.clone();
        for e in &children {
            environments.push(active.clone());
            let slot = if self.kinds.is_owned(&e.ty) {
                let id = LocalId(self.next_slot);
                self.next_slot += 1;
                self.temporaries.insert(id, e.span);
                active.scopes.push(id);
                Some(id)
            } else {
                None
            };
            slots.push(slot);
        }
        let mut next = commit(self, &active);
        for id in slots.iter().flatten().rev() {
            next = self.node(self.temporaries[id], Step::Take { id: *id }, vec![next]);
        }
        for i in (0..children.len()).rev() {
            if let Some(id) = slots[i] {
                next = self.node(children[i].span, Step::Bind { id }, vec![next]);
            }
            next = self.expr(children[i], next, &environments[i])?;
        }
        Ok(next)
    }
    fn expr(&mut self, e: &Expr, next: usize, env: &Env) -> Result<usize, Error> {
        use ExprKind::*;
        if self.nodes.len() > 1_000_000 {
            return Err(Error::at(
                self.source,
                e.span,
                "ownership graph limit exceeded",
            ));
        }
        let span = e.span;
        Ok(match &e.kind {
            Move(id) => self.node(span, Step::Take { id: *id }, vec![next]),
            Let { local, value, body } => {
                if self.kinds.is_owned(&self.function.locals[local.0].ty) {
                    let end = self.node(span, Step::Absent { id: *local }, vec![next]);
                    let mut nested = env.clone();
                    nested.scopes.push(*local);
                    let body = self.expr(body, end, &nested)?;
                    let bind = self.node(span, Step::Bind { id: *local }, vec![body]);
                    self.expr(value, bind, env)?
                } else {
                    let body = self.expr(body, next, env)?;
                    self.expr(value, body, env)?
                }
            }
            Using { value, body, .. } => {
                let b = self.expr(body, next, env)?;
                self.expr(value, b, env)?
            }
            Block(values) => {
                let mut entry = next;
                for v in values.iter().rev() {
                    entry = self.expr(v, entry, env)?;
                }
                entry
            }
            Label { label, body } => {
                let mut nested = env.clone();
                nested.labels.insert(
                    *label,
                    Target {
                        node: next,
                        scopes: env.scopes.clone(),
                    },
                );
                self.expr(body, next, &nested)?
            }
            If {
                condition,
                then_body,
                else_body,
            } => {
                let a = self.expr(then_body, next, env)?;
                let b = self.expr(else_body, next, env)?;
                let branch = self.node(span, Step::Nop, vec![a, b]);
                self.expr(condition, branch, env)?
            }
            Match { subject, arms, .. } => {
                let mut starts = vec![];
                for arm in arms {
                    let mut nested = env.clone();
                    let owners = arm
                        .bindings
                        .iter()
                        .filter(|id| self.kinds.is_owned(&self.function.locals[id.0].ty))
                        .copied()
                        .collect::<Vec<_>>();
                    nested.scopes.extend(&owners);
                    let mut exit = next;
                    for id in &owners {
                        exit = self.node(span, Step::Absent { id: *id }, vec![exit]);
                    }
                    let mut start = self.expr(&arm.body, exit, &nested)?;
                    for id in owners.iter().rev() {
                        start = self.node(span, Step::Bind { id: *id }, vec![start]);
                    }
                    starts.push(start);
                }
                let branch = self.node(span, Step::Nop, starts);
                self.expr(subject, branch, env)?
            }
            Loop {
                label,
                bindings,
                body,
            } => {
                let entry = self.node(span, Step::Nop, vec![]);
                self.nodes[entry].loop_entry = true;
                let owners = bindings
                    .iter()
                    .filter(|(id, _)| self.kinds.is_owned(&self.function.locals[id.0].ty))
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>();
                let mut nested = env.clone();
                nested.labels.insert(
                    *label,
                    Target {
                        node: entry,
                        scopes: env.scopes.clone(),
                    },
                );
                nested.scopes.extend(&owners);
                let mut exit = next;
                for id in &owners {
                    exit = self.node(span, Step::Absent { id: *id }, vec![exit]);
                }
                let mut start = self.expr(body, exit, &nested)?;
                for id in owners.iter().rev() {
                    start = self.node(span, Step::Bind { id: *id }, vec![start]);
                }
                self.nodes[entry].next = vec![start];
                self.operands(bindings.iter().map(|(_, v)| v).collect(), env, |_, _| entry)?
            }
            Recur { label, assignments } => {
                let target = env.labels.get(label).expect("typed loop").clone();
                self.operands(
                    assignments.iter().map(|(_, v)| v).collect(),
                    env,
                    |b, active| b.edge(span, active, &target),
                )?
            }
            Branch {
                label, condition, ..
            } => {
                let target = env.labels.get(label).expect("typed label").clone();
                self.operands(e.children(), env, |b, active| {
                    let exit = b.edge(span, active, &target);
                    if condition.is_some() {
                        b.node(span, Step::Nop, vec![exit, next])
                    } else {
                        exit
                    }
                })?
            }
            Return(_) | Unreachable => {
                self.operands(e.children(), env, |b, _| b.node(span, Step::Finish, vec![]))?
            }
            Throw { tag, .. } => self.operands(e.children(), env, |b, active| {
                b.raising(span, active, Some(*tag))
            })?,
            Try { body, handlers } => {
                let mut nested = env.clone();
                let mut entries = vec![];
                for arm in handlers {
                    let entry = self.expr(&arm.body, next, env)?;
                    entries.push((
                        arm.tag,
                        Target {
                            node: entry,
                            scopes: env.scopes.clone(),
                        },
                    ));
                }
                entries.extend(nested.handlers);
                nested.handlers = entries;
                self.expr(body, next, &nested)?
            }
            TaskScope { body } => self.expr(body, next, env)?,
            Call { function, .. } => {
                let effect = self.effects[function.0].clone();
                self.operands(e.children(), env, |b, active| {
                    let mut edges = vec![next];
                    for tag in &effect.throws {
                        edges.push(b.raising(span, active, Some(*tag)));
                    }
                    if effect.throws_unknown {
                        edges.push(b.raising(span, active, None));
                    }
                    b.node(span, Step::Nop, edges)
                })?
            }
            _ => self.operands(e.children(), env, |_, _| next)?,
        })
    }
}

#[derive(Clone)]
struct State {
    live: BTreeSet<LocalId>,
    moved: BTreeMap<LocalId, BTreeSet<Span>>,
}
pub fn extract(
    source: &str,
    function: &Function,
    kinds: &Kinds,
    effects: &[crate::effects::EffectSet],
    inputs: &[LocalId],
) -> Result<Graph, Error> {
    let body = function.body.as_ref().unwrap();
    let mut b = Builder {
        source,
        function,
        kinds,
        effects,
        nodes: vec![],
        next_slot: function.locals.len(),
        temporaries: BTreeMap::new(),
    };
    let finish = b.node(body.span, Step::Finish, vec![]);
    let entry = b.expr(body, finish, &Env::default())?;
    let mut states = vec![None::<State>; b.nodes.len()];
    states[entry] = Some(State {
        live: inputs.iter().copied().collect(),
        moved: BTreeMap::new(),
    });
    let mut queue = VecDeque::from([entry]);
    let mut steps = 0;
    while let Some(index) = queue.pop_front() {
        steps += 1;
        if steps > 2_000_000 {
            return Err(Error::at(
                source,
                body.span,
                "ownership analysis limit exceeded",
            ));
        }
        let mut state = states[index].clone().unwrap();
        let node = &b.nodes[index];
        let fail = |id: LocalId, message: &str| {
            let mut error = Error::at(source, node.span, message).with_code("E_OWNERSHIP");
            if let Some(local) = function.locals.get(id.0) {
                error = error.related(source, local.span, format!("`{}` bound here", local.name));
            } else {
                error = error
                    .note("owned temporary argument is live while later operands are evaluated");
            }
            error
        };
        match node.step {
            Step::Nop => {}
            Step::Bind { id } => {
                if !state.live.insert(id) {
                    return Err(fail(id, "owned binding is already live"));
                }
                state.moved.remove(&id);
            }
            Step::Take { id } => {
                if !state.live.remove(&id) {
                    let mut error = fail(id, "owned value is already moved or unavailable")
                        .with_code("E_OWNERSHIP_MOVED");
                    if let Some(origins) = state.moved.get(&id) {
                        for s in origins.iter().take(3) {
                            error = error.related(source, *s, "value moved here");
                        }
                    }
                    return Err(error);
                }
                state.moved.insert(id, BTreeSet::from([node.span]));
            }
            Step::Absent { id } => {
                if state.live.contains(&id) {
                    return Err(fail(id, "unconsumed owned binding at scope exit"));
                }
            }
            Step::Finish => {
                if let Some(id) = state.live.first() {
                    return Err(fail(
                        *id,
                        "unconsumed owned value on normal or exceptional exit",
                    ));
                }
            }
        }
        for target in &node.next {
            if let Some(previous) = &mut states[*target] {
                if previous.live != state.live {
                    return Err(Error::at(
                        source,
                        b.nodes[*target].span,
                        if b.nodes[*target].loop_entry {
                            "loop ownership contract is not restored before branch"
                        } else {
                            "branch ownership states differ"
                        },
                    )
                    .with_code("E_OWNERSHIP"));
                }
                let mut changed = false;
                for (id, spans) in &state.moved {
                    let into = previous.moved.entry(*id).or_default();
                    let n = into.len();
                    into.extend(spans);
                    changed |= into.len() != n;
                }
                if changed {
                    queue.push_back(*target);
                }
            } else {
                states[*target] = Some(state.clone());
                queue.push_back(*target);
            }
        }
    }
    for (node, state) in b.nodes.iter_mut().zip(states) {
        node.context = state.map(|s| s.live.into_iter().collect());
    }
    Ok(Graph {
        entry,
        nodes: b.nodes,
    })
}
