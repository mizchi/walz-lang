//! Facts about immutable local snapshots on normal control-flow paths.
//! Integer ranges use unsigned i32 *bits*. Loops widen their carried values;
//! field contents and call results are not assumed stable or inferred pure.
use crate::ir::*;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Range {
    pub min: u32,
    pub max: u32,
}
impl Range {
    const FULL: Self = Self {
        min: 0,
        max: u32::MAX,
    };
    const BOOL: Self = Self { min: 0, max: 1 };
    fn exact(value: u32) -> Self {
        Self {
            min: value,
            max: value,
        }
    }
    fn constant(self) -> Option<u32> {
        (self.min == self.max).then_some(self.min)
    }
    fn intersect(self, other: Self) -> Option<Self> {
        let min = self.min.max(other.min);
        let max = self.max.min(other.max);
        (min <= max).then_some(Self { min, max })
    }
    fn join(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Nullness {
    Null,
    NonNull,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExpressionFacts {
    /// Preorder node ID within this function; spans alone need not be unique
    /// for generated macro nodes. Both identify the inspected IR occurrence.
    pub node: usize,
    pub span: Span,
    pub operation: String,
    pub reachable: bool,
    pub normal_return_possible: bool,
    pub i32_range: Option<Range>,
    pub nullness: Option<Nullness>,
    pub array_length: Option<Range>,
    /// Some(false) means unproved, not a proof that the check will fail.
    pub null_check_proven: Option<bool>,
    pub bounds_proven: Option<bool>,
}

#[derive(Clone, Debug)]
struct Value {
    range: Option<Range>,
    nullness: Option<Nullness>,
    length: Option<Range>,
    key: Option<usize>,
    length_of: Option<usize>,
    /// Snapshot whose unsigned bits equal this array's immutable length.
    length_key: Option<usize>,
}
impl Value {
    fn unknown(ty: &Type) -> Self {
        Self {
            range: (*ty == Type::Num(NumType::I32)).then_some(Range::FULL),
            nullness: match ty {
                Type::Ref {
                    nullable: false, ..
                } => Some(Nullness::NonNull),
                Type::Ref { .. } => Some(Nullness::Unknown),
                _ => None,
            },
            length: None,
            key: None,
            length_of: None,
            length_key: None,
        }
    }
    fn join(&self, other: &Self) -> Self {
        Self {
            range: self.range.zip(other.range).map(|(a, b)| a.join(b)),
            nullness: match (self.nullness, other.nullness) {
                (Some(a), Some(b)) => Some(if a == b { a } else { Nullness::Unknown }),
                _ => None,
            },
            length: self.length.zip(other.length).map(|(a, b)| a.join(b)),
            key: self.key.filter(|key| Some(*key) == other.key),
            length_of: self.length_of.filter(|key| Some(*key) == other.length_of),
            length_key: self.length_key.filter(|key| Some(*key) == other.length_key),
        }
    }
}

#[derive(Clone)]
struct State {
    locals: Vec<Value>,
    less: BTreeSet<(usize, usize)>,
    reachable: bool,
}
impl State {
    fn join(a: &Self, b: &Self) -> Self {
        if !a.reachable {
            return b.clone();
        }
        if !b.reachable {
            return a.clone();
        }
        Self {
            locals: a
                .locals
                .iter()
                .zip(&b.locals)
                .map(|(x, y)| x.join(y))
                .collect(),
            less: a.less.intersection(&b.less).copied().collect(),
            reachable: true,
        }
    }
    fn bind(&mut self, id: LocalId, mut value: Value) {
        if value.key.is_none() {
            value.key = Some(id.0);
        }
        self.locals[id.0] = value;
    }
    fn non_null(&mut self, value: &Value) -> bool {
        let proven = value.nullness == Some(Nullness::NonNull);
        if value.nullness == Some(Nullness::Null) {
            self.reachable = false;
        }
        if let Some(key) = value.key {
            for local in &mut self.locals {
                if local.key == Some(key) && local.nullness.is_some() {
                    local.nullness = Some(Nullness::NonNull);
                }
            }
        }
        proven
    }
    fn narrow(&mut self, key: usize, range: Option<Range>) {
        let Some(range) = range else {
            self.reachable = false;
            return;
        };
        for local in &mut self.locals {
            if local.key == Some(key) && local.range.is_some() {
                local.range = Some(range);
            }
        }
    }
}

fn literal(text: &str) -> Option<u32> {
    let text = text.replace('_', "");
    let (negative, text) = if let Some(t) = text.strip_prefix('-') {
        (true, t)
    } else {
        (false, text.strip_prefix('+').unwrap_or(&text))
    };
    let n = if let Some(t) = text.strip_prefix("0x") {
        u32::from_str_radix(t, 16).ok()?
    } else {
        text.parse().ok()?
    };
    Some(if negative { 0u32.wrapping_sub(n) } else { n })
}

fn numeric(op: NumericOp, args: &[Value]) -> Option<Range> {
    use NumericOp::*;
    if op.signature().output != NumType::I32 {
        return None;
    }
    let boolean = matches!(
        op,
        I32Eqz
            | I64Eqz
            | I32Eq
            | I32Ne
            | I32LtS
            | I32LtU
            | I32LeS
            | I32LeU
            | I32GtS
            | I32GtU
            | I32GeS
            | I32GeU
            | I64Eq
            | I64Ne
            | I64LtS
            | I64LtU
            | I64LeS
            | I64LeU
            | I64GtS
            | I64GtU
            | I64GeS
            | I64GeU
            | F32Eq
            | F32Ne
            | F32Lt
            | F32Le
            | F32Gt
            | F32Ge
            | F64Eq
            | F64Ne
            | F64Lt
            | F64Le
            | F64Gt
            | F64Ge
    );
    if op.signature().input != NumType::I32 {
        return Some(if boolean { Range::BOOL } else { Range::FULL });
    }
    let a = args[0].range.unwrap_or(Range::FULL);
    if op == I32Eqz {
        return Some(match a.constant() {
            Some(v) => Range::exact(u32::from(v == 0)),
            None if a.min > 0 => Range::exact(0),
            _ => Range::BOOL,
        });
    }
    // Other unary instructions have no range transfer rule yet. Preserve a
    // conservative result without treating them as binary operations.
    if op.signature().arity == 1 {
        return Some(Range::FULL);
    }
    let b = args[1].range.unwrap_or(Range::FULL);
    if let (Some(a), Some(b)) = (a.constant(), b.constant()) {
        let v = match op {
            I32Add => Some(a.wrapping_add(b)),
            I32Sub => Some(a.wrapping_sub(b)),
            I32Mul => Some(a.wrapping_mul(b)),
            I32And => Some(a & b),
            I32Or => Some(a | b),
            I32Xor => Some(a ^ b),
            I32Shl => Some(a.wrapping_shl(b & 31)),
            I32ShrU => Some(a >> (b & 31)),
            I32ShrS => Some(((a as i32) >> (b & 31)) as u32),
            I32Eq => Some(u32::from(a == b)),
            I32Ne => Some(u32::from(a != b)),
            I32LtU => Some(u32::from(a < b)),
            I32LeU => Some(u32::from(a <= b)),
            I32GtU => Some(u32::from(a > b)),
            I32GeU => Some(u32::from(a >= b)),
            I32LtS => Some(u32::from((a as i32) < (b as i32))),
            I32LeS => Some(u32::from((a as i32) <= (b as i32))),
            I32GtS => Some(u32::from((a as i32) > (b as i32))),
            I32GeS => Some(u32::from((a as i32) >= (b as i32))),
            _ => None,
        };
        if let Some(v) = v {
            return Some(Range::exact(v));
        }
    }
    let range = match op {
        I32Add if u64::from(a.max) + u64::from(b.max) <= u64::from(u32::MAX) => Range {
            min: a.min + b.min,
            max: a.max + b.max,
        },
        I32Sub if a.min >= b.max => Range {
            min: a.min - b.max,
            max: a.max - b.min,
        },
        I32Mul if u64::from(a.max) * u64::from(b.max) <= u64::from(u32::MAX) => Range {
            min: a.min * b.min,
            max: a.max * b.max,
        },
        I32And => Range {
            min: 0,
            max: a.max.min(b.max),
        },
        I32ShrU if b.constant().is_some() => {
            let n = b.min & 31;
            Range {
                min: a.min >> n,
                max: a.max >> n,
            }
        }
        I32Shl if b.constant().is_some() && a.max <= (u32::MAX >> (b.min & 31)) => {
            let n = b.min & 31;
            Range {
                min: a.min << n,
                max: a.max << n,
            }
        }
        _ if boolean => Range::BOOL,
        _ => Range::FULL,
    };
    Some(range)
}

/// Peek only immutable snapshots/pure arithmetic. It never re-executes a call
/// or treats repeated mutable-field loads as the same value.
fn peek(e: &Expr, s: &State) -> Value {
    let mut value = Value::unknown(&e.ty);
    match &e.kind {
        ExprKind::Local(id) | ExprKind::Move(id) => return s.locals[id.0].clone(),
        ExprKind::Const {
            ty: NumType::I32,
            literal: text,
        } => value.range = literal(text).map(Range::exact).or(Some(Range::FULL)),
        ExprKind::ArrayLen(object) => {
            let object = peek(object, s);
            value.range = Some(object.length.unwrap_or(Range::FULL));
            value.length_of = object.length_key.or(object.key);
        }
        ExprKind::Numeric { op, args } => {
            let args = args.iter().map(|e| peek(e, s)).collect::<Vec<_>>();
            value.range = numeric(*op, &args);
        }
        ExprKind::AsNonNull(object) => {
            value = peek(object, s);
            value.nullness = Some(Nullness::NonNull);
        }
        ExprKind::Null(_) => value.nullness = Some(Nullness::Null),
        _ => {}
    }
    value
}

fn signed_intersection(range: Range, lo: i64, hi: i64) -> Option<Range> {
    if lo > hi {
        return None;
    }
    let mut result = None;
    for (a, b) in [(lo, hi.min(-1)), (lo.max(0), hi)] {
        if a > b {
            continue;
        }
        let part = Range {
            min: (a as i32) as u32,
            max: (b as i32) as u32,
        };
        if let Some(part) = range.intersect(part) {
            result = Some(result.map_or(part, |r: Range| r.join(part)));
        }
    }
    result
}

fn refine(e: &Expr, truth: bool, s: &mut State) {
    use NumericOp::*;
    match &e.kind {
        ExprKind::IsNull(object) => {
            let value = peek(object, s);
            let wanted = if truth {
                Nullness::Null
            } else {
                Nullness::NonNull
            };
            if let Some(actual) = value.nullness
                && actual != Nullness::Unknown
                && actual != wanted
            {
                s.reachable = false;
            }
            if let Some(key) = value.key {
                for local in &mut s.locals {
                    if local.key == Some(key) && local.nullness.is_some() {
                        local.nullness = Some(wanted);
                    }
                }
            }
        }
        ExprKind::Numeric { op: I32Eqz, args } => refine(&args[0], !truth, s),
        ExprKind::Numeric { op: I32And, args } if truth => {
            refine(&args[0], true, s);
            refine(&args[1], true, s);
        }
        ExprKind::Numeric { op: I32Or, args } if !truth => {
            refine(&args[0], false, s);
            refine(&args[1], false, s);
        }
        ExprKind::Numeric { op, args }
            if args.len() == 2 && op.signature().input == NumType::I32 =>
        {
            let a = peek(&args[0], s);
            let b = peek(&args[1], s);
            let op = if truth {
                *op
            } else {
                match op {
                    I32LtU => I32GeU,
                    I32GeU => I32LtU,
                    I32LeU => I32GtU,
                    I32GtU => I32LeU,
                    I32LtS => I32GeS,
                    I32GeS => I32LtS,
                    I32LeS => I32GtS,
                    I32GtS => I32LeS,
                    I32Eq => I32Ne,
                    I32Ne => I32Eq,
                    _ => return,
                }
            };
            if matches!(op, I32LtU | I32LtS)
                && (op == I32LtU || a.range.is_some_and(|r| r.max <= i32::MAX as u32))
                && let (Some(index), Some(array)) = (a.key, b.length_of.or(b.key))
            {
                s.less.insert((index, array));
            }
            if matches!(op, I32GtU | I32GtS)
                && (op == I32GtU || b.range.is_some_and(|r| r.max <= i32::MAX as u32))
                && let (Some(index), Some(array)) = (b.key, a.length_of.or(a.key))
            {
                s.less.insert((index, array));
            }
            if let (Some(key), Some(range), Some(c)) =
                (a.key, a.range, b.range.and_then(Range::constant))
            {
                let c_signed = i64::from(c as i32);
                let narrowed = match op {
                    I32LtU => c
                        .checked_sub(1)
                        .and_then(|max| range.intersect(Range { min: 0, max })),
                    I32LeU => range.intersect(Range { min: 0, max: c }),
                    I32GtU => c
                        .checked_add(1)
                        .and_then(|min| range.intersect(Range { min, max: u32::MAX })),
                    I32GeU => range.intersect(Range {
                        min: c,
                        max: u32::MAX,
                    }),
                    I32Eq => range.intersect(Range::exact(c)),
                    I32Ne if range.constant() == Some(c) => None,
                    I32Ne if range.min == c => c.checked_add(1).map(|min| Range {
                        min,
                        max: range.max,
                    }),
                    I32Ne if range.max == c => c.checked_sub(1).map(|max| Range {
                        min: range.min,
                        max,
                    }),
                    I32LtS => signed_intersection(range, i64::from(i32::MIN), c_signed - 1),
                    I32LeS => signed_intersection(range, i64::from(i32::MIN), c_signed),
                    I32GtS => signed_intersection(range, c_signed + 1, i64::from(i32::MAX)),
                    I32GeS => signed_intersection(range, c_signed, i64::from(i32::MAX)),
                    _ => Some(range),
                };
                s.narrow(key, narrowed);
            }
        }
        _ => {}
    }
    let value = peek(e, s);
    if let Some(range) = value.range {
        if truth && range.max == 0 || !truth && range.min > 0 {
            s.reachable = false;
        }
        if !truth && let Some(key) = value.key {
            s.narrow(key, range.intersect(Range::exact(0)));
        }
    }
}

struct Visitor<'a> {
    module: &'a Module,
    function: &'a Function,
    next: usize,
    facts: Vec<ExpressionFacts>,
}
impl Visitor<'_> {
    fn eval(&mut self, e: &Expr, s: &mut State) -> Value {
        use ExprKind::*;
        let node = self.next;
        self.next += 1;
        let reachable = s.reachable;
        let mut value = Value::unknown(&e.ty);
        let mut null_check = None;
        let mut bounds = None;
        let operation = match &e.kind {
            TaskScope { body } => {
                value = self.eval(body, s);
                "task.scope".into()
            }
            TaskSpawn { .. } | TaskJoin { .. } => {
                for child in e.children() {
                    self.eval(child, s);
                }
                s.locals = self
                    .function
                    .locals
                    .iter()
                    .map(|l| Value::unknown(&l.ty))
                    .collect();
                s.less.clear();
                "task.wait".into()
            }
            Label { body, .. } => {
                let reachable = s.reachable;
                self.eval(body, s);
                s.locals = self
                    .function
                    .locals
                    .iter()
                    .map(|l| Value::unknown(&l.ty))
                    .collect();
                s.less.clear();
                s.reachable = reachable;
                "block.label".into()
            }
            Branch {
                values, condition, ..
            } => {
                for v in values {
                    self.eval(v, s);
                }
                if let Some(c) = condition {
                    self.eval(c, s);
                } else {
                    s.reachable = false;
                }
                "br".into()
            }
            Return(v) => {
                if let Some(v) = v {
                    self.eval(v, s);
                }
                s.reachable = false;
                "return".into()
            }
            Newtype {
                value: inner,
                unwrap,
                ..
            } => {
                self.eval(inner, s);
                if *unwrap {
                    "newtype.unwrap"
                } else {
                    "newtype.wrap"
                }
                .into()
            }
            Data(_) => "text.lit".into(),
            List { op, args, .. } => {
                for arg in args {
                    self.eval(arg, s);
                }
                format!("list.{op:?}")
            }
            Buffer { op, args, .. } => {
                for arg in args {
                    self.eval(arg, s);
                }
                if *op == BufferOp::Get {
                    value.range = Some(Range { min: 0, max: 255 });
                }
                format!("buffer.{op:?}")
            }
            Const {
                ty: NumType::I32,
                literal: text,
            } => {
                value.range = literal(text).map(Range::exact).or(Some(Range::FULL));
                "i32.const".into()
            }
            Const { ty, .. } => format!("{}.const", ty.wat()),
            Local(id) | Move(id) => {
                value = s.locals[id.0].clone();
                "local".into()
            }
            Memory { op, args, .. } => {
                for arg in args {
                    self.eval(arg, s);
                }
                op.name().to_owned()
            }
            Wasm { instruction, args } => {
                for arg in args {
                    self.eval(arg, s);
                }
                instruction.opcode.clone()
            }
            Numeric { op, args } => {
                let args = args.iter().map(|e| self.eval(e, s)).collect::<Vec<_>>();
                value.range = numeric(*op, &args);
                op.signature().name.into()
            }
            Simd { op, args } => {
                for arg in args {
                    self.eval(arg, s);
                }
                op.name().into()
            }
            Null(_) => {
                value.nullness = Some(Nullness::Null);
                "ref.null".into()
            }
            IsNull(object) => {
                let object = self.eval(object, s);
                value.range = Some(match object.nullness {
                    Some(Nullness::Null) => Range::exact(1),
                    Some(Nullness::NonNull) => Range::exact(0),
                    _ => Range::BOOL,
                });
                "ref.is_null".into()
            }
            AsNonNull(object) => {
                value = self.eval(object, s);
                null_check = Some(s.non_null(&value));
                value.nullness = Some(Nullness::NonNull);
                "ref.as_non_null".into()
            }
            RefEq(a, b) => {
                self.eval(a, s);
                self.eval(b, s);
                value.range = Some(Range::BOOL);
                "ref.eq".into()
            }
            Let {
                local,
                value: initial,
                body,
            }
            | Using {
                local,
                value: initial,
                body,
            } => {
                let initial = self.eval(initial, s);
                s.bind(*local, initial);
                value = self.eval(body, s);
                "let".into()
            }
            Block(values) => {
                for child in values {
                    value = self.eval(child, s);
                }
                "block".into()
            }
            If {
                condition,
                then_body,
                else_body,
            } => {
                let condition_value = self.eval(condition, s);
                let mut yes = s.clone();
                let mut no = s.clone();
                if condition_value.range.is_some_and(|r| r.max == 0) {
                    yes.reachable = false;
                }
                if condition_value.range.is_some_and(|r| r.min > 0) {
                    no.reachable = false;
                }
                refine(condition, true, &mut yes);
                refine(condition, false, &mut no);
                let a = self.eval(then_body, &mut yes);
                let b = self.eval(else_body, &mut no);
                value = if !yes.reachable {
                    b
                } else if !no.reachable {
                    a
                } else {
                    a.join(&b)
                };
                *s = State::join(&yes, &no);
                "if".into()
            }
            StructGet { object, .. } => {
                let object = self.eval(object, s);
                null_check = Some(s.non_null(&object));
                "struct.get".into()
            }
            StructSet {
                object,
                value: stored,
                ..
            } => {
                let object = self.eval(object, s);
                self.eval(stored, s);
                null_check = Some(s.non_null(&object));
                "struct.set".into()
            }
            ArrayLen(object) => {
                let object = self.eval(object, s);
                null_check = Some(s.non_null(&object));
                value.range = Some(object.length.unwrap_or(Range::FULL));
                value.length_of = object.length_key.or(object.key);
                "array.len".into()
            }
            ArrayGet { object, index, .. } | ArraySet { object, index, .. } => {
                let object = self.eval(object, s);
                let index = self.eval(index, s);
                if let ArraySet { value: stored, .. } = &e.kind {
                    self.eval(stored, s);
                }
                null_check = Some(s.non_null(&object));
                let range_proven = index
                    .range
                    .zip(object.length)
                    .is_some_and(|(i, n)| i.max < n.min);
                let relation_proven = index
                    .key
                    .zip(object.length_key.or(object.key))
                    .is_some_and(|keys| s.less.contains(&keys));
                bounds = Some(range_proven || relation_proven);
                if index
                    .range
                    .zip(object.length)
                    .is_some_and(|(i, n)| i.min >= n.max)
                {
                    s.reachable = false;
                }
                if matches!(e.kind, ArrayGet { .. }) {
                    "array.get".into()
                } else {
                    "array.set".into()
                }
            }
            ArrayNew {
                initial, length, ..
            } => {
                self.eval(initial, s);
                let length = self.eval(length, s);
                value.length = length.range;
                value.length_key = length.length_of.or(length.key);
                "array.new".into()
            }
            ArrayFixed { values, .. } => {
                for e in values {
                    self.eval(e, s);
                }
                value.length = u32::try_from(values.len()).ok().map(Range::exact);
                "array.new_fixed".into()
            }
            StructNew { values, .. } | VariantNew { values, .. } => {
                for e in values {
                    self.eval(e, s);
                }
                "construct".into()
            }
            Call { function, args } => {
                for e in args {
                    self.eval(e, s);
                }
                format!("call {}", self.module.functions[function.0].name)
            }
            Loop { bindings, body, .. } => {
                for (_, initial) in bindings {
                    self.eval(initial, s);
                }
                for (id, _) in bindings {
                    s.bind(*id, Value::unknown(&self.function.locals[id.0].ty));
                }
                let ids = bindings.iter().map(|(id, _)| id.0).collect::<BTreeSet<_>>();
                s.less.retain(|(a, b)| !ids.contains(a) && !ids.contains(b));
                value = self.eval(body, s);
                "loop".into()
            }
            Recur { assignments, .. } => {
                for (_, e) in assignments {
                    self.eval(e, s);
                }
                s.reachable = false;
                "recur".into()
            }
            Match { subject, arms, .. } => {
                self.eval(subject, s);
                let base = s.clone();
                let mut normal: Option<(State, Value)> = None;
                for arm in arms {
                    let mut branch = base.clone();
                    for id in &arm.bindings {
                        branch.bind(*id, Value::unknown(&self.function.locals[id.0].ty));
                    }
                    let result = self.eval(&arm.body, &mut branch);
                    if branch.reachable {
                        normal = Some(match normal {
                            None => (branch, result),
                            Some((previous, old)) => {
                                (State::join(&previous, &branch), old.join(&result))
                            }
                        });
                    }
                }
                if let Some((state, result)) = normal {
                    *s = state;
                    value = result;
                } else {
                    s.reachable = false;
                }
                "match".into()
            }
            Try { body, handlers } => {
                let base = s.clone();
                let body_value = self.eval(body, s);
                let mut result_state = s.clone();
                value = body_value;
                for handler in handlers {
                    let mut branch = base.clone();
                    for id in &handler.bindings {
                        branch.bind(*id, Value::unknown(&self.function.locals[id.0].ty));
                    }
                    let result = self.eval(&handler.body, &mut branch);
                    if branch.reachable {
                        value = if result_state.reachable {
                            value.join(&result)
                        } else {
                            result
                        };
                    }
                    result_state = State::join(&result_state, &branch);
                }
                *s = result_state;
                "try".into()
            }
            Throw { values, .. } => {
                for e in values {
                    self.eval(e, s);
                }
                s.reachable = false;
                "throw".into()
            }
            Delivery { op, args, .. } => {
                for e in args {
                    self.eval(e, s);
                }
                format!("delivery.{op:?}")
            }
            ResourceNew { handle: e, .. }
            | ResourceClose(e)
            | ResourceHandle(e)
            | Close(e)
            | Drop(e) => {
                self.eval(e, s);
                "drop".into()
            }
            Nop => "nop".into(),
            Unreachable => {
                s.reachable = false;
                "unreachable".into()
            }
        };
        self.facts.push(ExpressionFacts {
            node,
            span: e.span,
            operation,
            reachable,
            normal_return_possible: s.reachable,
            i32_range: value.range,
            nullness: value.nullness,
            array_length: value.length,
            null_check_proven: null_check,
            bounds_proven: bounds,
        });
        value
    }
}

pub fn analyze(module: &Module, function: &Function) -> Vec<ExpressionFacts> {
    let Some(body) = &function.body else {
        return vec![];
    };
    let mut state = State {
        locals: function
            .locals
            .iter()
            .enumerate()
            .map(|(id, local)| {
                let mut value = Value::unknown(&local.ty);
                value.key = Some(id);
                value
            })
            .collect(),
        less: BTreeSet::new(),
        reachable: true,
    };
    let mut visitor = Visitor {
        module,
        function,
        next: 0,
        facts: vec![],
    };
    visitor.eval(body, &mut state);
    visitor.facts.sort_by_key(|fact| fact.node);
    visitor.facts
}
