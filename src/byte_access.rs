//! Fixed-width accesses to GC byte storage. Helpers evaluate operands once at
//! their call boundary and check the complete range before reading or writing.
use crate::ir::*;
use std::collections::BTreeSet;

impl ByteAccess {
    pub(crate) fn parse_read(name: &str) -> Option<Self> {
        Self::parse(name.strip_prefix("bytes.read-")?)
    }

    pub(crate) fn parse_write(name: &str) -> Option<Self> {
        let access = Self::parse(name.strip_prefix("array.write-")?)?;
        (access.format != ByteFormat::U16).then_some(access)
    }

    fn parse(name: &str) -> Option<Self> {
        let (format, order) = name.rsplit_once('-')?;
        Some(Self {
            format: match format {
                "i16" => ByteFormat::I16,
                "u16" => ByteFormat::U16,
                "i32" => ByteFormat::I32,
                "i64" => ByteFormat::I64,
                "f32" => ByteFormat::F32,
                "f64" => ByteFormat::F64,
                _ => return None,
            },
            order: match order {
                "le" => ByteOrder::Little,
                "be" => ByteOrder::Big,
                _ => return None,
            },
        })
    }

    pub(crate) fn value_type(self) -> NumType {
        match self.format {
            ByteFormat::I16 | ByteFormat::U16 | ByteFormat::I32 => NumType::I32,
            ByteFormat::I64 => NumType::I64,
            ByteFormat::F32 => NumType::F32,
            ByteFormat::F64 => NumType::F64,
        }
    }

    pub(crate) fn width(self) -> u32 {
        match self.format {
            ByteFormat::I16 | ByteFormat::U16 => 2,
            ByteFormat::I32 | ByteFormat::F32 => 4,
            ByteFormat::I64 | ByteFormat::F64 => 8,
        }
    }

    fn bits_type(self) -> &'static str {
        if self.width() == 8 { "i64" } else { "i32" }
    }

    fn suffix(self) -> String {
        let format = match self.format {
            ByteFormat::I16 => "i16",
            ByteFormat::U16 => "u16",
            ByteFormat::I32 => "i32",
            ByteFormat::I64 => "i64",
            ByteFormat::F32 => "f32",
            ByteFormat::F64 => "f64",
        };
        let order = match self.order {
            ByteOrder::Little => "le",
            ByteOrder::Big => "be",
        };
        format!("{format}_{order}")
    }

    pub(crate) fn read_helper(self) -> String {
        format!("$__bytes_read_{}", self.suffix())
    }

    pub(crate) fn write_helper(self, id: TypeId) -> String {
        format!("$__array_write_{}_{}", self.suffix(), id.0)
    }

    fn shift(self, index: u32) -> u32 {
        8 * match self.order {
            ByteOrder::Little => index,
            ByteOrder::Big => self.width() - 1 - index,
        }
    }

    fn check(self) -> String {
        format!(
            r#"
          (if (i32.gt_u (local.get $p) (array.len (local.get $a))) (then (unreachable)))
          (if (i32.gt_u (i32.const {}) (i32.sub (array.len (local.get $a)) (local.get $p))) (then (unreachable)))"#,
            self.width()
        )
    }

    fn read(self, bytes: &str, proven: bool) -> String {
        let bits = self.bits_type();
        let mut value = String::new();
        for index in 0..self.width() {
            let mut byte = format!(
                "(array.get_u $__byte_storage (local.get $a) (i32.add (local.get $p) (i32.const {index})))"
            );
            if bits == "i64" {
                byte = format!("(i64.extend_i32_u {byte})");
            }
            if self.shift(index) != 0 {
                byte = format!("({bits}.shl {byte} ({bits}.const {}))", self.shift(index));
            }
            value = if index == 0 {
                byte
            } else {
                format!("({bits}.or {value} {byte})")
            };
        }
        value = match self.format {
            ByteFormat::I16 => format!("(i32.extend16_s {value})"),
            ByteFormat::F32 => format!("(f32.reinterpret_i32 {value})"),
            ByteFormat::F64 => format!("(f64.reinterpret_i64 {value})"),
            _ => value,
        };
        format!(
            r#"(func {} (param $b (ref {bytes})) (param $p i32) (result {})
          (local $a (ref $__byte_storage))
          (local.set $a (struct.get {bytes} 0 (local.get $b)))
          {} {value})"#,
            format_args!(
                "{}{}",
                self.read_helper(),
                if proven { "_proven" } else { "" }
            ),
            self.value_type().wat(),
            if proven { String::new() } else { self.check() }
        )
    }

    fn write(self, id: TypeId, storage: &str) -> String {
        let bits = self.bits_type();
        let value = match self.format {
            ByteFormat::F32 => "(i32.reinterpret_f32 (local.get $v))",
            ByteFormat::F64 => "(i64.reinterpret_f64 (local.get $v))",
            _ => "(local.get $v)",
        };
        let mut stores = String::new();
        for index in 0..self.width() {
            let mut byte = "(local.get $bits)".to_owned();
            if self.shift(index) != 0 {
                byte = format!("({bits}.shr_u {byte} ({bits}.const {}))", self.shift(index));
            }
            if bits == "i64" {
                byte = format!("(i32.wrap_i64 {byte})");
            }
            // array.set truncates to packed i8; no source byte range check is needed.
            stores.push_str(&format!("(array.set {storage} (local.get $a) (i32.add (local.get $p) (i32.const {index})) {byte})"));
        }
        format!(
            r#"(func {} (param $a (ref {storage})) (param $p i32) (param $v {})
          (local $bits {bits}) {} (local.set $bits {value}) {stores})"#,
            self.write_helper(id),
            self.value_type().wat(),
            self.check()
        )
    }
}

/// Emit only helpers present after linking; no linear scratch memory is needed.
pub(crate) fn helpers(module: &Module) -> String {
    let mut reads = BTreeSet::new();
    let mut writes = BTreeSet::new();
    for function in &module.functions {
        let mut pending = function.body.iter().collect::<Vec<_>>();
        while let Some(e) = pending.pop() {
            match e.kind {
                ExprKind::Buffer {
                    op: BufferOp::Read(access),
                    buffer,
                    ..
                } => {
                    reads.insert((access, buffer, false));
                }
                ExprKind::Buffer {
                    op: BufferOp::ReadProven(access),
                    buffer,
                    ..
                } => {
                    reads.insert((access, buffer, true));
                }
                ExprKind::Buffer {
                    op: BufferOp::WriteArray(id, access),
                    ..
                } => {
                    writes.insert((id, access));
                }
                _ => {}
            }
            pending.extend(e.children());
        }
    }
    let mut out = String::new();
    for (access, buffer, proven) in reads {
        out.push_str(&access.read(&module.types[buffer.0].name, proven));
    }
    for (id, access) in writes {
        out.push_str(&access.write(id, &module.types[id.0].name));
    }
    out
}
