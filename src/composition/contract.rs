//! WIT ABI equality. Resolve::merge intentionally omits defined-type equality;
//! it must not be used as a validator. Resource definitions are paired through
//! actual interface/type ports, with a bijection that preserves alias identity.
use super::*;
use wit_parser::{Resolve, Type as T, TypeDefKind as K, TypeId, WorldId, WorldItem as Item};
fn fail(message: impl Into<String>) -> Error {
    Error::message(format!("WIT contract differs: {}", message.into())).with_code("E_WIT_WORLD")
}
pub(super) fn validate(a: &Resolve, aw: WorldId, b: &Resolve, bw: WorldId) -> Result<(), Error> {
    let mut c = Compare {
        a,
        b,
        resources: BTreeMap::new(),
        reverse: BTreeMap::new(),
        seen: BTreeSet::new(),
        steps: 0,
    };
    let ports = |r: &Resolve, w: WorldId, export: bool| {
        let items = if export {
            &r.worlds[w].exports
        } else {
            &r.worlds[w].imports
        };
        items
            .iter()
            .filter(
                |(_, v)| !matches!(v,Item::Type{id,..} if !matches!(r.types[*id].kind,K::Resource)),
            )
            .map(|(k, v)| (r.name_world_key(k), v.clone()))
            .collect::<BTreeMap<_, _>>()
    };
    let mut pairs = vec![];
    for export in [false, true] {
        let aa = ports(a, aw, export);
        let bb = ports(b, bw, export);
        if aa.keys().ne(bb.keys()) {
            return Err(fail("import/export port names differ"));
        }
        pairs.extend(
            aa.into_iter()
                .zip(bb)
                .map(|((name, a), (_, b))| (name, a, b)),
        );
    }
    // Register resource definitions before following function references. This
    // also pairs resource aliases exported by a different interface.
    for (_, a, b) in &pairs {
        c.register(a, b)?;
    }
    for (name, a, b) in pairs {
        c.item(&a, &b)
            .map_err(|e| fail(format!("port {name}: {e}")))?;
    }
    Ok(())
}
struct Compare<'a> {
    a: &'a Resolve,
    b: &'a Resolve,
    resources: BTreeMap<TypeId, TypeId>,
    reverse: BTreeMap<TypeId, TypeId>,
    seen: BTreeSet<(TypeId, TypeId)>,
    steps: usize,
}
fn peel(r: &Resolve, mut t: T) -> Result<T, Error> {
    for _ in 0..64 {
        if let T::Id(id) = t
            && let K::Type(next) = r.types[id].kind
        {
            t = next;
        } else {
            return Ok(t);
        }
    }
    Err(fail("type alias nesting limit"))
}
fn resource(r: &Resolve, t: TypeId) -> Option<TypeId> {
    let T::Id(id) = peel(r, T::Id(t)).ok()? else {
        return None;
    };
    matches!(r.types[id].kind, K::Resource).then_some(id)
}
impl Compare<'_> {
    fn pair_resource(&mut self, a: TypeId, b: TypeId) -> Result<(), Error> {
        match (resource(self.a, a), resource(self.b, b)) {
            (Some(a), Some(b)) => {
                if self.resources.get(&a).is_some_and(|v| *v != b)
                    || self.reverse.get(&b).is_some_and(|v| *v != a)
                {
                    return Err(fail("resource alias identity differs"));
                }
                self.resources.insert(a, b);
                self.reverse.insert(b, a);
                Ok(())
            }
            (None, None) => Ok(()),
            _ => Err(fail("a resource definition changed to a value type")),
        }
    }
    fn register(&mut self, a: &Item, b: &Item) -> Result<(), Error> {
        match (a, b) {
            (Item::Interface { id: a, .. }, Item::Interface { id: b, .. }) => {
                let a = &self.a.interfaces[*a];
                let b = &self.b.interfaces[*b];
                if a.types.keys().collect::<BTreeSet<_>>()
                    != b.types.keys().collect::<BTreeSet<_>>()
                    || a.functions.keys().collect::<BTreeSet<_>>()
                        != b.functions.keys().collect::<BTreeSet<_>>()
                {
                    return Err(fail("interface member names differ"));
                }
                for (name, id) in &a.types {
                    self.pair_resource(*id, b.types[name])?;
                }
                Ok(())
            }
            (Item::Type { id: a, .. }, Item::Type { id: b, .. }) => self.pair_resource(*a, *b),
            (Item::Function(_), Item::Function(_)) => Ok(()),
            _ => Err(fail("port kinds differ")),
        }
    }
    fn item(&mut self, a: &Item, b: &Item) -> Result<(), Error> {
        match (a, b) {
            (Item::Function(a), Item::Function(b)) => self.function(a, b),
            (Item::Interface { id: a, .. }, Item::Interface { id: b, .. }) => {
                let a = &self.a.interfaces[*a];
                let b = &self.b.interfaces[*b];
                for (name, id) in &a.types {
                    self.ty(T::Id(*id), T::Id(b.types[name]), 0)?;
                }
                for (name, f) in &a.functions {
                    self.function(f, &b.functions[name])?;
                }
                Ok(())
            }
            (Item::Type { id: a, .. }, Item::Type { id: b, .. }) => {
                self.ty(T::Id(*a), T::Id(*b), 0)
            }
            _ => Err(fail("port kinds differ")),
        }
    }
    fn function(
        &mut self,
        a: &wit_parser::Function,
        b: &wit_parser::Function,
    ) -> Result<(), Error> {
        if std::mem::discriminant(&a.kind) != std::mem::discriminant(&b.kind)
            || a.params.len() != b.params.len()
        {
            return Err(fail(format!("function {} kind or arity differs", a.name)));
        }
        if let (Some(a), Some(b)) = (a.kind.resource(), b.kind.resource()) {
            self.ty(T::Id(a), T::Id(b), 0)?;
        }
        for (a, b) in a.params.iter().zip(&b.params) {
            if a.name != b.name {
                return Err(fail("parameter names differ"));
            }
            self.ty(a.ty, b.ty, 0)?;
        }
        self.optional(a.result, b.result, 0)
    }
    fn optional(&mut self, a: Option<T>, b: Option<T>, depth: usize) -> Result<(), Error> {
        match (a, b) {
            (Some(a), Some(b)) => self.ty(a, b, depth + 1),
            (None, None) => Ok(()),
            _ => Err(fail("optional type payload differs")),
        }
    }
    fn ty(&mut self, a: T, b: T, depth: usize) -> Result<(), Error> {
        self.steps += 1;
        if self.steps > 65536 || depth > 64 {
            return Err(fail("type comparison limit"));
        }
        let a = peel(self.a, a)?;
        let b = peel(self.b, b)?;
        let (T::Id(a), T::Id(b)) = (a, b) else {
            return if a == b {
                Ok(())
            } else {
                Err(fail("primitive types differ"))
            };
        };
        if !self.seen.insert((a, b)) {
            return Ok(());
        }
        match (&self.a.types[a].kind, &self.b.types[b].kind) {
            (K::Resource, K::Resource) => {
                if self.resources.get(&a) == Some(&b) {
                    Ok(())
                } else {
                    Err(fail("resource definition identity differs"))
                }
            }
            (K::Handle(wit_parser::Handle::Own(a)), K::Handle(wit_parser::Handle::Own(b)))
            | (
                K::Handle(wit_parser::Handle::Borrow(a)),
                K::Handle(wit_parser::Handle::Borrow(b)),
            ) => self.ty(T::Id(*a), T::Id(*b), depth + 1),
            (K::List(a), K::List(b)) | (K::Option(a), K::Option(b)) => self.ty(*a, *b, depth + 1),
            (K::FixedLengthList(a, n), K::FixedLengthList(b, m)) if n == m => {
                self.ty(*a, *b, depth + 1)
            }
            (K::Map(a, aa), K::Map(b, bb)) => {
                self.ty(*a, *b, depth + 1)?;
                self.ty(*aa, *bb, depth + 1)
            }
            (K::Tuple(a), K::Tuple(b)) if a.types.len() == b.types.len() => {
                for (a, b) in a.types.iter().zip(&b.types) {
                    self.ty(*a, *b, depth + 1)?;
                }
                Ok(())
            }
            (K::Record(a), K::Record(b)) if a.fields.len() == b.fields.len() => {
                for (a, b) in a.fields.iter().zip(&b.fields) {
                    if a.name != b.name {
                        return Err(fail("record field names differ"));
                    }
                    self.ty(a.ty, b.ty, depth + 1)?;
                }
                Ok(())
            }
            (K::Variant(a), K::Variant(b)) if a.cases.len() == b.cases.len() => {
                for (a, b) in a.cases.iter().zip(&b.cases) {
                    if a.name != b.name {
                        return Err(fail("variant case names differ"));
                    }
                    self.optional(a.ty, b.ty, depth + 1)?;
                }
                Ok(())
            }
            (K::Result(a), K::Result(b)) => {
                self.optional(a.ok, b.ok, depth + 1)?;
                self.optional(a.err, b.err, depth + 1)
            }
            (K::Future(a), K::Future(b)) | (K::Stream(a), K::Stream(b)) => {
                self.optional(*a, *b, depth + 1)
            }
            (K::Enum(a), K::Enum(b))
                if a.cases
                    .iter()
                    .map(|c| &c.name)
                    .eq(b.cases.iter().map(|c| &c.name)) =>
            {
                Ok(())
            }
            (K::Flags(a), K::Flags(b))
                if a.flags
                    .iter()
                    .map(|c| &c.name)
                    .eq(b.flags.iter().map(|c| &c.name)) =>
            {
                Ok(())
            }
            _ => Err(fail("defined types differ (including own/borrow)")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const WIT: &str = r#"
        package test:contracts;
        interface api {
            resource file;
            resource socket;
            type visible = file;
            record entry { number: u32, handle: option<visible> }
            variant outcome { found(entry), missing }
            exchange: func(value: outcome) -> result<list<entry>, string>;
            later: async func() -> future<u32>;
            events: func() -> stream<u8>;
        }
        world app { import api; export api; }
    "#;
    fn graph(text: &str) -> (Resolve, WorldId) {
        let mut resolve = Resolve::default();
        let pkg = resolve.push_str("contract.wit", text).unwrap();
        let world = resolve.select_world(&[pkg], Some("app")).unwrap();
        (resolve, world)
    }
    #[test]
    fn defined_payloads_and_resource_aliases_are_checked_instead_of_only_ports() {
        let (actual, world) = graph(WIT);
        validate(&actual, world, &actual, world).unwrap();
        for (before, after) in [
            ("number: u32", "number: s32"),
            ("number: u32", "renamed: u32"),
            ("option<visible>", "list<visible>"),
            ("found(entry)", "found(u32)"),
            ("result<list<entry>, string>", "result<list<entry>, u32>"),
            ("future<u32>", "future<s32>"),
            ("stream<u8>", "stream<s8>"),
            ("later: async func", "later: func"),
            ("type visible = file", "type visible = socket"),
        ] {
            let (expected, expected_world) = graph(&WIT.replace(before, after));
            assert!(
                validate(&actual, world, &expected, expected_world).is_err(),
                "accepted change {before} -> {after}"
            );
        }
    }
}
