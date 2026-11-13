//! Test/benchmark-only equality contract for closed scalar Core Wasm.
//! Never used to change or approve compiler output in production.
use std::error::Error;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction, Module,
    TypeSection, ValType,
};
use wasmparser::{BlockType, ExternalKind, Operator, Parser, Payload};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn scalar(t: wasmparser::ValType) -> Result<ValType> {
    match t {
        wasmparser::ValType::I32 => Ok(ValType::I32),
        wasmparser::ValType::I64 => Ok(ValType::I64),
        _ => Err("parity contract: only i32/i64 value types are supported".into()),
    }
}

fn block(t: BlockType) -> Result<wasm_encoder::BlockType> {
    Ok(match t {
        BlockType::Empty => wasm_encoder::BlockType::Empty,
        BlockType::Type(t) => wasm_encoder::BlockType::Result(scalar(t)?),
        _ => return Err("parity contract: block parameters/type indices are unsupported".into()),
    })
}

fn instruction(op: Operator<'_>) -> Result<Instruction<'static>> {
    macro_rules! convert {
        ($($name:ident),* $(,)?) => {
            match op {
                $(Operator::$name => Instruction::$name,)*
                Operator::Block { blockty } => Instruction::Block(block(blockty)?),
                Operator::Loop { blockty } => Instruction::Loop(block(blockty)?),
                Operator::If { blockty } => Instruction::If(block(blockty)?),
                Operator::Br { relative_depth } => Instruction::Br(relative_depth),
                Operator::BrIf { relative_depth } => Instruction::BrIf(relative_depth),
                Operator::Call { function_index } => Instruction::Call(function_index),
                Operator::LocalGet { local_index } => Instruction::LocalGet(local_index),
                Operator::LocalSet { local_index } => Instruction::LocalSet(local_index),
                Operator::LocalTee { local_index } => Instruction::LocalTee(local_index),
                Operator::I32Const { value } => Instruction::I32Const(value),
                Operator::I64Const { value } => Instruction::I64Const(value),
                _ => return Err("parity contract: unsupported instruction".into()),
            }
        }
    }
    Ok(convert!(
        Unreachable,
        Nop,
        Else,
        End,
        Return,
        Drop,
        Select,
        I32Eqz,
        I64Eqz,
        I32Eq,
        I32Ne,
        I32LtS,
        I32LtU,
        I32GtS,
        I32GtU,
        I32LeS,
        I32LeU,
        I32GeS,
        I32GeU,
        I64Eq,
        I64Ne,
        I64LtS,
        I64LtU,
        I64GtS,
        I64GtU,
        I64LeS,
        I64LeU,
        I64GeS,
        I64GeU,
        I32Add,
        I32Sub,
        I32Mul,
        I32DivS,
        I32DivU,
        I32RemS,
        I32RemU,
        I32And,
        I32Or,
        I32Xor,
        I32Shl,
        I32ShrS,
        I32ShrU,
        I64Add,
        I64Sub,
        I64Mul,
        I64DivS,
        I64DivU,
        I64RemS,
        I64RemU,
        I64And,
        I64Or,
        I64Xor,
        I64Shl,
        I64ShrS,
        I64ShrU,
        I32WrapI64,
        I64ExtendI32S,
        I64ExtendI32U
    ))
}

/// Fully validate and re-encode a closed scalar module into a unique layout.
/// Ignore only debug-name sections, type interning, local grouping and LEB spelling.
/// Function/export order, signatures, local order and every instruction survive.
/// Reject everything whose semantics this contract cannot establish.
pub fn canonical(bytes: &[u8]) -> Result<Vec<u8>> {
    wasmparser::Validator::new().validate_all(bytes)?;
    let mut signatures = Vec::new();
    let mut indices = Vec::new();
    let mut exports = ExportSection::new();
    let mut code = CodeSection::new();
    let mut bodies = 0usize;
    for payload in Parser::new(0).parse_all(bytes) {
        match payload? {
            Payload::Version {
                encoding: wasmparser::Encoding::Module,
                ..
            }
            | Payload::End(_) => (),
            Payload::TypeSection(reader) => {
                for ty in reader.into_iter_err_on_gc_types() {
                    let ty = ty?;
                    if ty.params().len() > 256 || ty.results().len() > 1 {
                        return Err("parity contract: unsupported signature".into());
                    }
                    signatures.push((
                        ty.params()
                            .iter()
                            .copied()
                            .map(scalar)
                            .collect::<Result<Vec<_>>>()?,
                        ty.results()
                            .iter()
                            .copied()
                            .map(scalar)
                            .collect::<Result<Vec<_>>>()?,
                    ));
                }
            }
            Payload::FunctionSection(reader) => {
                indices = reader
                    .into_iter()
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if indices.len() > 1024 {
                    return Err("parity contract: function limit".into());
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let e = export?;
                    if e.kind != ExternalKind::Func {
                        return Err("parity contract: non-function export".into());
                    }
                    exports.export(e.name, ExportKind::Func, e.index);
                }
            }
            Payload::CodeSectionStart { .. } => (),
            Payload::CodeSectionEntry(body) => {
                let mut locals = Vec::new();
                for local in body.get_locals_reader()? {
                    let (count, ty) = local?;
                    if count > 1024 - locals.len() as u32 {
                        return Err("parity contract: local limit".into());
                    }
                    locals.extend(std::iter::repeat_n((1, scalar(ty)?), count as usize));
                }
                let mut function = Function::new(locals);
                let mut reader = body.get_operators_reader()?;
                while !reader.eof() {
                    function.instruction(&instruction(reader.read()?)?);
                }
                code.function(&function);
                bodies += 1;
            }
            Payload::CustomSection(section) if section.name() == "name" => (),
            _ => return Err("parity contract: unsupported section".into()),
        }
    }
    if bodies != indices.len() {
        return Err("parity contract: body count mismatch".into());
    }
    let mut types = TypeSection::new();
    let mut functions = FunctionSection::new();
    for (i, &index) in indices.iter().enumerate() {
        let (params, results) = signatures
            .get(index as usize)
            .ok_or("parity contract: type index")?;
        types
            .ty()
            .function(params.iter().copied(), results.iter().copied());
        functions.function(i as u32);
    }
    let mut module = Module::new();
    module
        .section(&types)
        .section(&functions)
        .section(&exports)
        .section(&code);
    let result = module.finish();
    wasmparser::Validator::new().validate_all(&result)?;
    Ok(result)
}
