//! Numeric linear-memory operations borrow their types from the Core validator.
//! Effect classification is a closed list; unknown operations fail closed.
use crate::{Error, ir::*};

pub fn supported(name: &str) -> bool {
    matches!(
        name,
        "memory.copy"
            | "memory.fill"
            | "i32.load"
            | "i64.load"
            | "f32.load"
            | "f64.load"
            | "i32.load8_s"
            | "i32.load8_u"
            | "i32.load16_s"
            | "i32.load16_u"
            | "i64.load8_s"
            | "i64.load8_u"
            | "i64.load16_s"
            | "i64.load16_u"
            | "i64.load32_s"
            | "i64.load32_u"
            | "i32.store"
            | "i64.store"
            | "f32.store"
            | "f64.store"
            | "i32.store8"
            | "i32.store16"
            | "i64.store8"
            | "i64.store16"
            | "i64.store32"
            | "v128.load"
            | "v128.store"
            | "v128.load8x8_s"
            | "v128.load8x8_u"
            | "v128.load16x4_s"
            | "v128.load16x4_u"
            | "v128.load32x2_s"
            | "v128.load32x2_u"
            | "v128.load8_splat"
            | "v128.load16_splat"
            | "v128.load32_splat"
            | "v128.load64_splat"
            | "v128.load32_zero"
            | "v128.load64_zero"
            | "v128.load8_lane"
            | "v128.load16_lane"
            | "v128.load32_lane"
            | "v128.load64_lane"
            | "v128.store8_lane"
            | "v128.store16_lane"
            | "v128.store32_lane"
            | "v128.store64_lane"
    )
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Instruction {
    pub opcode: String,
    pub memories: Vec<MemoryId>,
    pub immediates: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readonly: Option<crate::readonly::Load>,
}
impl Instruction {
    pub fn reads(&self) -> bool {
        self.opcode == "memory.copy" || self.opcode.contains(".load")
    }
    pub fn writes(&self) -> bool {
        !self.opcode.contains(".load")
    }
    pub fn render(&self, memories: &[MemoryDecl], args: &[String]) -> String {
        format!(
            "({}{}{}{})",
            self.opcode,
            self.memories
                .iter()
                .map(|id| format!(" {}", memories[id.0].name))
                .collect::<String>(),
            self.immediates
                .iter()
                .map(|s| format!(" {s}"))
                .collect::<String>(),
            args.iter().map(|s| format!(" {s}")).collect::<String>()
        )
    }
    pub(crate) fn hint(&self, index: usize) -> NumType {
        if index == 0 || self.opcode.starts_with("memory.") {
            return NumType::I32;
        }
        self.opcode
            .split_once('.')
            .and_then(|(n, _)| NumType::parse(n))
            .unwrap_or(NumType::I32)
    }
    pub(crate) fn result(&self, memories: &[MemoryDecl], inputs: &[Type]) -> Result<Type, Error> {
        let failure =
            |e: String| Error::message(format!("Wasm validator: {e}")).with_code("E_WAT_TYPE");
        let mut module = String::from("(module ");
        for memory in memories {
            module.push_str(&format!(
                "(memory {} {}{}{})",
                memory.name,
                memory.initial,
                memory.maximum.map(|n| format!(" {n}")).unwrap_or_default(),
                if memory.shared { " shared" } else { "" }
            ));
        }
        module.push_str("(func ");
        let mut args = Vec::new();
        for (i, ty) in inputs.iter().enumerate() {
            args.push(match ty {
                Type::Num(n)=>{module.push_str(&format!("(param $p{i} {})",n.wat()));format!("(local.get $p{i})")},
                Type::Unit=>"(nop)".into(),Type::Never=>"(unreachable)".into(),
                _=>return Err(failure("opaque memory operands must be numeric; unwrap nominal values explicitly and keep owned/borrowed values inside checked expressions".into())),
            });
        }
        module.push_str(&self.render(memories, &args));
        module.push_str("))");
        let bytes = wat::parse_str(&module).map_err(|e| failure(e.to_string()))?;
        let mut validator =
            wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all());
        for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
            let payload = payload.map_err(|e| failure(e.to_string()))?;
            if let wasmparser::ValidPayload::Func(function, body) = validator
                .payload(&payload)
                .map_err(|e| failure(e.to_string()))?
            {
                let mut function = function.into_validator(Default::default());
                function
                    .read_locals(&mut body.get_binary_reader())
                    .map_err(|e| failure(e.to_string()))?;
                let mut reader = body
                    .get_operators_reader()
                    .map_err(|e| failure(e.to_string()))?;
                while !reader.eof() {
                    let offset = reader.original_position();
                    let op = reader.read().map_err(|e| failure(e.to_string()))?;
                    if reader.eof() && matches!(op, wasmparser::Operator::End) {
                        let output = match function.operand_stack_height() {
                            0 => Type::Unit,
                            1 => match function.get_operand_type(0).flatten() {
                                Some(wasmparser::ValType::I32) => Type::Num(NumType::I32),
                                Some(wasmparser::ValType::I64) => Type::Num(NumType::I64),
                                Some(wasmparser::ValType::F32) => Type::Num(NumType::F32),
                                Some(wasmparser::ValType::F64) => Type::Num(NumType::F64),
                                Some(wasmparser::ValType::V128) => Type::Num(NumType::V128),
                                _ => return Err(failure(
                                    "only a single numeric result is supported at this boundary"
                                        .into(),
                                )),
                            },
                            _ => {
                                return Err(failure(
                                    "multiple stack results are unsupported in a typed expression"
                                        .into(),
                                ));
                            }
                        };
                        return Ok(if inputs.contains(&Type::Never) {
                            Type::Never
                        } else {
                            output
                        });
                    }
                    function
                        .op(offset, &op)
                        .map_err(|e| failure(e.to_string()))?;
                }
            }
        }
        Err(failure("missing operator result".into()))
    }
}
