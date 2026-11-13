//! Give foreign types a local WIT name in each printed scope. The decoded
//! graph retains validator identity; this independent printing copy adds `use`
//! aliases and rewrites only scope-local anonymous value/handle expressions.
use super::*;
use std::collections::HashMap;
use wit_parser::{Type as T, TypeDefKind as K, TypeOwner as Owner};

enum Slot {
    Interface(String),
    Import(wit_parser::WorldKey),
    Export(wit_parser::WorldKey),
}
pub(super) fn print(
    resolve: &wit_parser::Resolve,
    world: wit_parser::WorldId,
) -> Result<String, Error> {
    let mut r = resolve.clone();
    let scopes = r
        .interfaces
        .iter()
        .map(|(id, _)| Owner::Interface(id))
        .chain(r.worlds.iter().map(|(id, _)| Owner::World(id)))
        .collect::<Vec<_>>();
    let maximum = r.types.len().saturating_add(65536);
    for owner in scopes {
        let (types, functions) = match owner {
            Owner::Interface(id) => (
                r.interfaces[id]
                    .types
                    .iter()
                    .map(|(n, t)| (n.clone(), *t))
                    .collect::<Vec<_>>(),
                r.interfaces[id]
                    .functions
                    .iter()
                    .map(|(name, f)| (Slot::Interface(name.clone()), f.clone()))
                    .collect::<Vec<_>>(),
            ),
            Owner::World(id) => (
                r.worlds[id]
                    .imports
                    .iter()
                    .filter_map(|(k, v)| {
                        if let wit_parser::WorldItem::Type { id, .. } = v {
                            Some((r.name_world_key(k), *id))
                        } else {
                            None
                        }
                    })
                    .collect(),
                r.worlds[id]
                    .imports
                    .iter()
                    .filter_map(|(key, item)| {
                        if let wit_parser::WorldItem::Function(f) = item {
                            Some((Slot::Import(key.clone()), f.clone()))
                        } else {
                            None
                        }
                    })
                    .chain(r.worlds[id].exports.iter().filter_map(|(key, item)| {
                        if let wit_parser::WorldItem::Function(f) = item {
                            Some((Slot::Export(key.clone()), f.clone()))
                        } else {
                            None
                        }
                    }))
                    .collect(),
            ),
            Owner::None => unreachable!(),
        };
        let mut names = types
            .iter()
            .map(|(n, _)| n.clone())
            .chain(functions.iter().map(|(_, f)| f.name.clone()))
            .collect::<BTreeSet<_>>();
        if let Owner::World(id) = owner {
            names.extend(
                r.worlds[id]
                    .imports
                    .keys()
                    .chain(r.worlds[id].exports.keys())
                    .map(|k| r.name_world_key(k)),
            );
        }
        let mut aliases = HashMap::new();
        for (_, id) in &types {
            if let K::Type(T::Id(original)) = r.types[*id].kind
                && r.types[original].owner != owner
            {
                aliases.insert(original, *id);
            }
        }
        let mut p = Scope {
            r: &mut r,
            owner,
            names,
            aliases,
            memo: HashMap::new(),
            maximum,
        };
        for (_, id) in types {
            // An explicit `use` is already an alias, not its own body.
            if matches!(p.r.types[id].kind,K::Type(T::Id(t)) if p.r.types[t].owner!=owner) {
                continue;
            }
            let kind = p.kind(p.r.types[id].kind.clone(), 0)?;
            p.r.types[id].kind = kind;
        }
        for (slot, mut f) in functions {
            for param in &mut f.params {
                param.ty = p.ty(param.ty, 0)?;
            }
            if let Some(t) = f.result {
                f.result = Some(p.ty(t, 0)?);
            }
            match (owner, slot) {
                (Owner::Interface(id), Slot::Interface(name)) => {
                    p.r.interfaces[id].functions.insert(name, f);
                }
                (Owner::World(id), Slot::Import(key)) => {
                    p.r.worlds[id]
                        .imports
                        .insert(key, wit_parser::WorldItem::Function(f));
                }
                (Owner::World(id), Slot::Export(key)) => {
                    p.r.worlds[id]
                        .exports
                        .insert(key, wit_parser::WorldItem::Function(f));
                }
                _ => unreachable!(),
            }
        }
    }
    let pkg = r.worlds[world]
        .package
        .ok_or_else(|| error("missing WIT package"))?;
    let packages = r
        .packages
        .iter()
        .map(|(id, _)| id)
        .filter(|id| *id != pkg)
        .collect::<Vec<_>>();
    let mut printer = wit_component::WitPrinter::default();
    printer.print(&r, pkg, &packages).map_err(error)?;
    let text = printer.output.to_string();
    wit_parser::Resolve::default()
        .push_str("world.wit", &text)
        .map_err(|e| error(format!("generated WIT: {e:#}")))?;
    Ok(text)
}
struct Scope<'a> {
    r: &'a mut wit_parser::Resolve,
    owner: Owner,
    names: BTreeSet<String>,
    aliases: HashMap<wit_parser::TypeId, wit_parser::TypeId>,
    memo: HashMap<wit_parser::TypeId, wit_parser::TypeId>,
    maximum: usize,
}
impl Scope<'_> {
    fn allocate(&mut self, t: wit_parser::TypeDef) -> Result<wit_parser::TypeId, Error> {
        if self.r.types.len() >= self.maximum {
            return Err(error("WIT scope projection type limit"));
        }
        Ok(self.r.types.alloc(t))
    }
    fn ty(&mut self, t: T, depth: usize) -> Result<T, Error> {
        if depth > 64 {
            return Err(error("WIT scope projection nesting limit"));
        }
        let T::Id(id) = t else {
            return Ok(t);
        };
        let original = self.r.types[id].clone();
        if let Some(base) = &original.name {
            if original.owner == self.owner {
                return Ok(t);
            }
            if let Some(alias) = self.aliases.get(&id) {
                return Ok(T::Id(*alias));
            }
            if !matches!(original.owner, Owner::Interface(_)) {
                return Err(error("a public named type needs an interface definition"));
            }
            let mut name = base.clone();
            let mut suffix = 0;
            while !self.names.insert(name.clone()) {
                suffix += 1;
                name = format!("{base}-a{suffix}");
            }
            let mut alias = original;
            alias.name = Some(name.clone());
            alias.kind = K::Type(T::Id(id));
            alias.owner = self.owner;
            let alias = self.allocate(alias)?;
            self.aliases.insert(id, alias);
            match self.owner {
                Owner::Interface(i) => {
                    self.r.interfaces[i].types.insert(name, alias);
                }
                Owner::World(w) => {
                    self.r.worlds[w].imports.insert(
                        wit_parser::WorldKey::Name(name),
                        wit_parser::WorldItem::Type {
                            id: alias,
                            span: Default::default(),
                        },
                    );
                }
                Owner::None => unreachable!(),
            }
            return Ok(T::Id(alias));
        }
        if let Some(id) = self.memo.get(&id) {
            return Ok(T::Id(*id));
        }
        let mut copy = original;
        copy.kind = self.kind(copy.kind, depth + 1)?;
        copy.owner = self.owner;
        let copy = self.allocate(copy)?;
        self.memo.insert(id, copy);
        Ok(T::Id(copy))
    }
    fn kind(&mut self, mut k: K, depth: usize) -> Result<K, Error> {
        match &mut k {
            K::Handle(wit_parser::Handle::Own(r) | wit_parser::Handle::Borrow(r)) => {
                let T::Id(id) = self.ty(T::Id(*r), depth + 1)? else {
                    unreachable!()
                };
                *r = id;
            }
            K::Type(t) | K::List(t) | K::Option(t) | K::FixedLengthList(t, _) => {
                *t = self.ty(*t, depth + 1)?
            }
            K::Tuple(t) => {
                for t in &mut t.types {
                    *t = self.ty(*t, depth + 1)?;
                }
            }
            K::Result(t) => {
                t.ok = t.ok.map(|t| self.ty(t, depth + 1)).transpose()?;
                t.err = t.err.map(|t| self.ty(t, depth + 1)).transpose()?;
            }
            K::Future(t) | K::Stream(t) => *t = t.map(|t| self.ty(t, depth + 1)).transpose()?,
            K::Map(a, b) => {
                *a = self.ty(*a, depth + 1)?;
                *b = self.ty(*b, depth + 1)?;
            }
            K::Record(r) => {
                for f in &mut r.fields {
                    f.ty = self.ty(f.ty, depth + 1)?;
                }
            }
            K::Variant(v) => {
                for c in &mut v.cases {
                    c.ty = c.ty.map(|t| self.ty(t, depth + 1)).transpose()?;
                }
            }
            _ => {}
        }
        Ok(k)
    }
}
