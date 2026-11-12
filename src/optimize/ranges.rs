//! Range-check elimination on the normal path of checked IR. This is separate
//! from diagnostic may facts: immutable snapshots, explicit guards, and prior
//! successful accesses are the only premises. GC engine checks remain intact.
use super::{
    inline,
    range_facts::{Arena, Compare, Facts, Node, Term},
};
use crate::ir::*;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
struct Value {
    term: Term,
    length: Option<Term>,
    fields: Arc<BTreeMap<(TypeId, usize), Value>>,
}
impl Value {
    fn new(term: Term) -> Self {
        Self {
            term,
            length: None,
            fields: Arc::new(BTreeMap::new()),
        }
    }
    fn length(&self, arena: &mut Arena) -> Term {
        self.length
            .unwrap_or_else(|| arena.make(Node::Len(self.term)))
    }
}
#[derive(Clone)]
struct State {
    locals: Vec<Value>,
    facts: Facts,
    normal: bool,
}
impl State {
    fn assume(&mut self, condition: Term, truth: bool, arena: &Arena) {
        self.facts.assume(condition, truth, arena);
        self.normal &= !self.facts.impossible;
    }
}

pub(super) fn simplify(module: &Module) -> Module {
    let kinds = crate::ownership::Kinds::new(module);
    let plan = inline::Plan::new(module, &kinds);
    let mut out = module.clone();
    for (index, f) in out.functions.iter_mut().enumerate() {
        if !inline::shared(f, &kinds) {
            continue;
        }
        let Some(body) = &f.body else { continue };
        // Bound analysis work on large synthetic functions and type-graph tests.
        let mut pending = vec![body];
        let mut count = 0;
        while let Some(e) = pending.pop() {
            count += 1;
            if count > 4096 {
                break;
            }
            pending.extend(e.children());
        }
        if count > 4096 || f.locals.len() > 2048 {
            continue;
        }
        let original = f.clone();
        *f = plan.expand(module, index);
        let mut visitor = Visitor {
            module,
            arena: Arena::default(),
            changed: false,
        };
        let mut state = State {
            locals: f.locals.iter().map(|_| visitor.unknown()).collect(),
            facts: Facts::default(),
            normal: true,
        };
        visitor.eval(f.body.as_mut().unwrap(), &mut state);
        if !visitor.changed {
            *f = original;
        }
    }
    out
}

struct Visitor<'a> {
    module: &'a Module,
    arena: Arena,
    changed: bool,
}
impl Visitor<'_> {
    fn unknown(&mut self) -> Value {
        Value::new(self.arena.fresh())
    }
    fn constant(&mut self, n: u32) -> Value {
        Value::new(self.arena.constant(n))
    }
    fn join(&mut self, a: State, b: State) -> State {
        if !a.normal {
            return b;
        }
        if !b.normal {
            return a;
        }
        State {
            locals: a
                .locals
                .into_iter()
                .zip(b.locals)
                .map(|(a, b)| if a.term == b.term { a } else { self.unknown() })
                .collect(),
            facts: Facts::join(&a.facts, &b.facts),
            normal: true,
        }
    }
    fn numeric(&mut self, op: NumericOp, values: &[Value]) -> Value {
        use NumericOp::*;
        if op.signature().output != NumType::I32 {
            return self.unknown();
        }
        let a = values[0].term;
        let b = values
            .get(1)
            .map(|v| v.term)
            .unwrap_or_else(|| self.arena.constant(0));
        let node = match op {
            I32Add => Node::Add(a, b),
            I32Sub => Node::Sub(a, b),
            I32Eqz => Node::Cmp(Compare::Eq, a, b),
            I32LtU => Node::Cmp(Compare::Lt, a, b),
            I32LeU => Node::Cmp(Compare::Le, a, b),
            I32GtU => Node::Cmp(Compare::Lt, b, a),
            I32GeU => Node::Cmp(Compare::Le, b, a),
            I32Eq => Node::Cmp(Compare::Eq, a, b),
            I32Ne => Node::Cmp(Compare::Ne, a, b),
            _ if op.signature().input == NumType::I32 => {
                Node::Op(format!("{op:?}"), values.iter().map(|v| v.term).collect())
            }
            _ => return self.unknown(),
        };
        Value::new(self.arena.make(node))
    }
    fn eval(&mut self, e: &mut Expr, s: &mut State) -> Value {
        use ExprKind::*;
        if !s.normal {
            return self.unknown();
        }
        match &mut e.kind {
            Local(id) => s.locals[id.0].clone(),
            Const {
                ty: NumType::I32,
                literal,
            } => crate::flow::literal(literal)
                .map(|n| self.constant(n))
                .unwrap_or_else(|| self.unknown()),
            Const { .. } => self.unknown(),
            Data(bytes) => {
                let mut value = self.unknown();
                value.length = Some(self.arena.constant(bytes.len() as u32));
                value
            }
            Numeric { op, args } => {
                let values = args.iter_mut().map(|e| self.eval(e, s)).collect::<Vec<_>>();
                self.numeric(*op, &values)
            }
            Let { local, value, body } => {
                let value = self.eval(value, s);
                s.locals[local.0] = value;
                self.eval(body, s)
            }
            Block(items) => {
                let mut value = self.unknown();
                for item in items {
                    value = self.eval(item, s);
                }
                value
            }
            If {
                condition,
                then_body,
                else_body,
            } => {
                let c = self.eval(condition, s);
                let known = s.facts.truth(c.term, &self.arena);
                let mut yes = s.clone();
                yes.assume(c.term, true, &self.arena);
                let mut no = s.clone();
                no.assume(c.term, false, &self.arena);
                let a = self.eval(then_body, &mut yes);
                let b = self.eval(else_body, &mut no);
                let value = if !yes.normal {
                    b.clone()
                } else if !no.normal || a.term == b.term {
                    a.clone()
                } else if e.ty == Type::Num(NumType::I32) {
                    Value::new(self.arena.make(Node::Select(c.term, a.term, b.term)))
                } else {
                    self.unknown()
                };
                *s = self.join(yes, no);
                if let Some(truth) = known {
                    self.changed = true;
                    // Preserve *all* original condition evaluation, including
                    // null/dereference traps and side effects in its operands.
                    let condition = condition.as_ref().clone();
                    let chosen = if truth {
                        then_body.as_ref().clone()
                    } else {
                        else_body.as_ref().clone()
                    };
                    e.kind = Block(vec![
                        Expr {
                            ty: Type::Unit,
                            span: condition.span,
                            kind: Drop(Box::new(condition)),
                        },
                        chosen,
                    ]);
                }
                value
            }
            Loop { bindings, body, .. } => {
                for (_, initial) in bindings.iter_mut() {
                    self.eval(initial, s);
                }
                let mut iteration = s.clone();
                // Initial values do not constitute an invariant for later trips.
                for (local, _) in bindings {
                    iteration.locals[local.0] = self.unknown();
                }
                self.eval(body, &mut iteration);
                self.unknown()
            }
            Recur { assignments, .. } => {
                for (_, value) in assignments {
                    self.eval(value, s);
                }
                s.normal = false;
                self.unknown()
            }
            StructNew { ty, values } => {
                let values = values
                    .iter_mut()
                    .map(|e| self.eval(e, s))
                    .collect::<Vec<_>>();
                let mut out = self.unknown();
                if let TypeKind::Struct(fields) = &self.module.types[ty.0].kind {
                    out.fields = Arc::new(
                        values
                            .into_iter()
                            .enumerate()
                            .filter_map(|(i, v)| (!fields[i].mutable).then_some(((*ty, i), v)))
                            .collect(),
                    );
                }
                out
            }
            StructGet { ty, field, object } => {
                let value = self.eval(object, s);
                let TypeKind::Struct(fields) = &self.module.types[ty.0].kind else {
                    return self.unknown();
                };
                if fields[*field].mutable {
                    return self.unknown();
                }
                value
                    .fields
                    .get(&(*ty, *field))
                    .cloned()
                    .unwrap_or_else(|| {
                        Value::new(self.arena.make(Node::Field(*ty, *field, value.term)))
                    })
            }
            ArrayLen(object) => {
                let object = self.eval(object, s);
                Value::new(object.length(&mut self.arena))
            }
            ArrayNew {
                initial, length, ..
            } => {
                self.eval(initial, s);
                let length = self.eval(length, s);
                let mut out = self.unknown();
                out.length = Some(length.term);
                out
            }
            ArrayFixed { values, .. } => {
                for value in values.iter_mut() {
                    self.eval(value, s);
                }
                let mut out = self.unknown();
                out.length = Some(self.arena.constant(values.len() as u32));
                out
            }
            ArrayGet { object, index, .. } => {
                let object = self.eval(object, s);
                let index = self.eval(index, s);
                let one = self.arena.constant(1);
                let length = object.length(&mut self.arena);
                s.facts
                    .establish_range(length, index.term, one, &mut self.arena);
                self.unknown()
            }
            Buffer { op, args, .. } => {
                let values = args.iter_mut().map(|e| self.eval(e, s)).collect::<Vec<_>>();
                let mut out = self.unknown();
                match *op {
                    BufferOp::New => out.length = Some(self.arena.constant(values.len() as u32)),
                    BufferOp::Len => return Value::new(values[0].length(&mut self.arena)),
                    BufferOp::Get => {
                        let length = values[0].length(&mut self.arena);
                        let width = self.arena.constant(1);
                        s.facts
                            .establish_range(length, values[1].term, width, &mut self.arena);
                    }
                    BufferOp::Read(access) | BufferOp::ReadProven(access) => {
                        let length = values[0].length(&mut self.arena);
                        let width = self.arena.constant(access.width());
                        if s.facts.fits(length, values[1].term, width, &self.arena) {
                            self.changed |= matches!(op, BufferOp::Read(_));
                            *op = BufferOp::ReadProven(access);
                        }
                        s.facts
                            .establish_range(length, values[1].term, width, &mut self.arena);
                    }
                    BufferOp::Slice => {
                        let length = values[0].length(&mut self.arena);
                        s.facts.establish_range(
                            length,
                            values[1].term,
                            values[2].term,
                            &mut self.arena,
                        );
                        out.length = Some(values[2].term);
                    }
                    BufferOp::Decode | BufferOp::IsUtf8 if values.len() == 3 => {
                        let length = values[0].length(&mut self.arena);
                        s.facts.establish_range(
                            length,
                            values[1].term,
                            values[2].term,
                            &mut self.arena,
                        );
                    }
                    _ => {}
                }
                out
            }
            Drop(value) => {
                self.eval(value, s);
                self.unknown()
            }
            Unreachable => {
                s.normal = false;
                self.unknown()
            }
            Call { args, .. } => {
                for arg in args {
                    self.eval(arg, s);
                }
                s.facts = Facts::default();
                self.unknown()
            }
            Return(value) => {
                if let Some(v) = value {
                    self.eval(v, s);
                }
                s.normal = false;
                self.unknown()
            }
            Nop => self.unknown(),
            _ => {
                // Catch/label/match/unknown operations do not export normal-path
                // facts. Process each child independently to avoid arm leakage.
                let mut base = s.clone();
                base.facts = Facts::default();
                for child in e.children_mut() {
                    self.eval(child, &mut base.clone());
                }
                s.facts = Facts::default();
                self.unknown()
            }
        }
    }
}
