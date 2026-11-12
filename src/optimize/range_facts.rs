//! Must facts over immutable snapshots of unsigned i32 values. No signed or
//! wrapping-add comparison is interpreted as an unbounded range constraint.
use crate::ir::TypeId;
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Term = usize;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum Compare {
    Lt,
    Le,
    Eq,
    Ne,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum Node {
    Const(u32),
    Fresh(usize),
    Len(Term),
    Field(TypeId, usize, Term),
    Add(Term, Term),
    Sub(Term, Term),
    Cmp(Compare, Term, Term),
    Select(Term, Term, Term),
    Op(String, Vec<Term>),
}
#[derive(Default)]
pub(super) struct Arena {
    nodes: Vec<Node>,
    depths: Vec<usize>,
    index: BTreeMap<Node, Term>,
    next: usize,
}
impl Arena {
    pub(super) fn node(&self, t: Term) -> &Node {
        &self.nodes[t]
    }
    pub(super) fn fresh(&mut self) -> Term {
        self.next += 1;
        self.make(Node::Fresh(self.next))
    }
    pub(super) fn constant(&mut self, n: u32) -> Term {
        self.make(Node::Const(n))
    }
    pub(super) fn make(&mut self, node: Node) -> Term {
        let value = |t| match self.node(t) {
            Node::Const(n) => Some(*n),
            _ => None,
        };
        match node {
            Node::Add(a, b) => {
                if value(a) == Some(0) {
                    return b;
                }
                if value(b) == Some(0) {
                    return a;
                }
                if let (Some(a), Some(b)) = (value(a), value(b)) {
                    return self.constant(a.wrapping_add(b));
                }
            }
            Node::Sub(a, b) => {
                if a == b {
                    return self.constant(0);
                }
                if value(b) == Some(0) {
                    return a;
                }
                if let (Some(a), Some(b)) = (value(a), value(b)) {
                    return self.constant(a.wrapping_sub(b));
                }
            }
            _ => {}
        }
        let children = match &node {
            Node::Len(a) | Node::Field(_, _, a) => vec![*a],
            Node::Add(a, b) | Node::Sub(a, b) | Node::Cmp(_, a, b) => vec![*a, *b],
            Node::Select(a, b, c) => vec![*a, *b, *c],
            Node::Op(_, a) => a.clone(),
            _ => vec![],
        };
        let depth = children.iter().map(|t| self.depths[*t]).max().unwrap_or(0) + 1;
        if depth > 32 {
            return self.fresh();
        }
        if let Some(id) = self.index.get(&node) {
            return *id;
        }
        let id = self.nodes.len();
        self.index.insert(node.clone(), id);
        self.nodes.push(node);
        self.depths.push(depth);
        id
    }
    pub(super) fn sub(&self, a: Term, b: Term) -> Option<Term> {
        if matches!(self.node(b), Node::Const(0)) {
            Some(a)
        } else {
            self.index.get(&Node::Sub(a, b)).copied()
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct Facts {
    le: BTreeSet<(Term, Term)>,
    lt: BTreeSet<(Term, Term)>,
    pub(super) impossible: bool,
}
impl Facts {
    fn put(&mut self, a: Term, b: Term, strict: bool) {
        if self.le.len() + self.lt.len() < 128 {
            if strict {
                self.lt.insert((a, b));
            } else {
                self.le.insert((a, b));
            }
        }
    }
    pub(super) fn join(a: &Self, b: &Self) -> Self {
        if a.impossible {
            return b.clone();
        }
        if b.impossible {
            return a.clone();
        }
        Self {
            le: a.le.intersection(&b.le).copied().collect(),
            lt: a.lt.intersection(&b.lt).copied().collect(),
            impossible: false,
        }
    }
    fn le(&self, a: Term, b: Term, arena: &Arena) -> bool {
        if a == b
            || matches!(arena.node(a), Node::Const(0))
            || matches!(arena.node(b), Node::Const(u32::MAX))
        {
            return true;
        }
        if let (Node::Const(a), Node::Const(b)) = (arena.node(a), arena.node(b)) {
            return a <= b;
        }
        if self.le.contains(&(a, b)) || self.lt.contains(&(a, b)) {
            return true;
        }
        // Width one fits after an unsigned strict index guard, without subtraction wrap.
        if matches!(arena.node(a), Node::Const(1))
            && let Node::Sub(end, start) = arena.node(b)
            && self.lt(*start, *end, arena)
        {
            return true;
        }
        let mut pending = vec![a];
        let mut seen = BTreeSet::new();
        while let Some(t) = pending.pop() {
            if !seen.insert(t) || seen.len() > 128 {
                continue;
            }
            for &(left, right) in self.le.iter().chain(&self.lt) {
                let begins = left == t
                    || matches!((arena.node(t),arena.node(left)), (Node::Const(x),Node::Const(y)) if x <= y);
                if begins {
                    if right == b
                        || matches!((arena.node(right),arena.node(b)), (Node::Const(x),Node::Const(y)) if x <= y)
                    {
                        return true;
                    }
                    pending.push(right);
                }
            }
        }
        false
    }
    fn lt(&self, a: Term, b: Term, arena: &Arena) -> bool {
        if let (Node::Const(a), Node::Const(b)) = (arena.node(a), arena.node(b)) {
            return a < b;
        }
        self.lt.contains(&(a, b))
    }
    pub(super) fn truth(&self, term: Term, arena: &Arena) -> Option<bool> {
        self.truth_limited(term, arena, &mut 256)
    }
    fn truth_limited(&self, term: Term, arena: &Arena, budget: &mut usize) -> Option<bool> {
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        match arena.node(term) {
            Node::Const(n) => Some(*n != 0),
            Node::Cmp(c, a, b) => {
                // Normalized &&/|| include `ne condition 0` wrappers.
                if matches!(c, Compare::Eq | Compare::Ne)
                    && matches!(arena.node(*b), Node::Const(0))
                    && let Some(value) = self.truth_limited(*a, arena, budget)
                {
                    return Some(if *c == Compare::Eq { !value } else { value });
                }
                match c {
                    Compare::Lt if self.lt(*a, *b, arena) => Some(true),
                    Compare::Lt if self.le(*b, *a, arena) => Some(false),
                    Compare::Le if self.le(*a, *b, arena) => Some(true),
                    Compare::Le if self.lt(*b, *a, arena) => Some(false),
                    Compare::Eq | Compare::Ne
                        if self.le(*a, *b, arena) && self.le(*b, *a, arena) =>
                    {
                        Some(*c == Compare::Eq)
                    }
                    Compare::Eq | Compare::Ne
                        if self.lt(*a, *b, arena) || self.lt(*b, *a, arena) =>
                    {
                        Some(*c == Compare::Ne)
                    }
                    _ => None,
                }
            }
            Node::Select(c, a, b) => match self.truth_limited(*c, arena, budget) {
                Some(true) => self.truth_limited(*a, arena, budget),
                Some(false) => self.truth_limited(*b, arena, budget),
                None => {
                    let a = self.truth_limited(*a, arena, budget);
                    (a == self.truth_limited(*b, arena, budget))
                        .then_some(a)
                        .flatten()
                }
            },
            _ => None,
        }
    }
    pub(super) fn assume(&mut self, term: Term, truth: bool, arena: &Arena) {
        self.assume_limited(term, truth, arena, &mut 256);
    }
    fn assume_limited(&mut self, term: Term, truth: bool, arena: &Arena, budget: &mut usize) {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        if let Some(known) = self.truth(term, arena) {
            if known != truth {
                self.impossible = true;
            }
            return;
        }
        match *arena.node(term) {
            Node::Cmp(c, a, b) => {
                if matches!(c, Compare::Eq | Compare::Ne) && matches!(arena.node(b), Node::Const(0))
                {
                    self.assume_limited(
                        a,
                        if c == Compare::Eq { !truth } else { truth },
                        arena,
                        budget,
                    );
                    return;
                }
                match (c, truth) {
                    (Compare::Lt, true) | (Compare::Le, false) => {
                        self.put(if truth { a } else { b }, if truth { b } else { a }, true)
                    }
                    (Compare::Le, true) | (Compare::Lt, false) => {
                        self.put(if truth { a } else { b }, if truth { b } else { a }, false)
                    }
                    (Compare::Eq, true) | (Compare::Ne, false) => {
                        self.put(a, b, false);
                        self.put(b, a, false);
                    }
                    _ => {}
                }
            }
            Node::Select(c, a, b) => {
                let mut yes = self.clone();
                yes.assume_limited(c, true, arena, budget);
                yes.assume_limited(a, truth, arena, budget);
                let mut no = self.clone();
                no.assume_limited(c, false, arena, budget);
                no.assume_limited(b, truth, arena, budget);
                *self = Self::join(&yes, &no);
            }
            _ => {}
        }
    }
    pub(super) fn establish_range(
        &mut self,
        length: Term,
        start: Term,
        width: Term,
        arena: &mut Arena,
    ) {
        self.put(start, length, false);
        let remaining = arena.make(Node::Sub(length, start));
        self.put(width, remaining, false);
    }
    pub(super) fn fits(&self, length: Term, start: Term, width: Term, arena: &Arena) -> bool {
        let mut memo = BTreeMap::new();
        let mut budget = 256;
        self.fit(length, start, width, arena, &mut memo, &mut budget)
    }
    fn fit(
        &self,
        length: Term,
        start: Term,
        width: Term,
        arena: &Arena,
        memo: &mut BTreeMap<(Term, Term, Term), bool>,
        budget: &mut usize,
    ) -> bool {
        let key = (length, start, width);
        if let Some(value) = memo.get(&key) {
            return *value;
        }
        if *budget == 0 {
            return false;
        }
        *budget -= 1;
        let direct = self.le(start, length, arena)
            && (matches!(arena.node(width), Node::Const(0))
                || matches!(arena.node(width), Node::Const(1)) && self.lt(start, length, arena)
                || arena
                    .sub(length, start)
                    .is_some_and(|n| self.le(width, n, arena)));
        if direct {
            memo.insert(key, true);
            return true;
        }
        if let Node::Add(a, b) = *arena.node(start) {
            // Compose two nonwrapping subranges. No conclusion is drawn from
            // `start + width <= length` alone, since i32 addition can wrap.
            let spans = self
                .le
                .iter()
                .filter_map(|(n, remaining)| {
                    matches!(arena.node(*remaining), Node::Sub(..)).then_some(*n)
                })
                .collect::<BTreeSet<_>>();
            for span in spans {
                for (base, index) in [(a, b), (b, a)] {
                    if self.fit(length, base, span, arena, memo, budget)
                        && self.fit(span, index, width, arena, memo, budget)
                    {
                        memo.insert(key, true);
                        return true;
                    }
                }
            }
        }
        memo.insert(key, false);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_term_dags_exhaust_queries_conservatively() {
        let mut arena = Arena::default();
        let mut condition = arena.constant(1);
        for _ in 0..20 {
            let unknown = arena.fresh();
            condition = arena.make(Node::Select(unknown, condition, condition));
        }
        let mut facts = Facts::default();
        assert_eq!(facts.truth(condition, &arena), None);
        facts.assume(condition, false, &arena);
        assert!(!facts.impossible);
    }
}
