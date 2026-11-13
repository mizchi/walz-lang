// Declarative macro contracts for the Walz-written compiler, independent of the
// frozen scalar benchmark corpus.
export const programs = [
  [
    "macro-hygiene",
    `(module
  (@macro $twice (rule (?x:expr) (@let $saved ?x (+ $saved $saved))))
  (func $f (export "f") (result i32) (@let $saved (type i32) 21 (@$twice $saved))))`,
    42,
  ],
  [
    "macro-repeat",
    `(module
  (@macro $sum (rule (?x:expr) ?x) (rule (?x:expr ?rest:expr ...) (+ ?x (@$sum ?rest ...))))
  (func $f (export "f") (result i32) (@$sum (i32.const 10) (i32.const 12) (i32.const 20))))`,
    42,
  ],
  [
    "macro-empty-repeat",
    `(module
  (@macro $sequence (rule (?x:expr ...) (block ?x ...)))
  (func $f (export "f") (result i32) (block (result i32) (@$sequence) 42)))`,
    42,
  ],
  [
    "macro-type-fragment",
    `(module
  (@macro $typed (rule (?T:type ?x:literal) (@let $x (type ?T) ?x $x)))
  (func $f (export "f") (result i64) (@$typed i64 42)))`,
    42n,
  ],
  [
    "macro-binder",
    `(module
  (@macro $with (rule (?n:ident ?v:expr ?b:expr) (bind ?n ?b) (@let ?n ?v ?b)))
  (func $f (export "f") (result i32) (@let $x (type i32) 40 (@$with $x (+ $x 1) (+ $x 1)))))`,
    42,
  ],
  [
    "macro-repeat-binder",
    `(module
  (@macro $values (rule (((?n:ident ?v:expr) ...) ?b:expr) (bind ?n ?b) (@let ((?n (type i32) ?v) ...) ?b)))
  (func $f (export "f") (result i32) (@$values (($a (i32.const 20)) ($b (i32.const 22))) (+ $a $b))))`,
    42,
  ],
  [
    "macro-guard-infer",
    `(module
  (func $id (@type.params $T) (param $x $T) (result $T) $x)
  (@macro $id (rule (?x:expr) (infer (?T ?x)) (call $id<?T> ?x)))
  (@macro $width (rule (?x:expr) (when (type ?x i32)) 32) (rule (?x:expr) (when (type ?x i64)) 64))
  (func $f (export "f") (result i32) (- (@$width (@$id (i64.const 1))) 22)))`,
    42,
  ],
  [
    "macro-expected",
    `(module
  (@macro $zero (rule () (infer (?T expected)) (@let $zero (type ?T) 0 $zero)))
  (func $f (export "f") (result i64) (+ (@$zero) 42)))`,
    42n,
  ],
  [
    "macro-single-evaluation",
    `(module
  (type $C (struct (field (mut i32))))
  (@macro $twice (rule (?x:expr) (when (Add ?x)) (@let $tmp ?x (+ $tmp $tmp))))
  (func $next (param $c (ref $C)) (result i32) (block (result i32) (struct.set $C 0 $c (+ (struct.get $C 0 $c) 1)) (struct.get $C 0 $c)))
  (func $f (export "f") (result i32) (@let $c (struct.new $C 20) (@$twice (call $next $c)))))`,
    42,
  ],
  [
    "macro-discarded",
    `(module (@macro $discard (rule (?x:expr) (i32.const 42))) (func $f (export "f") (result i32) (@effect pure) (@$discard $missing)))`,
    42,
  ],
];
export const invalid = [
  ["macro-definition", "(module (@macro $bad (rule (?x:unknown) ?x)))", 211],
  ["macro-unknown", "(module (func (result i32) (@$missing)))", 212],
  [
    "macro-no-rule",
    "(module (@macro $m (rule (yes ?x:expr) ?x)) (func (result i32) (@$m no 42)))",
    213,
  ],
  ["macro-unknown-variable", "(module (@macro $m (rule (?x:expr) ?y)))", 211],
  [
    "macro-unbound-template",
    "(module (@macro $m (rule () $secret)) (func (param $secret i32) (result i32) (@$m)))",
    215,
  ],
  [
    "macro-undeclared-binder",
    "(module (@macro $m (rule (?n:ident ?x:expr ?b:expr) (@let ?n ?x ?b))) (func (result i32) (@$m $x (i32.const 42) $x)))",
    216,
  ],
  [
    "macro-escaped-body",
    "(module (@macro $m (rule (?n:ident ?b:expr) (bind ?n ?b) (block (result i32) (drop (@let ?n (i32.const 1) ?b)) ?b))) (func (result i32) (@$m $x $x)))",
    216,
  ],
  [
    "macro-type-error",
    "(module (@macro $m (rule (?x:expr) (+ ?x (i64.const 1)))) (func (result i32) (@$m (i32.const 42))))",
    110,
  ],
  [
    "macro-effect-error",
    "(module (type $M (struct (field (mut i32)))) (@macro $m (rule () (struct.new_default $M))) (func (result (ref $M)) (@effect pure) (@$m)))",
    123,
  ],
  [
    "macro-query-error",
    "(module (@macro $m (rule (?x:expr) (infer (?T ?x)) (i32.const 42))) (func (result i32) (@$m (i32.add (i64.const 1) (i32.const 2)))))",
    110,
  ],
  [
    "macro-ambiguous-query",
    "(module (@macro $m (rule (?x:expr) (infer (?T ?x)) ?x)) (func (result i32) (@$m 42)))",
    116,
  ],
  [
    "macro-recursion-limit",
    "(module (@macro $m (rule () (@$m))) (func (result i32) (@$m)))",
    217,
  ],
];
export const projects = [
  [
    "macro-private-helper",
    [
      [
        "main.walz",
        '(module (@use "./syntax.walz" (macro $twice $double)) (func $helper (param $x i32) (result i32) 0) (func $f (export "f") (result i32) (@$double (i32.const 21))))',
      ],
      [
        "syntax.walz",
        "(module (@export (macro $twice)) (func $helper (param $x i32) (result i32) (+ $x $x)) (@macro $twice (rule (?x:expr) (call $helper ?x))))",
      ],
    ],
    42,
  ],
];
export const badProjects = [
  [
    "macro-private",
    [
      ["main.walz", '(module (@use "./lib.walz" (macro $private)))'],
      ["lib.walz", "(module (@macro $private (rule () (i32.const 42))))"],
    ],
    201,
  ],
];
programs.push(
  [
    "macro-delegated-bind",
    `(module
  (@macro $with (rule (?n:ident ?v:expr ?b:expr) (bind ?n ?b) (@let ?n ?v ?b)))
  (@macro $forward (rule (?n:ident ?v:expr ?b:expr) (bind ?n ?b) (@$with ?n ?v ?b)))
  (func $f (export "f") (result i32) (@let $x (type i32) 40 (@$forward $x (+ $x 1) (+ $x 1)))))`,
    42,
  ],
  [
    "macro-private-delegation",
    `(module
  (@macro $with (rule (?n:ident ?v:expr ?b:expr) (bind ?n ?b) (@let ?n ?v ?b)))
  (@macro $answer (rule (?v:expr) (@$with $tmp ?v (+ $tmp $tmp))))
  (func $f (export "f") (result i32) (@let $tmp (type i32) 21 (@$answer $tmp))))`,
    42,
  ],
  [
    "macro-result-selector",
    `(module
  (@use "walz:core" (type $result))
  (@macro $value (rule (?x:expr) (infer (?T (result.ok ?x))) (@let $v (type ?T) 42 $v)))
  (func $f (export "f") (result i32) (@let $r (type $result<i32,i64>) (@result.ok 1) (@$value $r))))`,
    42,
  ],
  [
    "macro-generic-context",
    `(module
  (@macro $id (rule (?x:expr) (infer (?T ?x)) (@let $out (type ?T) ?x $out)))
  (func $id (@type.params $T) (param $x $T) (result $T) (@$id $x))
  (func $f (export "f") (result i64) (call $id<i64> 42)))`,
    42n,
  ],
  [
    "macro-caller-type",
    `(module
  (type $Box (struct (field i32)))
  (@macro $new (rule (?T:type ?x:expr) (struct.new ?T ?x)))
  (func $f (export "f") (result i32) (struct.get $Box 0 (@$new $Box (i32.const 42)))))`,
    42,
  ],
  [
    "macro-loop-hygiene",
    `(module
  (@macro $sum (rule (?x:expr) (@loop $loop (bind $n i32 ?x) (bind $sum i32 0) (result i32) (if (result i32) $n (then (@recur $loop (- $n 1) (+ $sum 7))) (else $sum)))))
  (func $f (export "f") (result i32) (@let $n (type i32) 6 (@$sum $n))))`,
    42,
  ],
  [
    "macro-immutable-query",
    `(module (@macro $id (rule (?x:expr) (infer (?T ?x)) ?x)) (func $f (export "f") (result i32) (@let $x (type i32) 42 (@$id $x))))`,
    42,
  ],
  [
    "macro-numeric-query",
    `(module (@macro $id (rule (?x:expr) (infer (?T ?x)) ?x)) (func $read (param i32) (result i32) (@$id (local.get 0))) (func $f (export "f") (result i32) (call $read 42)))`,
    42,
  ],
  [
    "macro-logical-class",
    `(module (@macro $which (rule (?x:expr) (when (Logical ?x)) (i32.const 0)) (rule (?x:expr) (i32.const 42))) (func $f (export "f") (result i32) (@$which (i64.const 0))))`,
    42,
  ],
);
invalid.push(
  [
    "macro-delegation-scope",
    "(module (@macro $with (rule (?n:ident ?v:expr ?b:expr) (bind ?n ?b) (@let ?n ?v ?b))) (@macro $f (rule (?n:ident ?v:expr ?b:expr) (@$with ?n ?v ?b))) (func (result i32) (@$f $x (i32.const 42) $x)))",
    216,
  ],
  [
    "macro-queried-immutable-set",
    "(module (@macro $m (rule (?x:expr) (infer (?T ?x expected)) (i32.const 42))) (func (result i32) (@let $x (type i32) 1 (@$m (block (result i32) (local.set $x 2) $x)))))",
    113,
  ],
  [
    "macro-queried-hidden-index",
    "(module (@macro $m (rule (?x:expr) (infer (?T ?x expected)) (i32.const 42))) (func (result i32) (@let $x (type i32) 1 (@$m (local.get 0)))))",
    109,
  ],
  [
    "macro-selector-mismatch",
    "(module (@macro $m (rule (?x:expr) (infer (?T (option.element ?x))) (i32.const 42))) (func (result i32) (@$m (i32.const 1))))",
    218,
  ],
  [
    "macro-repeat-length",
    "(module (@macro $m (rule ((?a:expr ...) (?b:expr ...)) (block (drop (+ ?a ?b)) ...))) (func (@$m ((i32.const 1)) ())))",
    214,
  ],
);
projects.push([
  "macro-reexport",
  [
    [
      "main.walz",
      '(module (@use "./facade.walz" (macro $forward $answer)) (func $f (export "f") (result i32) (@$answer (i32.const 21))))',
    ],
    [
      "facade.walz",
      '(module (@use "./syntax.walz" (macro $twice $forward)) (@export (macro $forward)))',
    ],
    [
      "syntax.walz",
      "(module (@export (macro $twice)) (@macro $twice (rule (?x:expr) (+ ?x ?x))))",
    ],
  ],
  42,
]);
programs.push(
  [
    "macro-generic-type-fragment",
    `(module (@macro $typed (rule (?T:type ?x:expr) (@let $out (type ?T) ?x $out))) (func $id (@type.params $T) (param $x $T) (result $T) (@$typed $T $x)) (func $f (export "f") (result i64) (call $id<i64> 42)))`,
    42n,
  ],
  [
    "macro-delegated-repeated-binders",
    `(module
 (@macro $values (rule (((?n:ident ?v:expr) ...) ?b:expr) (bind ?n ?b) (@let ((?n (type i32) ?v) ...) ?b)))
 (@macro $forward (rule (((?n:ident ?v:expr) ...) ?b:expr) (bind ?n ?b) (@$values ((?n ?v) ...) ?b)))
 (func $f (export "f") (result i32) (@$forward (($a (i32.const 20)) ($b (i32.const 22))) (+ $a $b))))`,
    42,
  ],
  [
    "macro-query-unit",
    '(module (@macro $m (rule (?x:expr) (infer (?T ?x)) (i32.const 42))) (func $f (export "f") (result i32) (@$m (nop))))',
    42,
  ],
  [
    "macro-query-scalar-hint",
    '(module (@macro $m (rule (?x:expr) (infer (?T ?x expected)) (@let $v (type ?T) ?x $v))) (func $f (export "f") (result i64) (@$m 42)))',
    42n,
  ],
  [
    "macro-query-right-operand",
    '(module (@macro $m (rule (?x:expr) (infer (?T ?x)) (@let $v (type ?T) ?x $v))) (func $f (export "f") (result i64) (@$m (+ 20 (i64.const 22)))))',
    42n,
  ],
);
invalid.push(
  [
    "macro-bottom-query",
    "(module (@macro $m (rule (?x:expr) (infer (?T ?x expected)) (i32.const 42))) (func (result i32) (@$m (unreachable))))",
    218,
  ],
  [
    "macro-reconstructed-delegate-body",
    "(module (@macro $with (rule (?n:ident ?v:expr ?b:expr) (bind ?n ?b) (@let ?n ?v ?b))) (@macro $f (rule (?n:ident ?v:expr ?b:expr) (bind ?n ?b) (@$with ?n ?v (block (result i32) ?b)))) (func (result i32) (@$f $x (i32.const 42) $x)))",
    216,
  ],
  [
    "macro-disagreeing-delegates",
    "(module (@macro $with (rule (?n:ident ?v:expr ?b:expr) (when (type ?v i32)) (bind ?n ?b) (@let ?n ?v ?b)) (rule (?n:ident ?v:expr ?b:expr) ?v)) (@macro $f (rule (?n:ident ?v:expr ?b:expr) (bind ?n ?b) (@$with ?n ?v ?b))) (func (result i32) (@$f $x (i32.const 42) $x)))",
    216,
  ],
);
invalid.push(
  [
    "macro-query-hidden-index-in-core",
    '(module (@use "walz:core" (type $list)) (@macro $m (rule (?x:expr) (infer (?T ?x)) (i32.const 42))) (func (result i32) (@let $x (type i32) 1 (@$m (@list.new i32 (local.get 0))))))',
    109,
  ],
  [
    "macro-invalid-guard-type",
    "(module (@macro $m (rule (?x:expr) (when (type ?x $Missing)) (i32.const 0)) (rule (?x:expr) (i32.const 42))) (func (result i32) (@$m (i32.const 1))))",
    218,
  ],
);
projects.push([
  "macro-opaque-api",
  [
    [
      "main.walz",
      '(module (@use "./lib.walz" (type $Token) (func $make) (macro $read)) (func $f (export "f") (result i32) (@$read (call $make))))',
    ],
    [
      "lib.walz",
      "(module (@export (opaque $Token) (func $make) (macro $read)) (type $Token (array (mut i32))) (func $make (result (ref $Token)) (array.new $Token 42 1)) (@macro $read (rule (?x:expr) (array.get $Token ?x 0))))",
    ],
  ],
  42,
]);
badProjects.push([
  "macro-opaque-query",
  [
    [
      "main.walz",
      '(module (@use "./lib.walz" (type $Token)) (@macro $m (rule (?x:expr) (infer (?T (array.element ?x))) (i32.const 42))) (func (param $x (ref $Token)) (result i32) (@$m $x)))',
    ],
    [
      "lib.walz",
      "(module (@export (opaque $Token)) (type $Token (array (mut i32))))",
    ],
  ],
  202,
]);
programs.push([
  "macro-caller-function",
  '(module (@macro $apply (rule (?f:ident ?x:expr) (call ?f ?x))) (func $double (param $x i32) (result i32) (+ $x $x)) (func $f (export "f") (result i32) (@$apply $double (i32.const 21))))',
  42,
]);
