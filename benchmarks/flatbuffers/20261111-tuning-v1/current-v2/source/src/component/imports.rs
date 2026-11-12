//! General value imports. Core/JSPI uses records; Components use canonical flats.
use super::{
    Adapter,
    abi::{align, coerce, core_type, load, pointer_at},
};
use crate::ir::*;

impl Adapter<'_> {
    pub(super) fn import_signature(&self, f: &Function) -> String {
        if self.memory_exports {
            return " (param i32 i32)".into();
        }
        let flat = f
            .params
            .iter()
            .flat_map(|id| self.flat(&f.locals[id.0].ty))
            .collect::<Vec<_>>();
        let mut signature = if flat.len() > 16 {
            " (param i32)".into()
        } else {
            flat.iter()
                .map(|n| format!(" (param {})", n.wat()))
                .collect::<String>()
        };
        if f.result != Type::Unit {
            let flat = self.flat(&f.result);
            signature.push_str(&if flat.is_empty() {
                String::new()
            } else if flat.len() > 1 {
                " (param i32)".into()
            } else {
                format!(" (result {})", flat[0].wat())
            });
        }
        signature
    }

    pub(super) fn value_host_wrapper(&self, index: usize, f: &Function) -> String {
        let mut signature = f
            .params
            .iter()
            .map(|id| {
                format!(
                    " (param $v{} {})",
                    id.0,
                    core_type(&f.locals[id.0].ty, self.module)
                )
            })
            .collect::<String>();
        if f.result != Type::Unit {
            signature.push_str(&format!(" (result {})", core_type(&f.result, self.module)));
        }
        let mut offset = 0;
        let mut init = String::new();
        let mut cleanup = String::new();
        let mut args = Vec::new();
        for id in &f.params {
            let ty = &f.locals[id.0].ty;
            offset = align(offset, self.layout(ty).align);
            let p = pointer_at("(local.get $args)", offset);
            init.push_str(&self.lower(ty, &format!("(local.get $v{})", id.0), &p));
            cleanup.push_str(&self.release(ty, &p));
            args.extend(self.flat_load(ty, &p).into_iter().map(|(s, _)| s));
            offset += self.layout(ty).size;
        }
        let mut out = format!(
            "(func {}{signature} (local $args i32) (local $out i32)",
            f.name
        );
        if f.result != Type::Unit {
            out.push_str(&format!(
                " (local $r {})",
                core_type(&f.result, self.module)
            ));
        }
        out.push_str(&format!(
            "(local.set $args (call $__alloc (i32.const {}))) {init}",
            offset.max(1)
        ));
        let flat_result = if f.result == Type::Unit {
            vec![]
        } else {
            self.flat(&f.result)
        };
        if self.memory_exports || args.len() > 16 {
            args = vec!["(local.get $args)".into()];
        }
        if f.result != Type::Unit {
            out.push_str(&format!(
                "(local.set $out (call $__alloc (i32.const {})))",
                self.layout(&f.result).size
            ));
        }
        if self.memory_exports || flat_result.len() > 1 {
            args.push("(local.get $out)".into());
        }
        let call = format!("(call $__host{index} {})", args.join(" "));
        if !self.memory_exports && flat_result.len() == 1 {
            out.push_str(&format!(
                "({}.store (local.get $out) {call})",
                flat_result[0].wat()
            ));
        } else {
            out.push_str(&call);
        }
        // Lift copies buffers and frees their payloads. Only descriptors remain.
        if f.result != Type::Unit {
            out.push_str(&format!(
                "(local.set $r {}) (call $__free (local.get $out))",
                self.lift(&f.result, "(local.get $out)")
            ));
        }
        out.push_str(&format!(
            "{cleanup} (call $__free (local.get $args)) (call $__checkpoint)"
        ));
        if f.result != Type::Unit {
            out.push_str("(local.get $r)");
        }
        out.push_str(")\n");
        out
    }

    pub(super) fn flat_load(&self, ty: &Type, p: &str) -> Vec<(String, NumType)> {
        if self.interface_type(ty).is_some() {
            return self.interface_flat_load(ty, p);
        }
        match ty {
            Type::Resource(_) | Type::Borrow(_) => vec![(load(NumType::I32, p), NumType::I32)],
            Type::Num(n) => vec![(load(*n, p), *n)],
            Type::Unit => vec![(format!("(i32.load8_u {p})"), NumType::I32)],
            Type::Value(id) => match self.value(*id) {
                ValueType::List(_) | ValueType::Text | ValueType::Bytes => vec![
                    (load(NumType::I32, p), NumType::I32),
                    (load(NumType::I32, &pointer_at(p, 4)), NumType::I32),
                ],
                ValueType::Future(_) | ValueType::Stream(_) => {
                    vec![(load(NumType::I32, p), NumType::I32)]
                }
                ValueType::Pair { first, second } => [
                    self.flat_load(first, p),
                    self.flat_load(
                        second,
                        &pointer_at(p, align(self.layout(first).size, self.layout(second).align)),
                    ),
                ]
                .concat(),
                _ => {
                    let tag = format!("(i32.load8_u {p})");
                    let payload = pointer_at(p, align(1, self.layout(ty).align));
                    let cases = self
                        .cases(*id)
                        .iter()
                        .map(|t| {
                            t.as_ref()
                                .map(|t| self.flat_load(t, &payload))
                                .unwrap_or_default()
                        })
                        .collect::<Vec<_>>();
                    let mut out = vec![(tag.clone(), NumType::I32)];
                    for (i, want) in self.flat(ty).iter().skip(1).enumerate() {
                        let values = cases
                            .iter()
                            .map(|c| {
                                c.get(i)
                                    .map(|(s, have)| coerce(s, *have, *want))
                                    .unwrap_or_else(|| format!("({}.const 0)", want.wat()))
                            })
                            .collect::<Vec<_>>();
                        out.push((
                            format!(
                                "(if (result {}) (i32.eqz {tag}) (then {}) (else {}))",
                                want.wat(),
                                values[0],
                                values[1]
                            ),
                            *want,
                        ));
                    }
                    out
                }
            },
            _ => unreachable!("validated portable import"),
        }
    }
}
