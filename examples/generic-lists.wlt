(module
  (%import "../stdlib/list.wlt" (func $list.map) (func $list.fold))
  (%closure $Scale (%capture $factor i64) (param $x i64) (result i64)
    (i64.mul (local.get $factor) (local.get $x)))

  (%closure $Sum (param $state i64) (param $x i64) (result i64)
    (i64.add (local.get $state) (local.get $x)))

  (%closure $Label (%capture $text %text) (param $x i64) (result %text) (local.get $text))

  (func $scale (export "scale") (param $xs (%list i64)) (param $factor i64)
    (result (%list i64)) (%effects reads writes allocates)
    (call
      (%inst $list.map i64 i64 $Scale)
      (local.get $xs)
      (%closure.new $Scale (local.get $factor))))

  (func $labels (export "labels") (param $xs (%list i64)) (param $label %text)
    (result (%list %text)) (%effects reads writes allocates)
    (call
      (%inst $list.map i64 %text $Label)
      (local.get $xs)
      (%closure.new $Label (local.get $label))))

  (func $sum (export "sum") (param $xs (%list i64)) (result i64) (%effects allocates)
    (call (%inst $list.fold i64 i64 $Sum) (local.get $xs) (%closure.new $Sum) (i64.const 0)))

  (func $identity
    (%type.params $T)
    (%where (shared $T) (nonunit $T))
    (param $x $T)
    (result $T)
    (%effects pure)
    (local.get $x))
  (export "identity" (func (%inst $identity (%list i64))))
  ;; This wrapper also checks the inferred effects of the concrete callback.
  (func $map-pure
    (%type.params $T $U $F)
    (%where
      (shared $T $U)
      (nonunit $T $U)
      (callable $F (param $T) (result $U) (%effects pure)))
    (param $xs (%list $T))
    (param $f (ref $F))
    (result (%list $U))
    (call (%inst $list.map $T $U $F) (local.get $xs) (local.get $f)))

  (func $scale-pure (export "scale-pure") (param $xs (%list i64)) (param $factor i64)
    (result (%list i64)) (%effects reads writes allocates)
    (call
      (%inst $map-pure i64 i64 $Scale)
      (local.get $xs)
      (%closure.new $Scale (local.get $factor))))

  (func $main (export "main") (result i64) (%effects reads writes allocates)
    (call $sum
      (call $scale
        (%list.new i64 (i64.const 1) (i64.const 2) (i64.const 3))
        (i64.const 7))))
  (type $Calls (struct (field (mut i32))))
  (%closure $Count (%capture $calls (ref $Calls)) (param $x i64) (result i64)
    (block
      (result i64)
      (struct.set $Calls 0
        (local.get $calls)
        (i32.add (struct.get $Calls 0 (local.get $calls)) (i32.const 1)))
      (local.get $x)))
  (%test "generic map and fold compose" (i64.eq (call $main) (i64.const 42)))
  (%test "empty map and fold do not call the callback"
    (i64.eq (call $sum (call $scale (%list.new i64) (i64.const 7))) (i64.const 0)))
  (%test "map supports nondefaultable text elements"
    (i32.eq
      (%bytes.len
        (%text.bytes
          (%list.get %text
            (call $labels (%list.new i64 (i64.const 1)) (%text.lit "abc"))
            (i32.const 0))))
      (i32.const 3)))
  (%test "map calls its concrete callback exactly once per element"
    (%let $calls (struct.new $Calls (i32.const 0))
      (block
        (result i32)
        (drop
          (call
            (%inst $list.map i64 i64 $Count)
            (%list.new i64 (i64.const 1) (i64.const 2) (i64.const 3))
            (%closure.new $Count (local.get $calls))))
        (i32.eq (struct.get $Calls 0 (local.get $calls)) (i32.const 3))))
  ))
