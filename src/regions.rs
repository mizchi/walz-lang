//! May-access paths and allocation-site escape analysis of the typed core.
//! Parameter/capture roots can alias. Allocation sites merge all invocations
//! and iterations. These summaries are not a proof of disjointness or purity.
use crate::ir::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

const MAX_DEPTH: usize = 3;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Root {
    Parameter {
        index: usize,
    },
    Capture {
        index: usize,
    },
    Allocation {
        function: String,
        span: Span,
        slot: u8,
    },
    Unknown,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Member {
    Field { index: usize },
    Element,
    Payload { variant: usize, index: usize },
    Any,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Path {
    pub root: Root,
    pub via: Vec<Member>,
}
type Points = BTreeSet<Path>;
type Heap = BTreeMap<Location, Points>;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Location {
    pub object: Path,
    pub member: Member,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct EscapeSummary {
    pub returned: bool,
    pub stored: bool,
    pub host: bool,
    pub thrown: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Allocation {
    pub site: Root,
    pub ty: String,
    pub escapes: EscapeSummary,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct MemorySummary {
    pub reads: BTreeSet<Location>,
    pub writes: BTreeSet<Location>,
    /// References/value carriers returned directly, before following the heap.
    pub returns: Points,
    pub allocations: Vec<Allocation>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Summary {
    reads: BTreeSet<Location>,
    writes: BTreeSet<Location>,
    heap: Heap,
    returns: Points,
    host: Points,
    thrown: Points,
    allocations: BTreeMap<Root, Type>,
}

impl Path {
    fn new(root: Root) -> Self {
        Self { root, via: vec![] }
    }
    fn project(&self, member: &Member) -> Self {
        if self.root == Root::Unknown || self.via.len() == MAX_DEPTH {
            return Self::new(Root::Unknown);
        }
        let mut next = self.clone();
        next.via.push(member.clone());
        next
    }
}
fn points(root: Root) -> Points {
    BTreeSet::from([Path::new(root)])
}
fn references(ty: &Type) -> bool {
    matches!(ty, Type::Ref { .. } | Type::Value(_))
}

fn merge_heap(target: &mut Heap, source: &Heap) {
    for (location, values) in source {
        target
            .entry(location.clone())
            .or_default()
            .extend(values.iter().cloned());
    }
}
impl Summary {
    fn union(&mut self, other: &Self) {
        self.reads.extend(other.reads.iter().cloned());
        self.writes.extend(other.writes.iter().cloned());
        merge_heap(&mut self.heap, &other.heap);
        self.returns.extend(other.returns.iter().cloned());
        self.host.extend(other.host.iter().cloned());
        self.thrown.extend(other.thrown.iter().cloned());
        self.allocations.extend(other.allocations.clone());
    }
}

fn project(objects: &Points, member: &Member, heap: &Heap) -> Points {
    objects
        .iter()
        .flat_map(|object| {
            heap.get(&Location {
                object: object.clone(),
                member: member.clone(),
            })
            .cloned()
            .unwrap_or_else(|| BTreeSet::from([object.project(member)]))
        })
        .collect()
}

/// Substitution reads every projection from the caller's weak heap. In
/// particular, callee return aliases are not treated as fresh allocations.
fn substitute(path: &Path, args: &[Points], captures: &[Points], heap: &Heap) -> Points {
    let mut values = match &path.root {
        Root::Parameter { index } => args.get(*index).cloned().unwrap_or_default(),
        Root::Capture { index } => captures.get(*index).cloned().unwrap_or_default(),
        _ => points(path.root.clone()),
    };
    for member in &path.via {
        values = project(&values, member, heap);
    }
    values
}
fn substitute_all(values: &Points, args: &[Points], captures: &[Points], heap: &Heap) -> Points {
    values
        .iter()
        .flat_map(|p| substitute(p, args, captures, heap))
        .collect()
}

struct Context<'a> {
    module: &'a Module,
    function: &'a Function,
    summaries: &'a [Summary],
    effects: &'a [crate::effects::EffectSet],
    locals: Vec<Points>,
    summary: Summary,
}
impl Context<'_> {
    fn bind(&mut self, id: LocalId, values: Points) {
        self.locals[id.0].extend(values);
    }
    fn allocate(&mut self, expr: &Expr, slot: u8, ty: &Type) -> Points {
        let root = Root::Allocation {
            function: self.function.name.clone(),
            span: expr.span,
            slot,
        };
        self.summary.allocations.insert(root.clone(), ty.clone());
        points(root)
    }
    fn store(&mut self, objects: &Points, member: Member, values: Points, initial: bool) {
        for object in objects {
            let location = Location {
                object: object.clone(),
                member: member.clone(),
            };
            if !initial
                && !matches!(object.root, Root::Allocation { .. })
                && !self.summary.heap.contains_key(&location)
            {
                self.summary
                    .heap
                    .insert(location.clone(), BTreeSet::from([object.project(&member)]));
            }
            self.summary
                .heap
                .entry(location)
                .or_default()
                .extend(values.iter().cloned());
        }
    }
    fn access(&mut self, objects: &Points, member: Member, write: bool) {
        let set = if write {
            &mut self.summary.writes
        } else {
            &mut self.summary.reads
        };
        set.extend(objects.iter().map(|object| Location {
            object: object.clone(),
            member: member.clone(),
        }));
    }
    fn havoc(&mut self) {
        let locations = self
            .summary
            .heap
            .keys()
            .filter(|location| {
                self.slot(location)
                    .is_none_or(|(ty, mutable)| mutable && references(&ty))
            })
            .cloned()
            .collect::<Vec<_>>();
        for location in locations {
            self.summary
                .heap
                .get_mut(&location)
                .unwrap()
                .insert(Path::new(Root::Unknown));
        }
    }
    fn slot(&self, location: &Location) -> Option<(Type, bool)> {
        let mut ty = match &location.object.root {
            Root::Parameter { index } => self.function.locals[self.function.params[*index].0]
                .ty
                .clone(),
            Root::Capture { index } => self.function.locals[self.function.captures[*index].0]
                .ty
                .clone(),
            Root::Allocation { .. } => self.summary.allocations.get(&location.object.root)?.clone(),
            Root::Unknown => return None,
        };
        for member in &location.object.via {
            ty = self.member(&ty, member)?.0;
        }
        self.member(&ty, &location.member)
    }
    fn member(&self, ty: &Type, member: &Member) -> Option<(Type, bool)> {
        let id = match ty {
            Type::Ref { ty, .. } | Type::Value(ty) => *ty,
            _ => return None,
        };
        match (&self.module.types[id.0].kind, member) {
            (
                TypeKind::Struct(fields)
                | TypeKind::Closure {
                    captures: fields, ..
                },
                Member::Field { index },
            ) => fields.get(*index).map(|f| (f.ty.clone(), f.mutable)),
            (TypeKind::Array(field), Member::Element) => Some((field.ty.clone(), field.mutable)),
            (TypeKind::Value(ValueType::List(t)), Member::Element) => Some((t.clone(), false)),
            (TypeKind::Variant(variants), Member::Payload { variant, index }) => variants
                .get(*variant)?
                .payloads
                .get(*index)
                .map(|ty| (ty.clone(), false)),
            (TypeKind::Value(value), Member::Payload { variant, index }) => value
                .variants(id)
                .get(*variant)?
                .payloads
                .get(*index)
                .map(|ty| (ty.clone(), false)),
            _ => None,
        }
    }
    fn call(&mut self, id: FunctionId, values: Vec<Points>) -> Points {
        let callee = &self.module.functions[id.0];
        let (args, captures) = if matches!(callee.kind, FunctionKind::Closure { .. }) {
            let captures = callee
                .captures
                .iter()
                .enumerate()
                .map(|(index, _)| project(&values[0], &Member::Field { index }, &self.summary.heap))
                .collect::<Vec<_>>();
            (values[1..].to_vec(), captures)
        } else {
            (values, vec![])
        };
        let callee_summary = &self.summaries[id.0];
        self.summary
            .allocations
            .extend(callee_summary.allocations.clone());
        // Stores are weak: initial/old references remain possible. Revisit
        // them to account for stores through aliases and recursive containers.
        loop {
            let previous = self.summary.heap.clone();
            for (location, values) in &callee_summary.heap {
                let objects = substitute(&location.object, &args, &captures, &self.summary.heap);
                let values = substitute_all(values, &args, &captures, &self.summary.heap);
                self.store(&objects, location.member.clone(), values, false);
            }
            if previous == self.summary.heap {
                break;
            }
        }
        for (source, write) in [
            (&callee_summary.reads, false),
            (&callee_summary.writes, true),
        ] {
            for location in source {
                let objects = substitute(&location.object, &args, &captures, &self.summary.heap);
                self.access(&objects, location.member.clone(), write);
            }
        }
        self.summary.host.extend(substitute_all(
            &callee_summary.host,
            &args,
            &captures,
            &self.summary.heap,
        ));
        self.summary.thrown.extend(substitute_all(
            &callee_summary.thrown,
            &args,
            &captures,
            &self.summary.heap,
        ));
        if self.effects[id.0].suspends
            || callee_summary
                .writes
                .iter()
                .any(|l| l.object.root == Root::Unknown)
        {
            self.havoc();
        }
        substitute_all(
            &callee_summary.returns,
            &args,
            &captures,
            &self.summary.heap,
        )
    }
    fn eval(&mut self, e: &Expr) -> Points {
        use ExprKind::*;
        let result = match &e.kind {
            Data(_) => self.allocate(e, 0, &e.ty),
            List { op, args, .. } => {
                let values = args.iter().map(|e| self.eval(e)).collect::<Vec<_>>();
                match op {
                    ListOp::Len => Points::new(),
                    ListOp::Get => project(&values[0], &Member::Element, &self.summary.heap),
                    _ => {
                        let mut elements = Points::new();
                        if *op == ListOp::New {
                            for value in values {
                                elements.extend(value);
                            }
                        } else {
                            if let ListOp::FromArray(id) = op
                                && matches!(self.module.types[id.0].kind,TypeKind::Array(ref field) if field.mutable)
                            {
                                self.access(&values[0], Member::Element, false);
                            }
                            elements = project(&values[0], &Member::Element, &self.summary.heap);
                        }
                        let objects = self.allocate(e, 0, &e.ty);
                        self.store(&objects, Member::Element, elements, true);
                        objects
                    }
                }
            }
            Buffer { op, args, .. } => {
                for arg in args {
                    let objects = self.eval(arg);
                    if let BufferOp::FromArray(ty) = op
                        && matches!(&self.module.types[ty.0].kind, TypeKind::Array(f) if f.mutable)
                    {
                        self.access(&objects, Member::Element, false);
                    }
                }
                if !op.allocates() {
                    Points::new()
                } else if *op == BufferOp::Decode {
                    // Option payload and its text allocation are private immutable storage.
                    let objects = self.allocate(e, 0, &e.ty);
                    let text = Type::Value(self.module.value_id(&ValueType::Text));
                    let payload = self.allocate(e, 1, &text);
                    self.store(
                        &objects,
                        Member::Payload {
                            variant: 1,
                            index: 0,
                        },
                        payload,
                        true,
                    );
                    objects
                } else {
                    self.allocate(e, 0, &e.ty)
                }
            }
            ResourceNew { handle, .. } => {
                self.eval(handle);
                self.allocate(e, 0, &e.ty)
            }
            Const { .. } | Null(_) | Nop | Unreachable => Points::new(),
            Local(id) | Move(id) => self.locals[id.0].clone(),
            Numeric { args, .. } | Simd { args, .. } => {
                for arg in args {
                    self.eval(arg);
                }
                Points::new()
            }
            ResourceClose(value)
            | ResourceHandle(value)
            | Close(value)
            | Drop(value)
            | IsNull(value) => {
                self.eval(value);
                Points::new()
            }
            AsNonNull(value) => self.eval(value),
            RefEq(a, b) => {
                self.eval(a);
                self.eval(b);
                Points::new()
            }
            Let { local, value, body } | Using { local, value, body } => {
                let value = self.eval(value);
                self.bind(*local, value);
                self.eval(body)
            }
            Block(values) => {
                let mut result = Points::new();
                for value in values {
                    result = self.eval(value);
                }
                result
            }
            If {
                condition,
                then_body,
                else_body,
            } => {
                self.eval(condition);
                let mut result = self.eval(then_body);
                result.extend(self.eval(else_body));
                result
            }
            Call { function, args } => {
                let values = args.iter().map(|e| self.eval(e)).collect();
                self.call(*function, values)
            }
            StructNew { values, .. } | ArrayFixed { values, .. } | VariantNew { values, .. } => {
                let values = values.iter().map(|v| self.eval(v)).collect::<Vec<_>>();
                let objects = self.allocate(e, 0, &e.ty);
                for (index, value) in values.into_iter().enumerate() {
                    let member = match &e.kind {
                        ArrayFixed { .. } => Member::Element,
                        VariantNew { variant, .. } => Member::Payload {
                            variant: *variant,
                            index,
                        },
                        _ => Member::Field { index },
                    };
                    self.store(&objects, member, value, true);
                }
                // Empty arrays still have a known (empty) contents set.
                if matches!(e.kind, ArrayFixed { .. }) {
                    self.store(&objects, Member::Element, Points::new(), true);
                }
                objects
            }
            ArrayNew {
                initial, length, ..
            } => {
                let value = self.eval(initial);
                self.eval(length);
                let objects = self.allocate(e, 0, &e.ty);
                self.store(&objects, Member::Element, value, true);
                objects
            }
            StructGet { ty, field, object } => {
                let objects = self.eval(object);
                let member = Member::Field { index: *field };
                let TypeKind::Struct(fields) = &self.module.types[ty.0].kind else {
                    unreachable!()
                };
                if fields[*field].mutable {
                    self.access(&objects, member.clone(), false);
                }
                project(&objects, &member, &self.summary.heap)
            }
            StructSet {
                field,
                object,
                value,
                ..
            } => {
                let objects = self.eval(object);
                let value = self.eval(value);
                let member = Member::Field { index: *field };
                self.access(&objects, member.clone(), true);
                self.store(&objects, member, value, false);
                Points::new()
            }
            ArrayGet { ty, object, index } => {
                let objects = self.eval(object);
                self.eval(index);
                let TypeKind::Array(field) = &self.module.types[ty.0].kind else {
                    unreachable!()
                };
                if field.mutable {
                    self.access(&objects, Member::Element, false);
                }
                project(&objects, &Member::Element, &self.summary.heap)
            }
            ArraySet {
                object,
                index,
                value,
                ..
            } => {
                let objects = self.eval(object);
                self.eval(index);
                let value = self.eval(value);
                self.access(&objects, Member::Element, true);
                self.store(&objects, Member::Element, value, false);
                Points::new()
            }
            ArrayLen(object) => {
                self.eval(object);
                Points::new()
            }
            Match { subject, arms, .. } => {
                let objects = self.eval(subject);
                let mut result = Points::new();
                for arm in arms {
                    for (index, id) in arm.bindings.iter().enumerate() {
                        let value = project(
                            &objects,
                            &Member::Payload {
                                variant: arm.variant,
                                index,
                            },
                            &self.summary.heap,
                        );
                        self.bind(*id, value);
                    }
                    result.extend(self.eval(&arm.body));
                }
                result
            }
            Loop { bindings, body, .. } => {
                let initial = bindings
                    .iter()
                    .map(|(id, e)| (*id, self.eval(e)))
                    .collect::<Vec<_>>();
                for (id, value) in initial {
                    self.bind(id, value);
                }
                let mut result = Points::new();
                loop {
                    let before = (self.locals.clone(), self.summary.clone());
                    result.extend(self.eval(body));
                    if before == (self.locals.clone(), self.summary.clone()) {
                        break;
                    }
                }
                result
            }
            Recur { assignments, .. } => {
                let values = assignments
                    .iter()
                    .map(|(id, e)| (*id, self.eval(e)))
                    .collect::<Vec<_>>();
                for (id, value) in values {
                    self.bind(id, value);
                }
                Points::new()
            }
            Throw { values, .. } => {
                for value in values {
                    let value = self.eval(value);
                    self.summary.thrown.extend(value);
                }
                Points::new()
            }
            Try { body, handlers } => {
                let mut result = self.eval(body);
                for handler in handlers {
                    for id in &handler.bindings {
                        if references(&self.function.locals[id.0].ty) {
                            self.bind(*id, points(Root::Unknown));
                        }
                    }
                    result.extend(self.eval(&handler.body));
                }
                result
            }
            Delivery { op, endpoint, args } => {
                let values = args.iter().map(|v| self.eval(v)).collect::<Vec<_>>();
                match op {
                    DeliveryOp::FutureReady | DeliveryOp::StreamEmpty | DeliveryOp::StreamCons => {
                        let object = self.allocate(e, 0, &e.ty);
                        let variant = usize::from(*op == DeliveryOp::StreamCons);
                        for (index, value) in values.into_iter().enumerate() {
                            self.store(&object, Member::Payload { variant, index }, value, true);
                        }
                        object
                    }
                    DeliveryOp::FutureAwait => {
                        self.havoc();
                        project(
                            &values[0],
                            &Member::Payload {
                                variant: 0,
                                index: 0,
                            },
                            &self.summary.heap,
                        )
                    }
                    DeliveryOp::StreamNext => {
                        self.havoc();
                        let TypeKind::Value(ValueType::Stream(payload)) =
                            &self.module.types[endpoint.0].kind
                        else {
                            unreachable!()
                        };
                        let payload = payload.clone();
                        let tail = usize::from(payload != Type::Unit);
                        let mut rest = project(
                            &values[0],
                            &Member::Payload {
                                variant: 1,
                                index: tail,
                            },
                            &self.summary.heap,
                        );
                        rest.extend(values[0].clone());
                        let option_ty =
                            Type::Value(self.module.value_id(&ValueType::Option(payload)));
                        let option = self.allocate(e, 1, &option_ty);
                        let item = project(
                            &values[0],
                            &Member::Payload {
                                variant: 1,
                                index: 0,
                            },
                            &self.summary.heap,
                        );
                        self.store(
                            &option,
                            Member::Payload {
                                variant: 1,
                                index: 0,
                            },
                            item,
                            true,
                        );
                        let pair = self.allocate(e, 0, &e.ty);
                        self.store(
                            &pair,
                            Member::Payload {
                                variant: 0,
                                index: 0,
                            },
                            rest,
                            true,
                        );
                        self.store(
                            &pair,
                            Member::Payload {
                                variant: 0,
                                index: 1,
                            },
                            option,
                            true,
                        );
                        pair
                    }
                    DeliveryOp::FutureClose | DeliveryOp::StreamClose => Points::new(),
                }
            }
        };
        if references(&e.ty) {
            result
        } else {
            Points::new()
        }
    }
}

/// Reachability includes container/capture edges and cycles. Unknown reachable
/// contents might alias any allocation, so it prevents a non-escape claim.
fn reachable(start: &Points, heap: &Heap) -> BTreeSet<Root> {
    let mut pending = start.iter().cloned().collect::<Vec<_>>();
    let mut seen = Points::new();
    let mut roots = BTreeSet::new();
    while let Some(path) = pending.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }
        roots.insert(path.root.clone());
        for (location, values) in heap {
            if location.object == path {
                pending.extend(values.iter().cloned());
            }
        }
    }
    roots
}

pub fn analyze(module: &Module) -> Vec<MemorySummary> {
    let effects = crate::effects::infer(module);
    let mut summaries = vec![Summary::default(); module.functions.len()];
    loop {
        let before = summaries.clone();
        for (index, function) in module.functions.iter().enumerate() {
            let mut context = Context {
                module,
                function,
                summaries: &before,
                effects: &effects,
                locals: vec![Points::new(); function.locals.len()],
                summary: Summary::default(),
            };
            for (i, id) in function.params.iter().enumerate() {
                if references(&function.locals[id.0].ty) {
                    context.bind(*id, points(Root::Parameter { index: i }));
                }
            }
            for (i, id) in function.captures.iter().enumerate() {
                if references(&function.locals[id.0].ty) {
                    context.bind(*id, points(Root::Capture { index: i }));
                }
            }
            if let Some(body) = &function.body {
                context.summary.returns = context.eval(body);
            } else {
                if references(&function.result) {
                    context.summary.returns = points(Root::Unknown);
                }
                if crate::bindings::lookup(function).is_none() {
                    let location = Location {
                        object: Path::new(Root::Unknown),
                        member: Member::Any,
                    };
                    context.summary.reads.insert(location.clone());
                    context.summary.writes.insert(location);
                    for values in &context.locals {
                        context.summary.host.extend(values.iter().cloned());
                    }
                }
            }
            summaries[index].union(&context.summary);
        }
        if summaries == before {
            break;
        }
    }
    summaries
        .into_iter()
        .map(|summary| {
            let returned = reachable(&summary.returns, &summary.heap);
            let mut external = Points::new();
            for (location, values) in &summary.heap {
                if !matches!(location.object.root, Root::Allocation { .. }) {
                    external.extend(values.iter().cloned());
                }
            }
            let stored = reachable(&external, &summary.heap);
            let host = reachable(&summary.host, &summary.heap);
            let thrown = reachable(&summary.thrown, &summary.heap);
            let escapes = |set: &BTreeSet<Root>, root: &Root| {
                set.contains(root) || set.contains(&Root::Unknown)
            };
            MemorySummary {
                reads: summary.reads,
                writes: summary.writes,
                returns: summary.returns,
                allocations: summary
                    .allocations
                    .into_iter()
                    .map(|(site, ty)| {
                        let flags = EscapeSummary {
                            returned: escapes(&returned, &site),
                            stored: escapes(&stored, &site),
                            host: escapes(&host, &site),
                            thrown: escapes(&thrown, &site),
                        };
                        Allocation {
                            site,
                            ty: module.type_name(&ty),
                            escapes: flags,
                        }
                    })
                    .collect(),
            }
        })
        .collect()
}
