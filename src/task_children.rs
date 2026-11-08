//! Private numeric scheduler ABI; closure environments/results stay in GC.
use crate::ir::*;

pub(crate) fn validate(source: &str, module: &Module) -> Result<(), crate::Error> {
    if !enabled(module) {
        return Ok(());
    }
    let effects = crate::effects::infer(module);
    for f in &module.functions {
        let mut pending = f.body.iter().collect::<Vec<_>>();
        while let Some(e) = pending.pop() {
            if let ExprKind::TaskSpawn { function, .. } = e.kind
                && effects[function.0].blocks
            {
                return Err(crate::Error::at(
                    source,
                    e.span,
                    "spawned closures cannot block their JS agent; use a suspending host operation",
                )
                .with_code("E_TASK_BLOCKING"));
            }
            pending.extend(e.children());
        }
    }
    Ok(())
}

pub(crate) fn enabled(module: &Module) -> bool {
    module
        .types
        .iter()
        .any(|t| matches!(t.kind, TypeKind::Value(ValueType::Task(_))))
        || module
            .functions
            .iter()
            .any(|f| f.body.as_ref().is_some_and(has_scope))
}
fn has_scope(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::TaskScope { .. }) || e.children().iter().any(|e| has_scope(e))
}

pub(crate) fn types(module: &Module) -> String {
    if !enabled(module) {
        return String::new();
    }
    let mut out = String::from(
        "(type $__child_slot (struct (field i32) (field i32) (field (mut (ref null eq))) (field (mut (ref null eq))) (field (mut (ref null $__child_slot)))))\n",
    );
    for (i, t) in module.types.iter().enumerate() {
        if let TypeKind::Value(ValueType::Task(payload)) = &t.kind {
            out.push_str(&format!(
                "(type $__child_result_{i} (struct {}))\n",
                if *payload == Type::Unit {
                    String::new()
                } else {
                    format!("(field {})", crate::emit::ty(module, payload))
                }
            ));
        }
    }
    out
}
pub(crate) fn imports(module: &Module) -> String {
    if !enabled(module) {
        return String::new();
    }
    [
        ("scope-new", "$__scope_new", "(param i32) (result i32)"),
        ("scope-close", "$__scope_close", "(param i32)"),
        ("scope-abort", "$__scope_abort", "(param i32)"),
        ("child-new", "$__child_new", "(param i32) (result i32)"),
        ("child-start", "$__child_start", "(param i32)"),
        ("child-join", "$__child_join", "(param i32)"),
        ("child-cancel", "$__child_cancel", "(param i32)"),
        (
            "child-timeout",
            "$__child_timeout",
            "(param i32 i32) (result i32)",
        ),
        ("scope-enter", "$__scope_enter", "(param i32) (result i32)"),
        ("child-adopt", "$__child_adopt", "(param i32 i32)"),
        ("resource-adopt", "$__resource_adopt", "(param i32 i32)"),
    ]
    .iter()
    .map(|(name, f, ty)| format!("(import \"__component\" \"{name}\" (func {f} {ty}))\n"))
    .collect()
}
pub(crate) fn helpers(module: &Module) -> String {
    if !enabled(module) {
        return String::new();
    }
    let mut out = String::from(
        r#"
      (global $__current_scope (mut i32) (i32.const 0))
      (func $__scope_restore (param $scope i32)
        (global.set $__current_scope (local.get $scope)) (drop (call $__scope_enter (local.get $scope))))
      (global $__children (mut (ref null $__child_slot)) (ref.null $__child_slot))
      (func $__children_clear (export "__children_clear") (global.set $__children (ref.null $__child_slot)))
      (func (export "__children_live") (result i32) (local $n i32) (local $s (ref null $__child_slot))
        (local.set $s (global.get $__children)) (loop $next (if (ref.is_null (local.get $s)) (then (return (local.get $n))))
          (local.set $n (i32.add (local.get $n) (i32.const 1))) (local.set $s (struct.get $__child_slot 4 (ref.as_non_null (local.get $s)))) (br $next)) (unreachable))
      (func (export "__child_discard") (param $id i32) (drop (call $__child_take (local.get $id))))
      (func $__child_find (param $id i32) (result (ref $__child_slot)) (local $s (ref null $__child_slot))
        (local.set $s (global.get $__children))
        (loop $next
          (if (ref.is_null (local.get $s)) (then (unreachable)))
          (if (i32.eq (struct.get $__child_slot 0 (ref.as_non_null (local.get $s))) (local.get $id))
            (then (return (ref.as_non_null (local.get $s)))))
          (local.set $s (struct.get $__child_slot 4 (ref.as_non_null (local.get $s)))) (br $next)) (unreachable))
      (func $__child_take (param $id i32) (result (ref null eq)) (local $s (ref $__child_slot)) (local $p (ref null $__child_slot)) (local $r (ref null eq))
        (local.set $s (call $__child_find (local.get $id)))
        (local.set $r (struct.get $__child_slot 3 (local.get $s)))
        (if (ref.eq (global.get $__children) (local.get $s))
          (then (global.set $__children (struct.get $__child_slot 4 (local.get $s))))
          (else (local.set $p (global.get $__children))
            (loop $next (if (ref.eq (struct.get $__child_slot 4 (ref.as_non_null (local.get $p))) (local.get $s))
              (then (struct.set $__child_slot 4 (ref.as_non_null (local.get $p)) (struct.get $__child_slot 4 (local.get $s))))
              (else (local.set $p (struct.get $__child_slot 4 (ref.as_non_null (local.get $p)))) (br $next))))))
        (struct.set $__child_slot 2 (local.get $s) (ref.null eq))
        (struct.set $__child_slot 3 (local.get $s) (ref.null eq))
        (struct.set $__child_slot 4 (local.get $s) (ref.null $__child_slot)) (local.get $r))
    "#,
    );
    out.push_str(&adoption(module));
    let mut dispatch = String::new();
    // Keep the checked task type at each spawn site. Distinct nominal payloads
    // can erase to the same scalar without merging their GC handle types.
    let mut instances = std::collections::BTreeSet::new();
    for f in &module.functions {
        let mut pending = f.body.iter().collect::<Vec<_>>();
        while let Some(e) = pending.pop() {
            if let ExprKind::TaskSpawn { function, .. } = e.kind
                && let Type::Value(task) = e.ty
            {
                instances.insert((function.0, task.0));
            }
            pending.extend(e.children());
        }
    }
    for (case, (index, task)) in instances.into_iter().enumerate() {
        let f = &module.functions[index];
        let FunctionKind::Closure { environment } = f.kind else {
            unreachable!()
        };
        let env = &module.types[environment.0].name;
        let handle = &module.types[task].name;
        out.push_str(&format!("(func $__spawn_{index}_{task} (param $scope i32) (param $env (ref {env})) (result (ref {handle})) (local $id i32)
          (call $__checkpoint) (local.set $id (call $__child_new (local.get $scope)))
          (global.set $__children (struct.new $__child_slot (local.get $id) (i32.const {case}) (local.get $env) (ref.null eq) (global.get $__children)))
          (call $__child_start (local.get $id)) (struct.new $__variant_{task}_0 (i32.const 0) (local.get $id)))\n"));
        let adopt = if crate::ownership::Kinds::new(module).is_owned(&Type::Ref {
            ty: environment,
            nullable: false,
        }) {
            format!(
                "(call $__adopt_{} (ref.cast (ref {env}) (struct.get $__child_slot 2 (local.get $s))) (local.get $scope))",
                environment.0
            )
        } else {
            String::new()
        };
        let call = format!(
            "(call {} (local.get $scope) (ref.cast (ref {env}) (struct.get $__child_slot 2 (local.get $s))))",
            f.name
        );
        let boxed = if f.result == Type::Unit {
            format!("(block (result (ref eq)) {call} (struct.new $__child_result_{task}))")
        } else {
            format!("(struct.new $__child_result_{task} {call})")
        };
        dispatch.push_str(&format!("(if (i32.eq (struct.get $__child_slot 1 (local.get $s)) (i32.const {case})) (then {adopt} (call $__checkpoint)
          (struct.set $__child_slot 3 (local.get $s) {boxed}) (struct.set $__child_slot 2 (local.get $s) (ref.null eq)) (return)))\n"));
    }
    out.push_str(&format!("(func (export \"__child_run\") (param $id i32) (param $scope i32) (local $s (ref $__child_slot))
       (call $__scope_restore (local.get $scope)) (local.set $s (call $__child_find (local.get $id))) {dispatch} (unreachable))\n"));
    for (i, t) in module.types.iter().enumerate() {
        if let TypeKind::Value(ValueType::Task(payload)) = &t.kind {
            let result = if *payload == Type::Unit {
                String::new()
            } else {
                format!("(result {})", crate::emit::ty(module, payload))
            };
            let take = format!(
                "(ref.cast (ref $__child_result_{i}) (call $__child_take (local.get $id)))"
            );
            let lift = if *payload == Type::Unit {
                format!("(drop {take})")
            } else {
                format!("(struct.get $__child_result_{i} 0 {take})")
            };
            out.push_str(&format!("(func $__cancel_{i} (param $task (ref {})) (local $id i32)
              (local.set $id (struct.get $__variant_{i}_0 1 (ref.cast (ref $__variant_{i}_0) (local.get $task))))
              (call $__child_cancel (local.get $id)) (call $__checkpoint) (drop (call $__child_take (local.get $id))))\n",t.name));
            if let Some(option) = module.types.iter().position(
                |t| matches!(&t.kind, TypeKind::Value(ValueType::Option(p)) if p == payload),
            ) {
                let lifted = if *payload == Type::Unit {
                    format!("(drop {take})")
                } else {
                    lift.clone()
                };
                out.push_str(&format!("(func $__timed_{i} (param $ms i32) (param $task (ref {})) (result (ref {})) (local $id i32)
                  (local.set $id (struct.get $__variant_{i}_0 1 (ref.cast (ref $__variant_{i}_0) (local.get $task))))
                  (if (result (ref {})) (call $__child_timeout (local.get $id) (local.get $ms))
                    (then (call $__checkpoint) (drop (call $__child_take (local.get $id))) (struct.new $__variant_{option}_0 (i32.const 0)))
                    (else (call $__checkpoint) {})))\n",t.name,module.types[option].name,module.types[option].name,
                    if *payload==Type::Unit {format!("(block (result (ref {})) {lifted} (struct.new $__variant_{option}_1 (i32.const 1)))",module.types[option].name)}else{format!("(struct.new $__variant_{option}_1 (i32.const 1) {lifted})")}));
            }
            out.push_str(&format!("(func $__join_{i} (param $task (ref {})) {result} (local $id i32)
          (local.set $id (struct.get $__variant_{i}_0 1 (ref.cast (ref $__variant_{i}_0) (local.get $task))))
          (call $__child_join (local.get $id)) (call $__checkpoint) {lift})\n",t.name));
        }
    }
    out
}

/// Restore the continuation's owner after both successful and exceptional JSPI
/// resumes. The host's current scope is transient state, never a stack identity.
pub(crate) fn wrap_imports(module: &Module, mut imports: String) -> String {
    if !enabled(module) {
        return imports;
    }
    let source = format!("(module {imports})");
    let root = crate::sexpr::parse(&source).expect("generated imports");
    for node in root.list().unwrap().iter().skip(1) {
        let Some(f) = node
            .list()
            .and_then(|p| p.last())
            .filter(|n| n.head() == Some("func"))
        else {
            continue;
        };
        let parts = f.list().unwrap();
        let name = parts[1].atom().unwrap();
        if !(name.starts_with("$__host")
            || name.starts_with("$__read")
            || [
                "$__scope_close",
                "$__child_join",
                "$__child_cancel",
                "$__child_timeout",
            ]
            .contains(&name))
        {
            continue;
        }
        let mut count = 0;
        let mut sig = String::new();
        let mut result = String::new();
        for p in &parts[2..] {
            sig.push_str(&source[p.span.start..p.span.end]);
            if p.head() == Some("param") {
                count += p.list().unwrap().len() - 1;
            }
            if p.head() == Some("result") {
                result.push_str(&source[p.span.start..p.span.end]);
            }
        }
        imports = imports.replace(&format!("(func {name} "), &format!("(func {name}_raw "));
        let args = (0..count)
            .map(|i| format!(" (local.get {i})"))
            .collect::<String>();
        imports.push_str(&format!(
            "(func {name} {sig} (local $__resume_scope i32)
          (local.set $__resume_scope (global.get $__current_scope))
          (block $__resumed {result} (block $__unwind (result exnref)
            (br $__resumed (try_table {result} (catch_all_ref $__unwind) (call {name}_raw{args}))))
            (call $__scope_restore (local.get $__resume_scope)) (throw_ref))
          (call $__scope_restore (local.get $__resume_scope)))\n"
        ));
    }
    imports
}

pub(crate) fn adoption(module: &Module) -> String {
    let kinds = crate::ownership::Kinds::new(module);
    let call = |ty: &Type, v: String| -> String {
        if !kinds.is_owned(ty) {
            return String::new();
        }
        let id = match ty {
            Type::Value(id) | Type::Resource(id) | Type::Ref { ty: id, .. } => id,
            _ => return String::new(),
        };
        format!("(call $__adopt_{} {v} (local.get $scope))", id.0)
    };
    let mut out = String::new();
    for (i, t) in module.types.iter().enumerate() {
        let ty = if matches!(t.kind, TypeKind::Resource { .. }) {
            Type::Resource(TypeId(i))
        } else {
            module.instance_type(TypeId(i))
        };
        if !kinds.is_owned(&ty) {
            continue;
        }
        let mut body = String::new();
        match &t.kind {
            TypeKind::Resource { .. } => {
                body = format!(
                    "(call $__resource_adopt (struct.get {} 0 (local.get $v)) (local.get $scope))",
                    t.name
                )
            }
            TypeKind::Closure { captures, .. } => {
                for (n, f) in captures.iter().enumerate() {
                    body.push_str(&call(
                        &f.ty,
                        format!("(struct.get {} {n} (local.get $v))", t.name),
                    ));
                }
            }
            TypeKind::Value(ValueType::Task(_)) => {
                body = format!(
                    "(call $__child_adopt (struct.get $__variant_{i}_0 1 (ref.cast (ref $__variant_{i}_0) (local.get $v))) (local.get $scope))"
                )
            }
            TypeKind::Value(v) => {
                if matches!(v, ValueType::Future(_) | ValueType::Stream(_))
                    && crate::abi_limits::facts(module, &ty).is_some()
                {
                    let tag = if matches!(v, ValueType::Future(_)) {
                        1
                    } else {
                        2
                    };
                    body.push_str(&format!("(if (i32.eq (struct.get {} 0 (local.get $v)) (i32.const {tag})) (then (struct.set $__task_entry 5 (struct.get $__remote_{i} 2 (ref.cast (ref $__remote_{i}) (local.get $v))) (local.get $scope)) (return)))",t.name));
                }
                for (case, variant) in v.variants(TypeId(i)).iter().enumerate() {
                    let mut fields = String::new();
                    let mut n = 1;
                    for p in &variant.payloads {
                        if *p != Type::Unit {
                            fields.push_str(&call(p,format!("(struct.get $__variant_{i}_{case} {n} (ref.cast (ref $__variant_{i}_{case}) (local.get $v)))")));
                            n += 1;
                        }
                    }
                    if !fields.is_empty() {
                        body.push_str(&format!("(if (i32.eq (struct.get {} 0 (local.get $v)) (i32.const {case})) (then {fields}))",t.name));
                    }
                }
            }
            _ => {}
        }
        out.push_str(&format!(
            "(func $__adopt_{i} (param $v {}) (param $scope i32) {body})\n",
            crate::emit::ty(module, &ty)
        ));
    }
    out
}
