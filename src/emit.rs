use crate::ir::*;

pub(crate) fn ty(module: &Module, ty: &Type) -> String {
    match ty {
        Type::Newtype(id) => match module.types[id.0].kind {
            TypeKind::Newtype(n) => n.wat().into(),
            _ => unreachable!(),
        },
        Type::Num(_) | Type::Ref { .. } => module.type_name(ty),
        Type::Value(id) | Type::Resource(id) | Type::Borrow(id) => {
            format!("(ref {})", module.types[id.0].name)
        }
        Type::Unit | Type::Never => unreachable!("unit and never are not storage types"),
    }
}

fn result(module: &Module, value: &Type) -> String {
    if matches!(value, Type::Unit | Type::Never) {
        String::new()
    } else {
        format!(" (result {})", ty(module, value))
    }
}

fn quoted(text: &str) -> String {
    let mut output = String::from("\"");
    for byte in text.bytes() {
        match byte {
            b'"' | b'\\' => {
                output.push('\\');
                output.push(char::from(byte));
            }
            0x20..=0x7e => output.push(char::from(byte)),
            _ => output.push_str(&format!("\\{byte:02x}")),
        }
    }
    output.push('"');
    output
}

fn storage(module: &Module, field: &Field) -> String {
    let value = ty(module, &field.ty);
    if field.mutable {
        format!("(mut {value})")
    } else {
        value
    }
}

fn variant_type(ty: TypeId, variant: usize) -> String {
    format!("$__variant_{}_{variant}", ty.0)
}

pub(crate) fn emit(module: &Module) -> String {
    emit_inner(module, None)
}

pub(crate) fn emit_component(module: &Module, adapter: &crate::component::Adapter<'_>) -> String {
    emit_inner(module, Some(adapter))
}

fn emit_inner(module: &Module, adapter: Option<&crate::component::Adapter<'_>>) -> String {
    let mut output = String::from("(module\n");
    // One explicit recursion group supports forward and recursive GC types.
    if module
        .types
        .iter()
        .any(|t| !matches!(t.kind, TypeKind::Newtype(_)))
        || adapter.is_some()
    {
        output.push_str("  (rec\n");
        if module
            .types
            .iter()
            .any(|t| matches!(t.kind, TypeKind::Value(ValueType::Bytes | ValueType::Text)))
        {
            output.push_str("(type $__byte_storage (array (mut i8)))\n");
        }
        if adapter.is_some() {
            output.push_str("(type $__task_entry (struct (field i32) (field i32) (field (mut i32)) (field (mut (ref null $__task_entry))) (field (mut (ref null $__task_entry)))))\n");
        }
        for (index, declaration) in module.types.iter().enumerate() {
            let definition = match &declaration.kind {
                TypeKind::Newtype(_) => continue,
                TypeKind::Struct(fields)
                | TypeKind::Closure {
                    captures: fields, ..
                } => {
                    let fields = fields
                        .iter()
                        .map(|f| {
                            format!(
                                " (field {}{})",
                                f.name.as_ref().map(|n| format!("{n} ")).unwrap_or_default(),
                                storage(module, f)
                            )
                        })
                        .collect::<String>();
                    format!("(struct{fields})")
                }
                TypeKind::Resource { .. } => "(struct (field i32) (field (mut i32)))".into(),
                TypeKind::Array(field) => format!("(array {})", storage(module, field)),
                TypeKind::Value(ValueType::List(payload)) => {
                    let element = if *payload == Type::Unit {
                        "i8".into()
                    } else {
                        ty(module, payload)
                    };
                    output.push_str(&format!(
                        "(type $__list_storage_{index} (array (mut {element})))\n"
                    ));
                    format!("(struct (field (ref $__list_storage_{index})))")
                }
                TypeKind::Value(ValueType::Bytes | ValueType::Text) => {
                    "(struct (field (ref $__byte_storage)))".into()
                }
                TypeKind::Variant(_) | TypeKind::Value(_) => {
                    let variants = match &declaration.kind {
                        TypeKind::Variant(variants) => {
                            std::borrow::Cow::Borrowed(variants.as_slice())
                        }
                        TypeKind::Value(value) => {
                            std::borrow::Cow::Owned(value.variants(TypeId(index)))
                        }
                        _ => unreachable!(),
                    };
                    output.push_str(&format!(
                        "    (type {} (sub (struct (field i32))))\n",
                        declaration.name
                    ));
                    for (case, variant) in variants.iter().enumerate() {
                        let fields = variant
                            .payloads
                            .iter()
                            .map(|payload| format!(" (field {})", ty(module, payload)))
                            .collect::<String>();
                        output.push_str(&format!(
                            "    (type {} (sub final {} (struct (field i32){fields})))\n",
                            variant_type(TypeId(index), case),
                            declaration.name
                        ));
                    }
                    if let Some(adapter) = adapter {
                        output.push_str(&adapter.remote_types(TypeId(index)));
                    }
                    continue;
                }
            };
            output.push_str(&format!("    (type {} {definition})\n", declaration.name));
        }
        output.push_str("  )\n");
    }
    // Imports must precede function definitions in the binary index space.
    if let Some(adapter) = adapter {
        output.push_str(&adapter.imports());
    }
    for function in &module.functions {
        if let FunctionKind::Import {
            module: namespace,
            name,
        } = &function.kind
        {
            if adapter.is_some() {
                continue;
            }
            output.push_str(&format!(
                "  (import {} {} (func {}{}{}))\n",
                quoted(namespace),
                quoted(name),
                function.name,
                params(module, function),
                result(module, &function.result)
            ));
        }
    }
    for tag in &module.tags {
        let payloads = tag
            .payloads
            .iter()
            .map(|p| format!(" {}", ty(module, p)))
            .collect::<String>();
        output.push_str(&format!("  (tag {} (param{payloads}))\n", tag.name));
    }
    output.push_str(&crate::buffers::helpers(module));
    output.push_str(&crate::lists::helpers(module));
    for (i, t) in module.types.iter().enumerate() {
        if let TypeKind::Resource { close, .. } = &t.kind {
            let name = &t.name;
            let close = &module.functions[close.0].name;
            output.push_str(&format!("(func $__resource_handle_{i} (param $r (ref {name})) (result i32) (if (struct.get {name} 1 (local.get $r)) (then (unreachable))) (struct.get {name} 0 (local.get $r)))\n(func $__resource_close_{i} (param $r (ref {name})) (if (struct.get {name} 1 (local.get $r)) (then (unreachable))) (struct.set {name} 1 (local.get $r) (i32.const 1)) (call {close} (struct.get {name} 0 (local.get $r))))\n"));
        }
    }
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        let mut emitter = Emitter {
            module,
            function,
            temporaries: vec![],
            next_control: 0,
            adapter,
            using: vec![],
            labels: vec![],
        };
        let body = emitter.expression(body);
        output.push_str(&format!("  (func {}", function.name));
        for export in &function.exports {
            output.push_str(&format!(" (export {})", quoted(export)));
        }
        if let FunctionKind::Closure { environment } = function.kind {
            output.push_str(&format!(
                " (param $__env (ref {}))",
                module.types[environment.0].name
            ));
        }
        output.push_str(&params(module, function));
        output.push_str(&result(module, &function.result));
        output.push('\n');
        for (index, binding) in function.locals.iter().enumerate() {
            if matches!(
                binding.kind,
                BindingKind::Let | BindingKind::Loop(_) | BindingKind::Match | BindingKind::Catch
            ) {
                output.push_str(&format!(
                    "    (local $__v{index} {})\n",
                    ty(module, &binding.ty)
                ));
            }
        }
        for (index, value) in emitter.temporaries.iter().enumerate() {
            output.push_str(&format!(
                "    (local $__temp{index} {})\n",
                ty(module, value)
            ));
        }
        output.push_str(&format!("    {body}\n  )\n"));
    }
    if let Some(adapter) = adapter {
        output.push_str(&adapter.helpers());
    }
    output.push_str(")\n");
    output
}

fn params(module: &Module, function: &Function) -> String {
    function
        .params
        .iter()
        .map(|id| {
            format!(
                " (param $__v{} {})",
                id.0,
                ty(module, &function.locals[id.0].ty)
            )
        })
        .collect()
}

struct Emitter<'a> {
    module: &'a Module,
    function: &'a Function,
    temporaries: Vec<Type>,
    next_control: usize,
    adapter: Option<&'a crate::component::Adapter<'a>>,
    using: Vec<(LocalId, TypeId, usize)>,
    labels: Vec<(LoopId, usize, Type)>,
}

impl Emitter<'_> {
    fn cleanup(&self, depth: usize) -> String {
        self.using[depth..].iter().rev().map(|(local,ty,flag)|
            format!(" (if (local.get $__temp{flag}) (then (local.set $__temp{flag} (i32.const 0)) (call $__resource_close_{} (local.get $__v{}))))",ty.0,local.0)
        ).collect()
    }
    fn save(&mut self, value: &Expr, ty: Type) -> (String, usize) {
        let temp = self.temporaries.len();
        self.temporaries.push(ty);
        (
            format!(" (local.set $__temp{temp} {})", self.expression(value)),
            temp,
        )
    }
    fn arguments(&mut self, expressions: &[Expr]) -> String {
        expressions
            .iter()
            .map(|e| format!(" {}", self.expression(e)))
            .collect()
    }

    fn expression(&mut self, expression: &Expr) -> String {
        use ExprKind::*;
        let code = match &expression.kind {
            Label { label, body } => {
                self.labels
                    .push((*label, self.using.len(), expression.ty.clone()));
                let b = self.expression(body);
                self.labels.pop();
                format!(
                    "(block $__label{}{} {b})",
                    label.0,
                    result(self.module, &expression.ty)
                )
            }
            Return(value) => {
                let (evaluation, operand) = if let Some(v) = value {
                    if self.function.result == Type::Unit {
                        (self.expression(v), String::new())
                    } else {
                        let (s, t) = self.save(v, self.function.result.clone());
                        (s, format!(" (local.get $__temp{t})"))
                    }
                } else {
                    (String::new(), String::new())
                };
                format!("(block {evaluation}{} (return{operand}))", self.cleanup(0))
            }
            Branch {
                label,
                values,
                condition,
                bindings,
                is_loop,
            } => {
                let (_, depth, target) = self
                    .labels
                    .iter()
                    .rev()
                    .find(|(id, _, _)| id == label)
                    .cloned()
                    .expect("checked label");
                let mut evaluations = String::new();
                let mut temps = vec![];
                for (i, v) in values.iter().enumerate() {
                    let ty = if *is_loop {
                        self.function.locals[bindings[i].0].ty.clone()
                    } else {
                        target.clone()
                    };
                    let (s, t) = self.save(v, ty);
                    evaluations.push_str(&s);
                    temps.push(t);
                }
                let mut updates = String::new();
                for (id, temp) in bindings.iter().zip(&temps) {
                    updates.push_str(&format!(
                        " (local.set $__v{} (local.get $__temp{temp}))",
                        id.0
                    ));
                }
                let args = if *is_loop {
                    String::new()
                } else {
                    temps
                        .iter()
                        .map(|t| format!(" (local.get $__temp{t})"))
                        .collect()
                };
                let name = format!("$__{}{}", if *is_loop { "loop" } else { "label" }, label.0);
                let close = self.cleanup(depth);
                if let Some(c) = condition {
                    let (s, t) = self.save(c, Type::Num(NumType::I32));
                    evaluations.push_str(&s);
                    if *is_loop {
                        let value = temps
                            .first()
                            .map(|t| format!(" (local.get $__temp{t})"))
                            .unwrap_or_default();
                        format!(
                            "(block{} {evaluations} (if (local.get $__temp{t}) (then {close}{updates} (br {name}))){value})",
                            result(self.module, &expression.ty)
                        )
                    } else {
                        format!(
                            "(block{} {evaluations} (br_if {name}{args} (block (result i32) (if (local.get $__temp{t}) (then {close})) (local.get $__temp{t}))))",
                            result(self.module, &expression.ty)
                        )
                    }
                } else {
                    format!("(block {evaluations}{close}{updates} (br {name}{args}))")
                }
            }
            Newtype { value, .. } => self.expression(value),
            List { op, list, args } => {
                let values = args.iter().map(|a| self.expression(a)).collect::<Vec<_>>();
                crate::lists::expression(self.module, *op, *list, &values)
            }
            Data(bytes) => {
                let Type::Value(id) = expression.ty else {
                    unreachable!()
                };
                format!(
                    "(struct.new {} (array.new_fixed $__byte_storage {}{}))",
                    self.module.types[id.0].name,
                    bytes.len(),
                    bytes
                        .iter()
                        .map(|b| format!(" (i32.const {b})"))
                        .collect::<String>()
                )
            }
            Buffer { op, buffer, args } => match op {
                BufferOp::New => {
                    let Type::Value(id) = expression.ty else {
                        unreachable!()
                    };
                    format!(
                        "(struct.new {} (array.new_fixed $__byte_storage {}{}))",
                        self.module.types[id.0].name,
                        args.len(),
                        self.arguments(args)
                    )
                }
                BufferOp::Len => {
                    let id = *buffer;
                    format!(
                        "(array.len {})",
                        crate::buffers::storage(
                            &self.module.types[id.0].name,
                            &self.expression(&args[0])
                        )
                    )
                }
                BufferOp::Get => {
                    let id = *buffer;
                    format!(
                        "(array.get_u $__byte_storage {} {})",
                        crate::buffers::storage(
                            &self.module.types[id.0].name,
                            &self.expression(&args[0])
                        ),
                        self.expression(&args[1])
                    )
                }
                BufferOp::FromArray(id) => format!(
                    "(call $__bytes_from_array_{}{})",
                    id.0,
                    self.arguments(args)
                ),
                BufferOp::Slice => format!("(call $__bytes_slice{})", self.arguments(args)),
                BufferOp::Encode => format!("(call $__text_bytes{})", self.arguments(args)),
                BufferOp::Decode => format!("(call $__text_decode{})", self.arguments(args)),
            },
            Const { ty, literal } => format!("({}.const {literal})", ty.wat()),
            Numeric { op, args } => format!("({}{})", op.signature().name, self.arguments(args)),
            Simd { op, args } => op.wat(&self.arguments(args)),
            Local(id) | Move(id) => match self.function.locals[id.0].kind {
                BindingKind::Capture(index) => {
                    let FunctionKind::Closure { environment } = self.function.kind else {
                        unreachable!()
                    };
                    format!(
                        "(struct.get {} {index} (local.get $__env))",
                        self.module.types[environment.0].name
                    )
                }
                _ => format!("(local.get $__v{})", id.0),
            },
            Let { local, value, body } => format!(
                "(block{} (local.set $__v{} {}) {})",
                result(self.module, &expression.ty),
                local.0,
                self.expression(value),
                self.expression(body)
            ),
            Block(expressions) => format!(
                "(block{}{})",
                result(self.module, &expression.ty),
                self.arguments(expressions)
            ),
            If {
                condition,
                then_body,
                else_body,
            } => format!(
                "(if{} {} (then {}) (else {}))",
                result(self.module, &expression.ty),
                self.expression(condition),
                self.expression(then_body),
                self.expression(else_body)
            ),
            Throw { tag, values } => format!(
                "(throw {}{})",
                self.module.tags[tag.0].name,
                self.arguments(values)
            ),
            Try { body, handlers } => self.try_expression(body, handlers, &expression.ty),
            VariantNew {
                ty,
                variant,
                values,
            } => format!(
                "(struct.new {} (i32.const {variant}){})",
                variant_type(*ty, *variant),
                self.arguments(values)
            ),
            Match { ty, subject, arms } => {
                self.match_expression(*ty, subject, arms, &expression.ty)
            }
            Loop {
                label,
                bindings,
                body,
            } => {
                let initializers = bindings
                    .iter()
                    .map(|(id, value)| {
                        format!(" (local.set $__v{} {})", id.0, self.expression(value))
                    })
                    .collect::<String>();
                self.labels
                    .push((*label, self.using.len(), expression.ty.clone()));
                let b = self.expression(body);
                self.labels.pop();
                format!(
                    "(block{}{} (loop $__loop{}{} {}))",
                    result(self.module, &expression.ty),
                    initializers,
                    label.0,
                    result(self.module, &expression.ty),
                    b
                )
            }
            Recur { label, assignments } => {
                let mut evaluations = String::new();
                let mut updates = String::new();
                // Evaluate all new values before changing any loop binding.
                for (id, value) in assignments {
                    let temp = self.temporaries.len();
                    self.temporaries.push(self.function.locals[id.0].ty.clone());
                    evaluations.push_str(&format!(
                        " (local.set $__temp{temp} {})",
                        self.expression(value)
                    ));
                    updates.push_str(&format!(
                        " (local.set $__v{} (local.get $__temp{temp}))",
                        id.0
                    ));
                }
                let depth = self
                    .labels
                    .iter()
                    .rev()
                    .find(|(id, _, _)| id == label)
                    .expect("checked loop")
                    .1;
                format!(
                    "(block{evaluations}{}{updates} (br $__loop{}))",
                    self.cleanup(depth),
                    label.0
                )
            }
            Call { function, args } => format!(
                "(call {}{})",
                self.module.functions[function.0].name,
                self.arguments(args)
            ),
            StructNew { ty, values } => format!(
                "(struct.new {}{})",
                self.module.types[ty.0].name,
                self.arguments(values)
            ),
            StructGet { ty, field, object } => format!(
                "(struct.get {} {field} {})",
                self.module.types[ty.0].name,
                self.expression(object)
            ),
            StructSet {
                ty,
                field,
                object,
                value,
            } => format!(
                "(struct.set {} {field} {} {})",
                self.module.types[ty.0].name,
                self.expression(object),
                self.expression(value)
            ),
            ArrayNew {
                ty,
                initial,
                length,
            } => format!(
                "(array.new {} {} {})",
                self.module.types[ty.0].name,
                self.expression(initial),
                self.expression(length)
            ),
            ArrayFixed { ty, values } => format!(
                "(array.new_fixed {} {}{})",
                self.module.types[ty.0].name,
                values.len(),
                self.arguments(values)
            ),
            ArrayGet { ty, object, index } => format!(
                "(array.get {} {} {})",
                self.module.types[ty.0].name,
                self.expression(object),
                self.expression(index)
            ),
            ArraySet {
                ty,
                object,
                index,
                value,
            } => format!(
                "(array.set {} {} {} {})",
                self.module.types[ty.0].name,
                self.expression(object),
                self.expression(index),
                self.expression(value)
            ),
            ArrayLen(object) => format!("(array.len {})", self.expression(object)),
            Null(ty) => format!("(ref.null {})", self.module.types[ty.0].name),
            IsNull(value) => format!("(ref.is_null {})", self.expression(value)),
            AsNonNull(value) => format!("(ref.as_non_null {})", self.expression(value)),
            RefEq(a, b) => format!("(ref.eq {} {})", self.expression(a), self.expression(b)),
            Drop(value) => format!("(drop {})", self.expression(value)),
            ResourceNew { ty, handle } => format!(
                "(struct.new {} {} (i32.const 0))",
                self.module.types[ty.0].name,
                self.expression(handle)
            ),
            ResourceHandle(value) => {
                let Type::Borrow(id) = value.ty else {
                    unreachable!()
                };
                format!(
                    "(call $__resource_handle_{} {})",
                    id.0,
                    self.expression(value)
                )
            }
            ResourceClose(value) => {
                let Type::Resource(id) = value.ty else {
                    unreachable!()
                };
                format!(
                    "(call $__resource_close_{} {})",
                    id.0,
                    self.expression(value)
                )
            }
            Using { local, value, body } => {
                let Type::Borrow(id) = self.function.locals[local.0].ty else {
                    unreachable!()
                };
                let v = self.expression(value);
                let flag = self.temporaries.len();
                self.temporaries.push(Type::Num(NumType::I32));
                self.using.push((*local, id, flag));
                let b = self.expression(body);
                let close = self.cleanup(self.using.len() - 1);
                self.using.pop();
                let r = result(self.module, &expression.ty);
                format!(
                    "(block{r} (local.set $__v{} {v}) (local.set $__temp{flag} (i32.const 1)) (block $__using_done{}{} (block $__using_caught{} (result exnref) (br $__using_done{} (try_table{r} (catch_all_ref $__using_caught{}) {b}))) {close} (throw_ref)) {close})",
                    local.0, local.0, r, local.0, local.0, local.0
                )
            }
            Close(value) => format!("(drop {})", self.expression(value)),
            Delivery { op, endpoint, args } => {
                self.delivery_expression(*op, *endpoint, args, &expression.ty)
            }
            Nop => "(nop)".into(),
            Unreachable => "(unreachable)".into(),
        };
        // A Wasm block restores its declared stack shape at its end. Preserve
        // bottom typing in the surrounding expression after a bottom-valued block.
        if expression.ty == Type::Never && !matches!(expression.kind, Unreachable) {
            format!("{code} (unreachable)")
        } else {
            code
        }
    }

    fn delivery_expression(
        &mut self,
        op: DeliveryOp,
        endpoint: TypeId,
        args: &[Expr],
        value: &Type,
    ) -> String {
        if let Some(adapter) = self.adapter
            && adapter.endpoints.contains(&endpoint)
            && matches!(
                op,
                DeliveryOp::FutureAwait
                    | DeliveryOp::FutureClose
                    | DeliveryOp::StreamNext
                    | DeliveryOp::StreamClose
            )
        {
            let operand = self.expression(&args[0]);
            if let Some(code) = adapter.delivery(op, endpoint, &operand) {
                return code;
            }
        }
        match op {
            DeliveryOp::FutureReady | DeliveryOp::StreamEmpty | DeliveryOp::StreamCons => {
                let case = usize::from(op == DeliveryOp::StreamCons);
                return format!(
                    "(struct.new {} (i32.const {case}){})",
                    variant_type(endpoint, case),
                    self.arguments(args)
                );
            }
            DeliveryOp::FutureClose | DeliveryOp::StreamClose => {
                return format!("(drop {})", self.expression(&args[0]));
            }
            _ => {}
        }
        let temp = self.temporaries.len();
        self.temporaries.push(Type::Value(endpoint));
        let operand = self.expression(&args[0]);
        let base = &self.module.types[endpoint.0].name;
        let tag = format!("(struct.get {base} 0 (local.get $__temp{temp}))");
        let result = result(self.module, value);
        let empty = variant_type(endpoint, 0);
        let empty_cast = format!("(ref.cast (ref {empty}) (local.get $__temp{temp}))");
        if op == DeliveryOp::FutureAwait {
            let read = if *value == Type::Unit {
                format!("(drop {empty_cast})")
            } else {
                format!("(struct.get {empty} 1 {empty_cast})")
            };
            return format!(
                "(block{result} (local.set $__temp{temp} {operand}) (if{result} (i32.eq {tag} (i32.const 0)) (then {read}) (else (unreachable))))"
            );
        }
        let TypeKind::Value(ValueType::Stream(payload)) = &self.module.types[endpoint.0].kind
        else {
            unreachable!()
        };
        let option = self.module.value_id(&ValueType::Option(payload.clone()));
        let Type::Value(pair) = value else {
            unreachable!()
        };
        let cons = variant_type(endpoint, 1);
        let cons_cast = format!("(ref.cast (ref {cons}) (local.get $__temp{temp}))");
        let tail_index = if *payload == Type::Unit { 1 } else { 2 };
        let tail = format!("(struct.get {cons} {tail_index} {cons_cast})");
        let item = if *payload == Type::Unit {
            String::new()
        } else {
            format!(" (struct.get {cons} 1 {cons_cast})")
        };
        let none = format!("(struct.new {} (i32.const 0))", variant_type(option, 0));
        let some = format!(
            "(struct.new {} (i32.const 1){item})",
            variant_type(option, 1)
        );
        let pair = variant_type(*pair, 0);
        format!("(block{result} (local.set $__temp{temp} {operand}) (if{result} (i32.eq {tag} (i32.const 0))
          (then (struct.new {pair} (i32.const 0) {empty_cast} {none}))
          (else (if{result} (i32.eq {tag} (i32.const 1))
            (then (struct.new {pair} (i32.const 0) {tail} {some})) (else (unreachable))))))")
    }

    fn try_expression(&mut self, body: &Expr, handlers: &[CatchArm], value: &Type) -> String {
        let id = self.next_control;
        self.next_control += 1;
        let done = format!("$__try_done{id}");
        let mut code = format!("(block {done}{}", result(self.module, value));
        for (index, handler) in handlers.iter().enumerate().rev() {
            let payloads = handler
                .tag
                .map(|tag| {
                    self.module.tags[tag.0]
                        .payloads
                        .iter()
                        .map(|p| format!(" {}", ty(self.module, p)))
                        .collect::<String>()
                })
                .unwrap_or_default();
            code.push_str(&format!(" (block $__catch{id}_{index}"));
            if !payloads.is_empty() {
                code.push_str(&format!(" (result{payloads})"));
            }
        }
        code.push_str(&format!(
            " (br {done} (try_table{}",
            result(self.module, value)
        ));
        if self.adapter.is_some() {
            // The private control exception must bypass source catch_all.
            // Branch outside all user handlers, then rethrow below.
            code.push_str(&format!(" (catch $__task_cancel $__cancel_bypass{id})"));
        }
        for (index, handler) in handlers.iter().enumerate() {
            let catch = handler
                .tag
                .map(|tag| format!("catch {}", self.module.tags[tag.0].name))
                .unwrap_or_else(|| "catch_all".into());
            code.push_str(&format!(" ({catch} $__catch{id}_{index})"));
        }
        code.push_str(&format!(" {}))", self.expression(body)));
        for handler in handlers {
            code.push(')');
            // The tag's payload arrives on the Wasm operand stack in declaration
            // order. Pop it into immutable source bindings in reverse order.
            for binding in handler.bindings.iter().rev() {
                code.push_str(&format!(" (local.set $__v{})", binding.0));
            }
            code.push_str(&format!(" (br {done} {})", self.expression(&handler.body)));
        }
        code.push(')');
        if self.adapter.is_some() {
            format!(
                "(block $__cancel_done{id}{} (block $__cancel_bypass{id} (br $__cancel_done{id} {code})) (throw $__task_cancel))",
                result(self.module, value)
            )
        } else {
            code
        }
    }

    fn match_expression(
        &mut self,
        ty: TypeId,
        subject: &Expr,
        arms: &[MatchArm],
        value: &Type,
    ) -> String {
        let temp = self.temporaries.len();
        self.temporaries.push(self.module.instance_type(ty));
        let subject = self.expression(subject);
        let result = result(self.module, value);
        let mut dispatch = String::new();
        for arm in arms {
            let concrete = variant_type(ty, arm.variant);
            let mut bindings = arm.bindings.iter().enumerate().map(|(index, local)| {
                format!(" (local.set $__v{} (struct.get {concrete} {} (ref.cast (ref {concrete}) (local.get $__temp{temp}))))",
                    local.0, index + 1)
            }).collect::<String>();
            // Payload reads already cast their receiver. Nullary cases also
            // need to reject a foreign base value carrying a valid case tag.
            if arm.bindings.is_empty() {
                bindings.push_str(&format!(
                    " (drop (ref.cast (ref {concrete}) (local.get $__temp{temp})))"
                ));
            }
            dispatch.push_str(&format!(
                "(if{result} (i32.eq (struct.get {} 0 (local.get $__temp{temp})) (i32.const {})) (then{bindings} {}) (else ",
                self.module.types[ty.0].name, arm.variant, self.expression(&arm.body)
            ));
        }
        // Source construction is closed, but a foreign Wasm caller can forge
        // a base value/tag. Never interpret it as an arbitrary final case.
        dispatch.push_str("(unreachable)");
        for _ in arms {
            dispatch.push_str("))");
        }
        format!("(block{result} (local.set $__temp{temp} {subject}) {dispatch})")
    }
}
