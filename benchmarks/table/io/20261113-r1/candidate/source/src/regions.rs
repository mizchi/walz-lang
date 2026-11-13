//! May-access paths and allocation-site escape analysis of the typed core.
//! Parameter/capture roots can alias. Allocation sites merge all invocations
//! and iterations. These summaries are not a proof of disjointness or purity.
use crate::ir::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

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

// Insert borrowed paths only when new. Flattening overlapping alias sets into
// a Vec before deduplication can create a quadratic number of Path/String clones.
fn union_paths(target: &mut Points, source: &Points) {
    for path in source {
        if !target.contains(path) {
            target.insert(path.clone());
        }
    }
}
fn project(objects: &Points, member: &Member, heap: &Heap) -> Points {
    let mut result = Points::new();
    for object in objects {
        if let Some(values) = heap.get(&Location {
            object: object.clone(),
            member: member.clone(),
        }) {
            union_paths(&mut result, values);
        } else {
            result.insert(object.project(member));
        }
    }
    result
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
    fn cached(
        path: &Path,
        args: &[Points],
        captures: &[Points],
        heap: &Heap,
        memo: &mut BTreeMap<Path, Rc<Points>>,
    ) -> Rc<Points> {
        if let Some(result) = memo.get(path) {
            return Rc::clone(result);
        }
        let result = if let Some((last, prefix)) = path.via.split_last() {
            let parent = Path {
                root: path.root.clone(),
                via: prefix.to_vec(),
            };
            project(&cached(&parent, args, captures, heap, memo), last, heap)
        } else {
            substitute(path, args, captures, heap)
        };
        let result = Rc::new(result);
        memo.insert(path.clone(), Rc::clone(&result));
        result
    }
    let mut result = Points::new();
    let mut memo = BTreeMap::new();
    for path in values {
        union_paths(&mut result, &cached(path, args, captures, heap, &mut memo));
    }
    result
}

struct Context<'a> {
    module: &'a Module,
    function: &'a Function,
    summaries: &'a [Rc<Summary>],
    effects: &'a [crate::effects::EffectSet],
    locals: Vec<Points>,
    summary: Summary,
    labels: BTreeMap<LoopId, Points>,
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
    fn store(&mut self, objects: &Points, member: Member, values: Points, initial: bool) -> bool {
        let mut changed = false;
        for object in objects {
            let location = Location {
                object: object.clone(),
                member: member.clone(),
            };
            if !self.summary.heap.contains_key(&location) {
                let old = if !initial && !matches!(object.root, Root::Allocation { .. }) {
                    BTreeSet::from([object.project(&member)])
                } else {
                    Points::new()
                };
                self.summary.heap.insert(location.clone(), old);
                // Even an empty slot changes projection from unknown to empty.
                changed = true;
            }
            let slot = self.summary.heap.get_mut(&location).unwrap();
            let previous = slot.len();
            slot.extend(values.iter().cloned());
            changed |= slot.len() != previous;
        }
        changed
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
            let mut changed = false;
            for (location, values) in &callee_summary.heap {
                let objects = substitute(&location.object, &args, &captures, &self.summary.heap);
                let values = substitute_all(values, &args, &captures, &self.summary.heap);
                changed |= self.store(&objects, location.member.clone(), values, false);
            }
            if !changed {
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
            TaskScope { body } => self.eval(body),
            TaskSpawn { environment, .. } => {
                let captures = self.eval(environment);
                self.summary.host.extend(captures);
                let location = Location {
                    object: Path::new(Root::Unknown),
                    member: Member::Any,
                };
                self.summary.reads.insert(location.clone());
                self.summary.writes.insert(location);
                self.havoc();
                self.allocate(e, 0, &e.ty)
            }
            TaskJoin { task, timeout, .. } => {
                if let Some(timeout) = timeout {
                    self.eval(timeout);
                }
                self.eval(task);
                self.havoc();
                points(Root::Unknown)
            }
            Label { label, body } => {
                let mut p = self.eval(body);
                if let Some(branches) = self.labels.get(label) {
                    p.extend(branches.iter().cloned());
                }
                p
            }
            Branch {
                label,
                values,
                condition,
                bindings,
                is_loop,
            } => {
                let evaluated = values.iter().map(|v| self.eval(v)).collect::<Vec<_>>();
                if let Some(c) = condition {
                    self.eval(c);
                }
                if *is_loop {
                    for (id, p) in bindings.iter().zip(&evaluated) {
                        self.bind(*id, p.clone());
                    }
                } else {
                    for p in &evaluated {
                        self.labels
                            .entry(*label)
                            .or_default()
                            .extend(p.iter().cloned());
                    }
                }
                if condition.is_some() {
                    evaluated.into_iter().next().unwrap_or_default()
                } else {
                    Points::new()
                }
            }
            Return(v) => {
                if let Some(v) = v {
                    let p = self.eval(v);
                    self.summary.returns.extend(p);
                }
                Points::new()
            }
            Newtype { value, .. } => {
                self.eval(value);
                Points::new()
            }
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
                for (index, arg) in args.iter().enumerate() {
                    let objects = self.eval(arg);
                    if index == 0
                        && let BufferOp::FromArray(ty) = op
                        && matches!(&self.module.types[ty.0].kind, TypeKind::Array(f) if f.mutable)
                    {
                        self.access(&objects, Member::Element, false);
                    }
                    if index == 0
                        && matches!(op, BufferOp::CopyToArray(_) | BufferOp::WriteArray(..))
                    {
                        self.access(&objects, Member::Element, true);
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
            Wasm { instruction, args } => {
                for arg in args {
                    self.eval(arg);
                }
                let unknown = [Path {
                    root: Root::Unknown,
                    via: vec![],
                }]
                .into_iter()
                .collect();
                if instruction.reads() {
                    self.access(&unknown, Member::Any, false);
                }
                if instruction.writes() {
                    self.access(&unknown, Member::Any, true);
                }
                Points::new()
            }
            Memory { op, args, .. } => {
                for arg in args {
                    self.eval(arg);
                }
                let unknown = [Path {
                    root: Root::Unknown,
                    via: vec![],
                }]
                .into_iter()
                .collect();
                if op.reads {
                    self.access(&unknown, Member::Any, false);
                }
                if op.writes {
                    self.access(&unknown, Member::Any, true);
                }
                Points::new()
            }
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
            ArrayGet {
                ty, object, index, ..
            } => {
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
            ArrayCopy { args, source, .. } => {
                for (index, arg) in args.iter().enumerate() {
                    let objects = self.eval(arg);
                    if index == 0 {
                        self.access(&objects, Member::Element, true);
                    }
                    if index == 2
                        && matches!(&self.module.types[source.0].kind, TypeKind::Array(f) if f.mutable)
                    {
                        self.access(&objects, Member::Element, false);
                    }
                }
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
                    DeliveryOp::StreamReadBatch => {
                        self.havoc();
                        // A batch can contain any shared payload reachable from
                        // the input stream or supplied by an unknown host.
                        let pair = self.allocate(e, 0, &e.ty);
                        self.store(
                            &pair,
                            Member::Payload {
                                variant: 0,
                                index: 0,
                            },
                            values[1].clone(),
                            true,
                        );
                        self.store(
                            &pair,
                            Member::Payload {
                                variant: 0,
                                index: 1,
                            },
                            points(Root::Unknown),
                            true,
                        );
                        pair
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

fn compute(module: &Module, sparse: bool, mut evaluated: impl FnMut(usize)) -> Vec<Rc<Summary>> {
    let effects = crate::effects::infer(module);
    let count = module.functions.len();
    let mut callers = vec![BTreeSet::new(); count];
    for (caller, function) in module.functions.iter().enumerate() {
        let mut pending = function.body.iter().collect::<Vec<_>>();
        while let Some(expression) = pending.pop() {
            if let ExprKind::Call { function, .. } = expression.kind {
                callers[function.0].insert(caller);
            }
            pending.extend(expression.children());
        }
    }
    let mut summaries = vec![Rc::new(Summary::default()); count];
    let mut dirty = vec![true; count];
    loop {
        // Retain the original synchronous rounds: each transfer sees the same
        // previous round. Rc avoids cloning unchanged, potentially large heaps.
        let before = summaries.clone();
        let mut next = vec![!sparse; count];
        let mut changed = false;
        for (index, function) in module.functions.iter().enumerate() {
            if sparse && !dirty[index] {
                continue;
            }
            evaluated(index);
            let mut context = Context {
                labels: BTreeMap::new(),
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
                let normal = context.eval(body);
                context.summary.returns.extend(normal);
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
            Rc::make_mut(&mut summaries[index]).union(&context.summary);
            if summaries[index] != before[index] {
                changed = true;
                for caller in &callers[index] {
                    next[*caller] = true;
                }
            }
        }
        if !changed {
            break;
        }
        dirty = next;
    }
    summaries
}

pub fn analyze(module: &Module) -> Vec<MemorySummary> {
    let summaries = compute(module, true, |_| {});
    summaries
        .into_iter()
        .map(|summary| {
            let summary = Rc::unwrap_or_clone(summary);
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

#[cfg(test)]
mod scheduling_tests {
    use super::*;

    #[test]
    fn memoized_substitution_preserves_overlapping_aliases_and_unknown_roots() {
        fn reference_project(objects: &Points, member: &Member, heap: &Heap) -> Points {
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
        fn reference(path: &Path, args: &[Points], captures: &[Points], heap: &Heap) -> Points {
            let mut result = match path.root {
                Root::Parameter { index } => args.get(index).cloned().unwrap_or_default(),
                Root::Capture { index } => captures.get(index).cloned().unwrap_or_default(),
                _ => points(path.root.clone()),
            };
            for member in &path.via {
                result = reference_project(&result, member, heap);
            }
            result
        }
        let objects = (0..24)
            .map(|i| {
                Path::new(Root::Allocation {
                    function: "$alias".into(),
                    span: Span {
                        start: i,
                        end: i + 1,
                    },
                    slot: 0,
                })
            })
            .collect::<Vec<_>>();
        let all = objects.iter().cloned().collect::<Points>();
        let args = vec![all.clone(), points(Root::Unknown)];
        let captures = vec![objects[8..].iter().cloned().collect()];
        let mut heap = Heap::new();
        for object in &objects {
            heap.insert(
                Location {
                    object: object.clone(),
                    member: Member::Field { index: 0 },
                },
                all.clone(),
            );
            heap.insert(
                Location {
                    object: object.clone(),
                    member: Member::Element,
                },
                args[1].clone(),
            );
        }
        heap.insert(
            Location {
                object: Path::new(Root::Unknown),
                member: Member::Element,
            },
            all.clone(),
        );
        let mut values = all.clone();
        for root in [
            Root::Parameter { index: 0 },
            Root::Parameter { index: 1 },
            Root::Capture { index: 0 },
            Root::Unknown,
        ] {
            for via in [
                vec![],
                vec![Member::Field { index: 0 }],
                vec![Member::Field { index: 0 }, Member::Element],
                vec![Member::Element, Member::Field { index: 0 }, Member::Element],
            ] {
                values.insert(Path {
                    root: root.clone(),
                    via,
                });
            }
        }
        let expected = values
            .iter()
            .flat_map(|p| reference(p, &args, &captures, &heap))
            .collect::<Points>();
        assert_eq!(substitute_all(&values, &args, &captures, &heap), expected);
        assert_eq!(
            project(&all, &Member::Field { index: 0 }, &heap),
            reference_project(&all, &Member::Field { index: 0 }, &heap)
        );
    }

    #[test]
    fn store_delta_includes_empty_slot_creation_and_preserves_projection() {
        let module = crate::check("(module (func $f (nop)))").unwrap();
        let mut context = Context {
            module: &module,
            function: &module.functions[0],
            summaries: &[],
            effects: &[],
            locals: vec![],
            summary: Summary::default(),
            labels: BTreeMap::new(),
        };
        let object = points(Root::Allocation {
            function: "$f".into(),
            span: Span { start: 0, end: 1 },
            slot: 0,
        });
        assert!(!project(&object, &Member::Element, &context.summary.heap).is_empty());
        assert!(context.store(&object, Member::Element, Points::new(), true));
        assert!(project(&object, &Member::Element, &context.summary.heap).is_empty());
        assert!(!context.store(&object, Member::Element, Points::new(), false));
        let value = points(Root::Unknown);
        assert!(context.store(&object, Member::Element, value.clone(), false));
        assert_eq!(
            project(&object, &Member::Element, &context.summary.heap),
            value
        );
        assert!(!context.store(&object, Member::Element, value, false));
    }

    fn compare(source: &str) {
        let module = crate::check(source).unwrap();
        let mut naive = 0;
        let expected = compute(&module, false, |_| naive += 1);
        let mut sparse = 0;
        let actual = compute(&module, true, |_| sparse += 1);
        assert_eq!(
            actual, expected,
            "scheduling must preserve every may-access/escape fact"
        );
        assert!(
            sparse < naive,
            "unchanged dependencies must not be reevaluated: {sparse}/{naive}"
        );
    }

    #[test]
    fn dependency_schedule_preserves_recursive_aliases_and_unknown_calls() {
        compare(
            r#"(module
          (type $B (struct (field $next (mut (ref null $B)))))
          (import "env" "unknown" (func $unknown (param $x (ref $B)) (result (ref $B))))
          (func $leaf (param $x (ref $B)) (result (ref $B)) $x)
          (func $middle (param $x (ref $B)) (result (ref $B)) (call $leaf $x))
          (func $outer (param $x (ref $B)) (result (ref $B)) (call $middle $x))
          (func $cycle (param $x (ref $B)) (param $n i32) (result (ref $B))
            (if (result (ref $B)) $n
              (then (block (result (ref $B)) (struct.set $B $next $x $x) (call $cycle $x (- $n 1))))
              (else (call $unknown (call $outer $x)))))
          (func $independent (result i32) 42))"#,
        );
    }

    #[test]
    fn dependency_schedule_preserves_cached_reference_views() {
        compare(
            r#"(module
          (@use "walz:core" (type $list))
          (type $B (struct (field $v i32)))
          (type $A (array (mut (ref null $B))))
          (type $C (struct (field $cache (mut (ref null $A)))))
          (func $get (param $c (ref $C)) (result (ref $B))
            (@let $a (if (result (ref $A)) (ref.is_null (struct.get $C $cache $c))
              (then (@let $a (array.new $A (ref.null $B) 1)
                (block (result (ref $A)) (struct.set $C $cache $c $a) $a)))
              (else (ref.as_non_null (struct.get $C $cache $c))))
              (if (result (ref $B)) (ref.is_null (array.get $A $a 0))
                (then (@let $v (struct.new $B 42)
                  (block (result (ref $B)) (array.set $A $a 0 $v) $v)))
                (else (ref.as_non_null (array.get $A $a 0))))))
          (func $view (param $c (ref $C)) (result $list<(ref $B)>) (@list.new (ref $B) (call $get $c)))
          (func $use (param $c (ref $C)) (result (ref $B)) (@list.get (ref $B) (call $view $c) 0))
          (func $independent (result i32) 1))"#,
        );
    }
}
