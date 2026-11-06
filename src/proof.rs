//! Proof obligations over the actual typed function bodies. The initial law
//! fragment is pure, acyclic i32/i64 code. No bounded-input testing is used.

use crate::Error;
use crate::ir::*;
use std::collections::BTreeSet;

fn ty(ty: &Type) -> Result<&'static str, Error> {
    match ty {
        Type::Num(NumType::I32) => Ok("BitVec 32"),
        Type::Num(NumType::I64) => Ok("BitVec 64"),
        Type::Unit => Ok("Unit"),
        _ => Err(Error::message(
            "law proofs support only i32/i64 and unit; unsupported law type".into(),
        )),
    }
}

fn literal(text: &str) -> Result<String, Error> {
    let text = text.replace('_', "");
    let (negative, text) = if let Some(rest) = text.strip_prefix('-') {
        (true, rest)
    } else {
        (false, text.strip_prefix('+').unwrap_or(&text))
    };
    let value = if let Some(rest) = text.strip_prefix("0x") {
        u128::from_str_radix(rest, 16)
    } else {
        text.parse::<u128>()
    }
    .map_err(|_| Error::message("unsupported law integer literal".into()))?;
    Ok(format!("{}{value}", if negative { "-" } else { "" }))
}

fn truth(condition: String) -> String {
    format!("(if {condition} then (1 : BitVec 32) else 0)")
}

fn numeric(op: NumericOp, args: &[Expr]) -> Result<String, Error> {
    let signature = op.signature();
    let values = args.iter().map(expression).collect::<Result<Vec<_>, _>>()?;
    let a = &values[0];
    let operation = signature.name.split_once('.').unwrap().1;
    let unsupported = || Error::message(format!("unsupported law operation: {}", signature.name));
    match operation {
        "eqz" => return Ok(truth(format!("{a} = 0"))),
        "extend_i32_s" => return Ok(format!("(BitVec.signExtend 64 {a})")),
        "extend_i32_u" => return Ok(format!("(BitVec.setWidth 64 {a})")),
        _ => {}
    }
    let b = values.get(1).ok_or_else(unsupported)?;
    let code = match operation {
        "add" => format!("({a} + {b})"),
        "sub" => format!("({a} - {b})"),
        "mul" => format!("({a} * {b})"),
        "and" => format!("({a} &&& {b})"),
        "or" => format!("({a} ||| {b})"),
        "xor" => format!("({a} ^^^ {b})"),
        "eq" => truth(format!("{a} = {b}")),
        "ne" => truth(format!("{a} ≠ {b}")),
        "lt_u" => truth(format!("{a} < {b}")),
        "le_u" => truth(format!("{a} ≤ {b}")),
        "gt_u" => truth(format!("{b} < {a}")),
        "ge_u" => truth(format!("{b} ≤ {a}")),
        "lt_s" => truth(format!("BitVec.slt {a} {b}")),
        "le_s" => truth(format!("BitVec.sle {a} {b}")),
        "gt_s" => truth(format!("BitVec.slt {b} {a}")),
        "ge_s" => truth(format!("BitVec.sle {b} {a}")),
        "shl" | "shr_s" | "shr_u" => {
            let mask = if signature.input == NumType::I32 {
                31
            } else {
                63
            };
            let shift = format!("({b} &&& ({mask} : {}))", ty(&args[1].ty)?);
            match operation {
                "shl" => format!("({a} <<< {shift})"),
                "shr_u" => format!("({a} >>> {shift})"),
                _ => format!("(BitVec.sshiftRight' {a} {shift})"),
            }
        }
        _ => return Err(unsupported()),
    };
    Ok(code)
}

fn expression(expression: &Expr) -> Result<String, Error> {
    use ExprKind::*;
    ty(&expression.ty)?;
    let code = match &expression.kind {
        Const { literal: value, .. } => format!("({} : {})", literal(value)?, ty(&expression.ty)?),
        Local(id) => format!("v{}", id.0),
        Numeric { op, args } => numeric(*op, args)?,
        Let { local, value, body } => format!(
            "(let v{} : {} := {}; {})", local.0, ty(&value.ty)?,
            self::expression(value)?, self::expression(body)?
        ),
        Block(values) => {
            let mut result = "()".into();
            for (i, value) in values.iter().rev().enumerate() {
                let value = self::expression(value)?;
                result = if i == 0 { value } else { format!("(let _ := {value}; {result})") };
            }
            result
        }
        If { condition, then_body, else_body } => format!(
            "(if {} = 0 then {} else {})", self::expression(condition)?,
            self::expression(else_body)?, self::expression(then_body)?
        ),
        Call { function, args } => {
            let args = args.iter().map(|arg| Ok(format!(" {}", self::expression(arg)?)))
                .collect::<Result<String, Error>>()?;
            format!("(f{}{args})", function.0)
        }
        Drop(value) => format!("(let _ := {}; ())", self::expression(value)?),
        Nop => "()".into(),
        _ => return Err(Error::message(
            "unsupported law operation: laws require pure acyclic integer expressions; loops, heap, exceptions and floats need a separate proof model".into()
        )),
    };
    Ok(code)
}

fn dependencies(
    id: FunctionId,
    module: &Module,
    active: &mut BTreeSet<FunctionId>,
    done: &mut BTreeSet<FunctionId>,
    ordered: &mut Vec<FunctionId>,
) -> Result<(), Error> {
    if done.contains(&id) {
        return Ok(());
    }
    if !active.insert(id) {
        return Err(Error::message(
            "recursive law dependencies require an explicit inductive proof model".into(),
        ));
    }
    let body = module.functions[id.0]
        .body
        .as_ref()
        .ok_or_else(|| Error::message("law proof cannot use an import".into()))?;
    let mut pending = vec![body];
    while let Some(expression) = pending.pop() {
        if let ExprKind::Call { function, .. } = expression.kind {
            dependencies(function, module, active, done, ordered)?;
        }
        pending.extend(expression.children());
    }
    active.remove(&id);
    done.insert(id);
    ordered.push(id);
    Ok(())
}

/// Produce inspectable Lean source. Laws universally quantify numeric params;
/// bv_decide produces proof terms checked by Lean's kernel over fixed-width bits.
pub fn emit(module: &Module) -> Result<String, Error> {
    let lowered = crate::newtypes::lower(module);
    let module = &lowered;
    let mut output = String::new();
    if !module.laws.is_empty() {
        output.push_str("import Std.Tactic.BVDecide\n");
    }
    output.push_str(include_str!("../proof/Ownership.lean"));
    output.push('\n');
    output.push_str(
        include_str!("../proof/Delivery.lean")
            .strip_prefix("import Lean\n")
            .expect("delivery model imports Lean"),
    );
    output.push_str(
        include_str!("../proof/Tasks.lean")
            .strip_prefix("import Lean\n")
            .expect("task model imports Lean"),
    );
    if module.laws.is_empty() {
        return Ok(output);
    }
    output.push_str(
        "\nnamespace WltLaws\nset_option maxRecDepth 4096\nset_option maxHeartbeats 4000000\n",
    );
    let mut ordered = Vec::new();
    let mut done = BTreeSet::new();
    for law in &module.laws {
        dependencies(
            law.function,
            module,
            &mut BTreeSet::new(),
            &mut done,
            &mut ordered,
        )?;
    }
    for id in &ordered {
        let function = &module.functions[id.0];
        if !function.captures.is_empty() {
            return Err(Error::message("law proof cannot use captures".into()));
        }
        let params = function
            .params
            .iter()
            .map(|id| Ok(format!(" (v{} : {})", id.0, ty(&function.locals[id.0].ty)?)))
            .collect::<Result<String, Error>>()?;
        let body = expression(function.body.as_ref().unwrap())
            .map_err(|error| error.note(format!("law dependency `{}`", function.name)))?;
        output.push_str(&format!(
            "def f{}{params} : {} := {body}\n",
            id.0,
            ty(&function.result)?
        ));
    }
    for (index, law) in module.laws.iter().enumerate() {
        let mut reachable = Vec::new();
        dependencies(
            law.function,
            module,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
            &mut reachable,
        )?;
        let definitions = reachable
            .iter()
            .map(|id| format!("f{}", id.0))
            .collect::<Vec<_>>()
            .join(", ");
        let function = &module.functions[law.function.0];
        let params = function
            .params
            .iter()
            .map(|id| Ok(format!(" (v{} : {})", id.0, ty(&function.locals[id.0].ty)?)))
            .collect::<Result<String, Error>>()?;
        let arguments = function
            .params
            .iter()
            .map(|id| format!(" v{}", id.0))
            .collect::<String>();
        output.push_str(&format!("theorem law{index}{params} : f{}{arguments} ≠ 0 := by\n  simp only [{definitions}] <;> bv_decide\n", law.function.0));
    }
    output.push_str("end WltLaws\n");
    Ok(output)
}
