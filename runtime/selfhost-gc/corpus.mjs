// GC v1 correctness corpus; scalar benchmark captures remain unchanged.
export const programs = [
  [
    "struct",
    '(module (type $S (struct (field $x i32))) (func (export "f") (result i32) (struct.get $S $x (struct.new $S (i32.const 42)))))',
    42,
  ],
  [
    "packed",
    '(module (type $S (struct (field i8))) (func (export "f") (result i32) (struct.get_s $S 0 (struct.new $S (i32.const 255)))))',
    -1,
  ],
  [
    "array",
    '(module (type $A (array (mut i32))) (func (export "f") (result i32) (local $a (ref null $A)) (local.set $a (array.new $A (i32.const 0) (i32.const 3))) (array.set $A (local.get $a) (i32.const 1) (i32.const 42)) (array.get $A (local.get $a) (i32.const 1))))',
    42,
  ],
  [
    "recursive",
    '(module (type $N (struct (field $tail (ref null $N)) (field $v i64))) (func (export "f") (result i64) (struct.get $N $v (struct.new $N (ref.null $N) (i64.const 42)))))',
    42n,
  ],
  [
    "mutual",
    '(module (rec (type $A (struct (field (ref null $B)))) (type $B (array (ref null $A)))) (func (export "f") (result i32) (ref.is_null (struct.get $A 0 (struct.new_default $A)))))',
    1,
  ],
  [
    "cast",
    '(module (type $S (struct (field i32))) (func (export "f") (result i32) (struct.get $S 0 (ref.cast (ref $S) (struct.new $S (i32.const 42))))))',
    42,
  ],
  [
    "canonical-cast",
    '(module (type $S (struct (field i32))) (type $T (struct (field i32))) (func (export "f") (result i32) (struct.get $T 0 (ref.cast (ref $T) (struct.new $S (i32.const 42))))))',
    42,
  ],
  [
    "i31",
    '(module (func (export "f") (result i32) (i31.get_s (ref.i31 (i32.const 2147483647)))))',
    -1,
  ],
  [
    "nonnull-let",
    '(module (type $S (struct (field i32))) (func (export "f") (result i32) (@let $s (struct.new $S (i32.const 42)) (struct.get $S 0 $s))))',
    42,
  ],
  [
    "subtype",
    '(module (type $B (sub (struct (field i32)))) (type $D (sub final $B (struct (field i32) (field i64)))) (func $read (param $x (ref $B)) (result i32) (struct.get $B 0 (local.get $x))) (func (export "f") (result i32) (call $read (struct.new $D (i32.const 42) (i64.const 7)))))',
    42,
  ],
  [
    "reference-if",
    '(module (type $S (struct (field i32))) (func (export "f") (result i32) (struct.get $S 0 (if (result (ref $S)) (i32.const 1) (then (struct.new $S (i32.const 42))) (else (struct.new $S (i32.const 7)))))))',
    42,
  ],
  [
    "typed-select",
    '(module (type $S (struct (field i32))) (func (export "f") (result i32) (struct.get $S 0 (select (result (ref $S)) (struct.new $S (i32.const 42)) (struct.new $S (i32.const 7)) (i32.const 1)))))',
    42,
  ],
  [
    "nullable-test",
    '(module (type $S (struct)) (func (export "f") (result i32) (ref.test (ref null $S) (ref.null any))))',
    1,
  ],
  [
    "nonnull-test",
    '(module (type $S (struct)) (func (export "f") (result i32) (ref.test (ref $S) (ref.null any))))',
    0,
  ],
  [
    "null-trap",
    '(module (type $S (struct (field i32))) (func (export "f") (result i32) (struct.get $S 0 (ref.null $S))))',
    "trap",
  ],
  [
    "cast-trap",
    '(module (type $S (struct)) (type $A (array i32)) (func (export "f") (result (ref $S)) (ref.cast (ref $S) (array.new_default $A (i32.const 0)))))',
    "trap",
  ],
  [
    "nonnull-trap",
    '(module (func (export "f") (result (ref any)) (ref.as_non_null (ref.null any))))',
    "trap",
  ],
  [
    "reference-equality",
    '(module (func (export "f") (result i32) (ref.eq (ref.i31 (i32.const 7)) (ref.i31 (i32.const 7)))))',
    1,
  ],
  [
    "array-fixed",
    '(module (type $A (array i16)) (func (export "f") (result i32) (array.get_u $A (array.new_fixed $A 3 (i32.const 1) (i32.const 65535) (i32.const 42)) (i32.const 2))))',
    42,
  ],
  [
    "array-fill-copy",
    '(module (type $A (array (mut i32))) (func (export "f") (result i32) (local $a (ref null $A)) (local $b (ref null $A)) (local.set $a (array.new_default $A (i32.const 3))) (local.set $b (array.new_default $A (i32.const 3))) (array.fill $A (local.get $a) (i32.const 0) (i32.const 42) (i32.const 3)) (array.copy $A $A (local.get $b) (i32.const 1) (local.get $a) (i32.const 0) (i32.const 2)) (array.get $A (local.get $b) (i32.const 2))))',
    42,
  ],
  [
    "pure-immutable",
    '(module (type $S (struct (field i32))) (func (export "f") (result i32) (@effect pure) (struct.get $S 0 (struct.new $S (i32.const 42)))))',
    42,
  ],
  [
    "nullable-let",
    '(module (type $S (struct)) (func (export "f") (result i32) (@let $s (type (ref null $S)) (ref.null $S) (ref.is_null $s))))',
    1,
  ],
];
