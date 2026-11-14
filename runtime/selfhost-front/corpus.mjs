// Bootstrap frontend v10 correctness corpus; historical scalar captures are immutable.
export const programs = [
  [
    "reads-bound",
    '(module (type $M (struct (field (mut i32)))) (func $read (param $m (ref $M)) (result i32) (@effect reads) (struct.get $M 0 $m)) (func (export "f") (result i32) (@effect reads allocates) (call $read (struct.new $M 42))))',
    42,
  ],
  [
    "combined-bound",
    '(module (type $M (struct (field (mut i32)))) (func (export "f") (result i32) (@effect reads writes allocates) (@let (($m (struct.new $M 0))) (block (result i32) (struct.set $M 0 $m 42) (struct.get $M 0 $m)))))',
    42,
  ],

  [
    "type-value-namespaces",
    '(module (type $T (struct)) (type $Box (@type.params $T) (struct (field $T $T))) (func $id (@type.params $T) (param $T $T) (result $T) $T) (func (export "f") (result i32) (struct.get $Box<i32> $T (struct.new $Box<i32> (call $id<i32> 42)))))',
    42,
  ],
  [
    "shared-unit",
    '(module (@use "walz:core" (type $unit)) (func $empty (@type.params $T) (@where (shared $T)) (result $T) (nop)) (func (export "f") (result i32) (block (result i32) (call $empty<$unit>) 42)))',
    42,
  ],
  [
    "explicit-rec-group",
    '(module (rec (type $A (struct (field (ref null $B)))) (type $B (struct (field (ref null $A))))) (func (export "f") (result i32) (@let (($a (struct.new_default $A))) (+ (ref.is_null (struct.get $A 0 $a)) 41))))',
    42,
  ],
  [
    "generic-rec-group",
    '(module (rec (type $A (@type.params $T) (struct (field $value $T) (field (ref null $B<$T>)))) (type $B (@type.params $T) (struct (field (ref null $A<$T>))))) (func (export "f") (result i32) (struct.get $A<i32> $value (struct.new $A<i32> 42 (ref.null $B<i32>)))))',
    42,
  ],
  [
    "numeric-local-type-index",
    '(module (@use "walz:core" (type $list)) (type (struct (field i32))) (func $answer (result i32) (struct.get 0 0 (struct.new 0 42))) (func (export "f") (result i32) (@let (($xs (@list.new i32 0))) (call 0))))',
    42,
  ],
  [
    "short-circuit",
    '(module (func (export "f") (result i32) (+ (&& 0 (unreachable)) (|| 2 (unreachable)))))',
    1,
  ],
  [
    "typed-tuple-pattern",
    '(module (@use "walz:core" (type $tuple)) (func (export "f") (result i32) (@let ($a $b) (type $tuple<i32,i32>) (@tuple.new 20 22) (+ $a $b))))',
    42,
  ],
  [
    "tuple-for-each",
    '(module (@use "walz:core" (type $tuple) (type $list)) (func (export "f") (result i32) (local $sum i32) (@let $xs (@list.new $tuple<i32,i32> (@tuple.new 20 22)) (block (result i32) (@for-each ($a $b) $xs (local.set $sum (+ $sum (+ $a $b)))) $sum))))',
    42,
  ],
  [
    "byte-builder",
    '(module (@use "walz:core" (type $bytes) (func $bytes-builder.new) (func $bytes-builder.byte) (func $bytes-builder.finish)) (func (export "f") (result i32) (@let $b (call $bytes-builder.new 2) (block (result i32) (call $bytes-builder.byte $b 42) (@bytes.get (call $bytes-builder.finish $b) 0)))))',
    42,
  ],

  [
    "let-many",
    '(module (func (export "f") (result i32) (@let (($a (i32.const 20)) ($b (+ $a 2))) (+ $a $b))))',
    42,
  ],
  [
    "let-shadow",
    '(module (func (export "f") (result i32) (@let (($x (i32.const 20)) ($x (+ $x 2))) $x)))',
    22,
  ],
  [
    "generic-explicit",
    '(module (func $id (@type.params $T) (@where (shared $T) (nonunit $T)) (param $x $T) (result $T) (@effect pure) $x) (func (export "f") (result i32) (call (@inst $id i32) 42)))',
    42,
  ],
  [
    "generic-inferred",
    '(module (func $id (@type.params $T) (@where (shared $T) (nonunit $T)) (param $x $T) (result $T) $x) (func (export "f") (result i32) (call $id (i32.const 42))))',
    42,
  ],
  [
    "generic-expected",
    '(module (func $id (@type.params $T) (param $x $T) (result $T) $x) (func (export "f") (result i64) (call $id 42)))',
    42n,
  ],
  [
    "generic-struct",
    '(module (type $Box (@type.params $T) (@where (shared $T) (nonunit $T)) (struct (field $value $T))) (func (export "f") (result i32) (struct.get $Box<i32> $value (struct.new $Box<i32> 42))))',
    42,
  ],
  [
    "generic-recursion",
    '(module (type $Node (@type.params $T) (@where (shared $T) (nonunit $T)) (struct (field $value $T) (field $next (ref null $Node<$T>)))) (func (export "f") (result i32) (struct.get $Node<i32> $value (struct.new $Node<i32> 42 (ref.null $Node<i32>)))))',
    42,
  ],
  [
    "loop",
    '(module (func (export "f") (result i32) (@loop $sum (bind $i i32 6) (bind $acc i32 0) (result i32) (if (result i32) (i32.eqz $i) (then $acc) (else (@recur $sum (- $i 1) (+ $acc 7)))))))',
    42,
  ],
  [
    "simultaneous-recur",
    '(module (func (export "f") (result i32) (@loop $swap (bind $n i32 1) (bind $a i32 1) (bind $b i32 2) (result i32) (if (result i32) $n (then (@recur $swap 0 $b $a)) (else (+ (* $a 10) $b))))))',
    21,
  ],
  [
    "loop-reference",
    '(module (type $S (struct (field i32))) (func (export "f") (result i32) (@loop $again (bind $x (ref $S) (struct.new $S 42)) (bind $n i32 1) (result i32) (if (result i32) $n (then (@recur $again $x 0)) (else (struct.get $S 0 $x))))))',
    42,
  ],
  [
    "for",
    '(module (func (export "f") (result i32) (local $sum i32) (block (result i32) (@for $sum-loop (bind $i i32 0) (while (< $i 7)) (step (+ $i 1)) (local.set $sum (+ $sum (* $i 2)))) $sum)))',
    42,
  ],
  [
    "list",
    '(module (@use "walz:core" (type $list)) (func (export "f") (result i32) (@effect pure) (@list.get i32 (@list.new i32 10 20 42) 2)))',
    42,
  ],
  [
    "for-each",
    '(module (@use "walz:core" (type $list)) (func (export "f") (result i32) (local $sum i32) (@let $xs (@list.new i32 10 20 12) (block (result i32) (@for-each $x $xs (local.set $sum (+ $sum $x))) $sum))))',
    42,
  ],
  [
    "empty-for-each",
    '(module (@use "walz:core" (type $list)) (func (export "f") (result i32) (@let $xs (@list.new i32) (block (result i32) (@for-each $x $xs (unreachable)) 42))))',
    42,
  ],
  [
    "list-snapshot",
    '(module (@use "walz:core" (type $list)) (type $A (array (mut i32))) (func (export "f") (result i32) (@let (($a (array.new $A 42 2)) ($xs (@list.from-array i32 $A $a))) (block (result i32) (array.set $A $a 0 7) (@list.get i32 $xs 0)))))',
    42,
  ],
  [
    "result",
    '(module (@use "walz:core" (type $result)) (func $value (result $result<i32,i64>) (@effect pure) (@result.ok 42)) (func (export "f") (result i32) (@match (call $value) (@case ok ($n) $n) (@case err ($e) (i32.wrap_i64 $e)))))',
    42,
  ],
  [
    "lazy-result",
    '(module (@use "walz:core" (type $result)) (func (export "f") (result i32) (@let $r (type $result<i32,i32>) (@result.ok 42) (@match $r (@case ok ($n) $n) (@case err ($e) (unreachable))))))',
    42,
  ],
  [
    "option",
    '(module (@use "walz:core" (type $option)) (func (export "f") (result i32) (@let $r (type $option<i32>) (@option.some 42) (@match $r (@case none () 0) (@case some ($n) $n)))))',
    42,
  ],
  [
    "option-default",
    '(module (@use "walz:core" (type $option)) (func (export "f") (result i32) (@let $r (type $option<i32>) (@option.none) (@match $r (@case some ($n) $n) (@default 42)))))',
    42,
  ],
  [
    "tuple",
    '(module (@use "walz:core" (type $tuple)) (func $pair (result $tuple<i32,i32>) (@tuple.new 20 22)) (func (export "f") (result i32) (@let ($a $b) (call $pair) (+ $a $b))))',
    42,
  ],
  [
    "bytes",
    '(module (@use "walz:core" (type $bytes)) (func (export "f") (result i32) (@bytes.get (@bytes.new 1 2 42) 2)))',
    42,
  ],
  [
    "byte-range-trap",
    '(module (@use "walz:core" (type $bytes)) (func (export "f") (result i32) (@bytes.len (@bytes.new 256))))',
    "trap",
  ],
  [
    "bytes-copy",
    '(module (@use "walz:core" (type $bytes)) (type $A (array (mut i8))) (func (export "f") (result i32) (@let $a (array.new_default $A 2) (block (result i32) (@bytes.copy-to-array $A $a 0 (@bytes.new 20 42) 0 2) (array.get_u $A $a 1)))))',
    42,
  ],
  [
    "bytes-snapshot",
    '(module (@use "walz:core" (type $bytes)) (type $A (array (mut i8))) (func (export "f") (result i32) (@let (($a (array.new $A 42 2)) ($b (@bytes.from-array $A $a))) (block (result i32) (array.set $A $a 0 7) (@bytes.get $b 0)))))',
    42,
  ],
  [
    "string",
    '(module (@use "walz:core" (type $string) (type $bytes)) (func (export "f") (result i32) (@let $s (type $string) "é" (@bytes.len (@string.bytes $s)))))',
    2,
  ],
  [
    "invalid-utf8",
    '(module (@use "walz:core" (type $bytes) (type $string) (type $option)) (func (export "f") (result i32) (@match (@string.from-bytes (@bytes.new 0xc0 0x80)) (@case none () 42) (@case some ($s) 0))))',
    42,
  ],
  [
    "valid-utf8",
    '(module (@use "walz:core" (type $bytes) (type $string) (type $option)) (func (export "f") (result i32) (@match (@string.from-bytes (@bytes.new 0xf0 0x9f 0xa6 0x80)) (@case none () 0) (@case some ($s) 42))))',
    42,
  ],
  [
    "list-builder",
    '(module (@use "walz:core" (type $list-builder) (type $list) (func $list-builder.new) (func $list-builder.push) (func $list-builder.finish)) (func (export "f") (result i32) (@let $b (call $list-builder.new<i32>) (block (result i32) (call $list-builder.push<i32> $b 20) (call $list-builder.push<i32> $b 22) (@let $xs (call $list-builder.finish<i32> $b) (+ (@list.get i32 $xs 0) (@list.get i32 $xs 1)))))))',
    42,
  ],
];
export const invalid = [
  [
    "specialization-growth",
    "(module (type $Box (@type.params $T) (struct (field $T))) (func $grow (@type.params $T) (result i32) (call $grow<(ref $Box<$T>)>)) (func (result i32) (call $grow<i32>)))",
    150,
  ],
  [
    "bytes-copy-storage",
    '(module (@use "walz:core" (type $bytes)) (type $A (array (mut i32))) (func (@bytes.copy-to-array $A (array.new_default $A 1) 0 (@bytes.new 42) 0 1)))',
    110,
  ],
  [
    "malformed-inst",
    "(module (func $f (@type.params $T)) (func (call (@inst))))",
    206,
  ],
  ["malformed-loop", "(module (func (@loop)))", 205],
  [
    "extra-logical-operand",
    '(module (@use "walz:core" (type $bytes)) (func (result i32) (&& 1 1 (unreachable))))',
    106,
  ],
  [
    "opaque-erasure",
    '(module (@use "walz:core" (type $bytes)) (func (param $x (ref null any))))',
    118,
  ],

  [
    "extra-operand",
    '(module (@use "walz:core" (type $bytes)) (func (result i32) (@bytes.len (@bytes.new) (unreachable))))',
    106,
  ],
  [
    "unknown-unused-type-argument",
    "(module (func $f (@type.params $T) (result i32) 42) (func (result i32) (call $f<$Missing>)))",
    118,
  ],
  [
    "duplicate-type-parameter",
    "(module (func $f (@type.params $T $T) (result i32) 42))",
    206,
  ],
  [
    "open-generic-export",
    '(module (func $f (export "f") (@type.params $T) (result i32) 42))',
    206,
  ],
  [
    "native-index-cannot-name-import",
    '(module (@use "walz:core" (type $list)) (func (result i32) (@list.len i32 (ref.null 0))))',
    201,
  ],

  [
    "loop-write",
    "(module (func (result i32) (@loop $l (bind $x i32 42) (result i32) (block (result i32) (local.set $x 7) $x))))",
    113,
  ],
  [
    "hidden-local",
    "(module (func (result i32) (@loop $l (bind $x i32 42) (result i32) (local.get 0))))",
    109,
  ],
  [
    "non-tail-recur",
    "(module (func (result i32) (@loop $l (bind $x i32 0) (result i32) (+ 1 (@recur $l 1)))))",
    205,
  ],
  [
    "recur-arity",
    "(module (func (result i32) (@loop $l (bind $x i32 0) (result i32) (@recur $l 1 2))))",
    205,
  ],
  [
    "generic-arity",
    "(module (func $id (@type.params $T) (param $x $T) (result $T) $x) (func (result i32) (call (@inst $id) 42)))",
    206,
  ],
  [
    "nonunit-constraint",
    '(module (@use "walz:core" (type $unit)) (type $Box (@type.params $T) (@where (shared $T) (nonunit $T)) (struct (field $T))) (func (result (ref $Box<$unit>)) (unreachable)))',
    207,
  ],
  [
    "unsupported-constraint",
    "(module (func $f (@type.params $T) (@where (Unknown $T)) (param $x $T) (result $T) $x) (func (result i32) (call $f<i32> 42)))",
    207,
  ],
  [
    "incomplete-match",
    '(module (@use "walz:core" (type $option)) (func (result i32) (@let $v (type $option<i32>) (@option.none) (@match $v (@case some ($n) $n)))))',
    208,
  ],
  [
    "payload-arity",
    '(module (@use "walz:core" (type $option)) (func (result i32) (@let $v (type $option<i32>) (@option.none) (@match $v (@case none ($n) 0) (@case some ($n) $n)))))',
    208,
  ],
  [
    "invalid-string",
    '(module (@use "walz:core" (type $string)) (func (result $string) "\\ff"))',
    209,
  ],
  [
    "effect-union",
    "(module (type $M (struct (field (mut i32)))) (func (param $m (ref $M)) (@effect reads) (drop (struct.get $M 0 $m)) (drop (struct.new_default $M))))",
    123,
  ],
  [
    "generic-effect",
    "(module (type $M (struct (field (mut i32)))) (func $f (@type.params $T) (param $x $T) (result (ref $M)) (@effect pure) (struct.new_default $M)) (func (result (ref $M)) (call $f<i32> 42)))",
    123,
  ],
];
export const projects = [
  [
    "relative",
    [
      [
        "src/main.walz",
        '(module (@use "./lib/../math.walz" (func $answer)) (func (export "f") (result i32) (call $answer)))',
      ],
      [
        "src/math.walz",
        "(module (@export (func $answer)) (func $answer (result i32) 42) (func $unused (result f64) (f64.const 1)))",
      ],
    ],
    42,
  ],
  [
    "opaque",
    [
      [
        "main.walz",
        '(module (@use "./token.walz" (type $Token) (func $make) (func $read)) (func (export "f") (result i32) (call $read (call $make))))',
      ],
      [
        "token.walz",
        "(module (@export (opaque $Token) (func $make) (func $read)) (type $Token (struct (field $value i32))) (func $make (result (ref $Token)) (struct.new $Token 42)) (func $read (param $t (ref $Token)) (result i32) (struct.get $Token $value $t)))",
      ],
    ],
    42,
  ],
];
export const badProjects = [
  [
    "missing",
    [["main.walz", '(module (@use "./missing.walz" (func $f)))']],
    200,
  ],
  ["namespace", [["main.walz", '(module (@use "library" (func $f)))']], 200],
  [
    "cycle",
    [
      [
        "main.walz",
        '(module (@use "./lib.walz" (func $b)) (@export (func $a)) (func $a))',
      ],
      [
        "lib.walz",
        '(module (@use "./main.walz" (func $a)) (@export (func $b)) (func $b))',
      ],
    ],
    203,
  ],
  [
    "private",
    [
      ["main.walz", '(module (@use "./lib.walz" (func $secret)))'],
      ["lib.walz", "(module (func $secret))"],
    ],
    201,
  ],
  [
    "opaque-read",
    [
      [
        "main.walz",
        '(module (@use "./lib.walz" (type $Token)) (func (param $x (ref $Token)) (result i32) (struct.get $Token 0 $x)))',
      ],
      [
        "lib.walz",
        "(module (@export (opaque $Token)) (type $Token (struct (field i32))))",
      ],
    ],
    202,
  ],
  [
    "opaque-generic-cast",
    [
      [
        "main.walz",
        '(module (@use "./lib.walz" (type $Token)) (type $Shadow (struct (field i32))) (func $cast (@type.params $T) (param $x (ref $Shadow)) (result (ref $T)) (ref.cast (ref $T) $x)) (func (param $x (ref $Shadow)) (result (ref $Token)) (call $cast<$Token> $x)))',
      ],
      [
        "lib.walz",
        "(module (@export (opaque $Token)) (type $Token (struct (field i32))))",
      ],
    ],
    202,
  ],
  [
    "source-range",
    [
      [
        "main.walz",
        '(module (@use "./lib.walz" (func $bad)) (func (result i64) (call $bad)))',
      ],
      [
        "lib.walz",
        "(module (@export (func $bad)) (func $bad (result i64) (i32.const 42)))",
      ],
    ],
    110,
  ],
];

import * as macros from "./macros.mjs";
programs.push(...macros.programs);
invalid.push(...macros.invalid);
projects.push(...macros.projects);
badProjects.push(...macros.badProjects);

import {connectionErrors, libraryProjects, libraryErrors} from "./connections.mjs";
invalid.push(...connectionErrors.map(([name, source, code]) => [name, source, code]));
projects.push(...libraryProjects);
badProjects.push(...libraryErrors);

import * as parity from "./parity.mjs";
programs.push(...parity.programs);
invalid.push(...parity.invalid, ...parity.boundedInvalid);
projects.push(...parity.projects, ...parity.nativeIndexProjects);
badProjects.push(...parity.badProjects);

import * as budgets from "./budgets.mjs";
programs.push(...budgets.programs);
invalid.push(...budgets.invalid, ...budgets.boundedInvalid);

import * as bytes from "./bytes.mjs";
programs.push(...bytes.programs, ...bytes.nativeBranchPrograms);
invalid.push(...bytes.invalid);
badProjects.push(...bytes.badProjects);

import * as scaling from "./scaling.mjs";
programs.push(...scaling.programs, ...scaling.nativePrograms);
invalid.push(...scaling.invalid, ...scaling.boundedInvalid);

import * as serialization from "./serialization.mjs";
programs.push(...serialization.programs);
invalid.push(...serialization.invalid);

import * as tuning from "./tuning.mjs";
programs.push(...tuning.programs);

import * as abi from "./abi.mjs";
programs.push(...abi.programs);
invalid.push(...abi.invalid,...abi.unsupported);
projects.push(...abi.projects);
badProjects.push(...abi.badProjects);

import * as control from "./control.mjs";
programs.push(...control.programs,...control.nativePrograms);
invalid.push(...control.invalid,...control.unsupported);
