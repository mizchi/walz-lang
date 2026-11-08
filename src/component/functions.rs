//! Core function boundaries and task-scoped cleanup.
use super::Adapter;
use super::abi::{align, core_type, load, pointer_at};
use crate::ir::*;

impl Adapter<'_> {
    pub(crate) fn imports(&self) -> String {
        let mut out = String::from(
            "(import \"__component\" \"memory\" (memory 1 256))\n(import \"__component\" \"alloc\" (func $__alloc (param i32) (result i32)))\n(import \"__component\" \"free\" (func $__free (param i32)))\n",
        );
        out.push_str(&crate::task_children::imports(self.module));
        out.push_str("(import \"__component\" \"cancel-requested\" (global $__cancel_requested (mut i32)))\n(import \"__component\" \"free-all\" (func $__free_all))\n");
        for (i, f) in self
            .module
            .functions
            .iter()
            .enumerate()
            .filter(|(_, f)| matches!(f.kind, FunctionKind::Import { .. }))
        {
            if f.import_contract.is_some() {
                out.push_str(&format!(
                    "(import \"__component\" \"host{i}\" (func $__host{i}{}))\n",
                    self.import_signature(f)
                ));
                continue;
            }
            let params = f
                .params
                .iter()
                .flat_map(|id| self.flat(&f.locals[id.0].ty))
                .map(|n| format!(" (param {})", n.wat()))
                .collect::<String>();
            let result = if f.result == Type::Unit {
                String::new()
            } else {
                format!(" (result {})", self.flat(&f.result)[0].wat())
            };
            out.push_str(&format!(
                "(import \"__component\" \"host{i}\" (func $__host{i}{params}{result}))\n"
            ));
        }
        for id in &self.endpoints {
            let future = matches!(self.value(*id), ValueType::Future(_));
            let kind = if future { "future" } else { "stream" };
            let n = id.0;
            out.push_str(&format!("(import \"__component\" \"read{n}\" (func $__read{n} (param i32 i32{}) (result i32)))\n(import \"__component\" \"drop{n}\" (func $__drop{n} (param i32)))\n(import \"__component\" \"ready{n}\" (func $__ready{n} (param i32 i32) (result i32)))\n",if future{""}else{" i32"}));
            if kind == "stream" {
                out.push_str(&format!("(import \"__component\" \"prepend{n}\" (func $__prepend{n} (param i32 i32 i32) (result i32)))\n"));
            }
        }
        crate::task_children::wrap_imports(self.module, out)
    }
    pub(crate) fn helpers(&self) -> String {
        let mut out = self.task_helpers();
        out.push_str(&crate::task_children::helpers(self.module));
        out.push_str(&self.value_helpers());
        for id in &self.endpoints {
            out.push_str(&self.endpoint_helpers(*id));
        }
        for (i, f) in self.module.functions.iter().enumerate() {
            if matches!(f.kind, FunctionKind::Import { .. }) {
                out.push_str(&self.host_wrapper(i, f));
            }
            if !f.exports.is_empty() && !self.module.tests.iter().any(|t| t.function.0 == i) {
                out.push_str(&self.export_wrapper(i, f));
            }
        }
        out
    }
    fn host_wrapper(&self, index: usize, f: &Function) -> String {
        if f.import_contract.is_some() {
            return self.value_host_wrapper(index, f);
        }
        let params = f
            .params
            .iter()
            .map(|id| {
                format!(
                    " (param $__v{} {})",
                    id.0,
                    core_type(&f.locals[id.0].ty, self.module)
                )
            })
            .collect::<String>();
        let mut locals = String::new();
        let mut init = String::new();
        let mut args = String::new();
        let mut cleanup = String::new();
        for id in &f.params {
            let ty = &f.locals[id.0].ty;
            let value = format!("(local.get $__v{})", id.0);
            match ty {
                Type::Num(_) => args.push_str(&format!(" {value}")),
                Type::Value(t) if matches!(self.value(*t), ValueType::Text | ValueType::Bytes) => {
                    let p = format!("(local.get $__p{})", id.0);
                    let len = format!("(local.get $__len{})", id.0);
                    locals.push_str(&format!(
                        " (local $__p{} i32) (local $__len{} i32)",
                        id.0, id.0
                    ));
                    // A flattened host argument needs ptr/len values, not an
                    // allocated in-memory descriptor. Array length is immutable.
                    init.push_str(&format!(
                        "(local.set $__len{} (array.len (struct.get {} 0 {value}))) (local.set $__p{} (call $__lower_flat_buffer_{} {value}))",
                        id.0,self.module.types[t.0].name,id.0,t.0
                    ));
                    args.push_str(&format!(" {p} {len}"));
                    cleanup.push_str(&format!("(if {len} (then (call $__free {p})))"));
                }
                _ => unreachable!("trusted binding parameters must have an implemented adapter"),
            }
        }
        let call = format!("(call $__host{index}{args})");
        let (result, body) = if f.result == Type::Unit {
            (String::new(), format!("{call} {cleanup}"))
        } else {
            locals.push_str(&format!(" (local $__r {})", self.flat(&f.result)[0].wat()));
            let value = "(local.get $__r)";
            let value = if let Type::Value(id) = f.result {
                self.remote(id, value)
            } else {
                value.into()
            };
            (
                format!(" (result {})", core_type(&f.result, self.module)),
                format!("(local.set $__r {call}) {cleanup} {value}"),
            )
        };
        format!(
            "(func {}{params}{result}{locals} {init} {body} (call $__checkpoint))\n",
            f.name
        )
    }

    fn task_helpers(&self) -> String {
        let drops = self.endpoints.iter().map(|id|format!("(if (i32.eq (struct.get $__task_entry 0 (ref.as_non_null (local.get $e))) (i32.const {})) (then (call $__drop{} (struct.get $__task_entry 1 (ref.as_non_null (local.get $e))))))",id.0,id.0)).collect::<String>();
        let drops = if self.native_resources {
            format!("{drops}{}",self.module.types.iter().enumerate().filter_map(|(n,t)|{
                let TypeKind::Resource{close,..}=t.kind else{return None};
                Some(format!("(if (i32.eq (struct.get $__task_entry 0 (ref.as_non_null (local.get $e))) (i32.const {n})) (then (call {} (struct.get $__task_entry 1 (ref.as_non_null (local.get $e))))))",self.module.functions[close.0].name))
            }).collect::<String>())
        } else {
            drops
        };
        let owner = if crate::task_children::enabled(self.module) {
            "(global.get $__current_scope)"
        } else {
            "(i32.const 0)"
        };
        let checkpoint = if crate::task_children::enabled(self.module) {
            "(if (call $__scope_enter (global.get $__current_scope)) (then (throw $__task_cancel)))"
        } else {
            ""
        };
        let scoped = if crate::task_children::enabled(self.module) {
            format!("(func (export \"__scope_cleanup\") (param $scope i32) (local $e (ref null $__task_entry)) (local $next (ref null $__task_entry)) (local $previous_scope i32)
          (local.set $previous_scope (global.get $__current_scope)) (global.set $__current_scope (local.get $scope))
          (local.set $e (global.get $__task_owned)) (loop $again (if (ref.is_null (local.get $e)) (then) (else
            (local.set $next (struct.get $__task_entry 3 (ref.as_non_null (local.get $e))))
            (if (i32.eq (struct.get $__task_entry 5 (ref.as_non_null (local.get $e))) (local.get $scope)) (then {drops} (call $__task_release (ref.as_non_null (local.get $e)))))
            (local.set $e (local.get $next)) (br $again)))) (global.set $__current_scope (local.get $previous_scope)))")
        } else {
            String::new()
        };
        format!("{scoped} (tag $__task_cancel (export \"__task_cancel\"))
          (global $__task_owned (mut (ref null $__task_entry)) (ref.null $__task_entry))
          (func $__checkpoint (if (global.get $__cancel_requested) (then (throw $__task_cancel))) {checkpoint})
          (func $__task_register (param $kind i32) (param $handle i32) (result (ref $__task_entry))
            (local $e (ref $__task_entry))
            (local.set $e (struct.new $__task_entry (local.get $kind) (local.get $handle) (i32.const 1) (global.get $__task_owned) (ref.null $__task_entry) {owner}))
            (if (ref.is_null (global.get $__task_owned)) (then) (else (struct.set $__task_entry 4 (ref.as_non_null (global.get $__task_owned)) (local.get $e))))
            (global.set $__task_owned (local.get $e)) (local.get $e))
          (func $__task_release (param $e (ref $__task_entry))
            (local $next (ref null $__task_entry)) (local $previous (ref null $__task_entry))
            (if (i32.eqz (struct.get $__task_entry 2 (local.get $e))) (then (unreachable)))
            (local.set $next (struct.get $__task_entry 3 (local.get $e)))
            (local.set $previous (struct.get $__task_entry 4 (local.get $e)))
            (if (ref.is_null (local.get $previous)) (then (global.set $__task_owned (local.get $next)))
              (else (struct.set $__task_entry 3 (ref.as_non_null (local.get $previous)) (local.get $next))))
            (if (ref.is_null (local.get $next)) (then) (else (struct.set $__task_entry 4 (ref.as_non_null (local.get $next)) (local.get $previous))))
            (struct.set $__task_entry 2 (local.get $e) (i32.const 0))
            (struct.set $__task_entry 3 (local.get $e) (ref.null $__task_entry))
            (struct.set $__task_entry 4 (local.get $e) (ref.null $__task_entry)))
          (func $__task_cleanup (export \"__task_cleanup\") (local $e (ref null $__task_entry))
            (loop $again (local.set $e (global.get $__task_owned))
              (if (ref.is_null (local.get $e)) (then) (else
                {drops} (call $__task_release (ref.as_non_null (local.get $e))) (br $again))))
            (global.set $__task_owned (ref.null $__task_entry)) (call $__free_all))\n")
    }
    fn export_wrapper(&self, index: usize, f: &Function) -> String {
        let params = f
            .params
            .iter()
            .map(|id| &f.locals[id.0].ty)
            .collect::<Vec<_>>();
        let flat = params.iter().flat_map(|t| self.flat(t)).collect::<Vec<_>>();
        let indirect = flat.len() > 16 || self.memory_exports && !params.is_empty();
        let signature = if indirect {
            " (param $args i32)".into()
        } else {
            flat.iter()
                .enumerate()
                .map(|(i, n)| format!(" (param $a{i} {})", n.wat()))
                .collect::<String>()
        };
        let result_flat = if f.result == Type::Unit {
            vec![]
        } else {
            self.flat(&f.result)
        };
        let indirect_result =
            result_flat.len() > 1 || self.memory_exports && !result_flat.is_empty();
        let result = if result_flat.is_empty() {
            String::new()
        } else {
            format!(
                " (result {})",
                if indirect_result {
                    "i32"
                } else {
                    result_flat[0].wat()
                }
            )
        };
        let mut locals = String::from(" (local $p i32)");
        let child_tasks = crate::task_children::enabled(self.module);
        // The hidden scope parameter exists module-wide, but only suspending
        // boundaries can spawn/wait children. Pure exports must stay synchronous.
        let scoped = child_tasks
            && self
                .module
                .abi_suspends
                .get(&f.name)
                .copied()
                .unwrap_or(false);
        if child_tasks {
            locals.push_str(" (local $__scope i32)");
        }
        let mut init = String::new();
        let mut slot = 0;
        let mut offset = 0;
        for (i, ty) in params.iter().enumerate() {
            locals.push_str(&format!(" (local $v{i} {})", core_type(ty, self.module)));
            let lift = if indirect {
                offset = align(offset, self.layout(ty).align);
                let code = self.lift(ty, &pointer_at("(local.get $args)", offset));
                offset += self.layout(ty).size;
                code
            } else {
                let n = self.flat(ty).len();
                let v = flat[slot..slot + n]
                    .iter()
                    .enumerate()
                    .map(|(j, t)| (format!("(local.get $a{})", slot + j), *t))
                    .collect::<Vec<_>>();
                slot += n;
                self.flat_lift(ty, &v)
            };
            init.push_str(&format!(" (local.set $v{i} {lift})"));
        }
        if indirect {
            init.push_str(" (call $__free (local.get $args))");
        }
        init.push_str(" (call $__checkpoint)");
        if scoped {
            init.push_str(" (local.set $__scope (call $__scope_new (i32.const 0)))");
        }
        let args = (0..params.len())
            .map(|i| format!(" (local.get $v{i})"))
            .collect::<String>();
        let call = format!(
            "(call {}{}{args})",
            f.name,
            if child_tasks {
                " (local.get $__scope)"
            } else {
                ""
            }
        );
        let body = if f.result == Type::Unit {
            format!("{init} {call}")
        } else {
            locals.push_str(&format!(
                " (local $r {})",
                core_type(&f.result, self.module)
            ));
            let layout = self.layout(&f.result);
            let lower = self.lower(&f.result, "(local.get $r)", "(local.get $p)");
            let finish = if result_flat.is_empty() {
                "(call $__free (local.get $p))".into()
            } else if indirect_result {
                "(local.get $p)".into()
            } else {
                let read = if self.interface_type(&f.result).is_some() {
                    self.flat_load(&f.result, "(local.get $p)")[0].0.clone()
                } else if matches!(f.result,Type::Value(id) if matches!(self.value(id),ValueType::Result{..}))
                {
                    "(i32.load8_u (local.get $p))".into()
                } else {
                    load(result_flat[0], "(local.get $p)")
                };
                locals.push_str(&format!(" (local $flat {})", result_flat[0].wat()));
                format!("(local.set $flat {read}) (call $__free (local.get $p)) (local.get $flat)")
            };
            format!(
                "{init} (local.set $r {call}) (local.set $p (call $__alloc (i32.const {}))) {lower} {finish}",
                layout.size
            )
        };
        let borrowed = params
            .iter()
            .enumerate()
            .filter_map(|(n, t)| {
                let Type::Borrow(id) = t else { return None };
                let TypeKind::Resource { close, .. } = self.module.types[id.0].kind else {
                    unreachable!()
                };
                Some(format!(
                    "(call {} (struct.get {} 0 (local.get $v{n})))",
                    self.module.functions[close.0].name, self.module.types[id.0].name
                ))
            })
            .collect::<String>();
        let mut out = format!(
            "(func (export \"__export_{index}\"){signature}{result}{locals}
          (block $__cancelled (return (try_table{result} (catch $__task_cancel $__cancelled)
            {body} {borrowed} {} (global.set $__task_owned (ref.null $__task_entry)))))
          {} (call $__task_cleanup) (throw $__task_cancel))\n",
            if scoped {
                "(call $__scope_close (local.get $__scope)) (call $__checkpoint)"
            } else {
                ""
            },
            if scoped {
                "(call $__scope_close (local.get $__scope))"
            } else {
                ""
            }
        );
        if indirect_result {
            out.push_str(&format!(
                "(func (export \"__post_{index}\") (param i32) {} (call $__free (local.get 0)))\n",
                self.release(&f.result, "(local.get 0)")
            ));
        }
        out
    }
}
