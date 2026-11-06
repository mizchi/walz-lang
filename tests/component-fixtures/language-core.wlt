(module
  (%use "./calculate.wlt" (func $run $calculate.run))
  (%use "wlt:core" (type $result) (type $list) (type $future) (type $option))
  (%use "./calculate.wlt" (type $Id))
  (type $Small (%newtype i32))
  (type $Float (%newtype f32))
  (type $Weight (%newtype f64))
  (func $id (export "id") (param $x $Id) (result $Id) (%effect pure) (local.get $x))

  (func $small
    (export "small")
    (param $x $Small)
    (result $Small)
    (%effect pure)
    (local.get $x))

  (func $float
    (export "float")
    (param $x $Float)
    (result $Float)
    (%effect pure)
    (local.get $x))

  (func $weight
    (export "weight")
    (param $x $Weight)
    (result $Weight)
    (%effect pure)
    (local.get $x))

  (func $ids
    (export "ids")
    (param $xs (%inst $list $Id))
    (result (%inst $list $Id))
    (%effect pure)
    (local.get $xs))

  (func $ints
    (export "ints")
    (param $xs (%inst $list i64))
    (result (%inst $list i64))
    (%effect pure)
    (local.get $xs))

  (func $ready-id
    (export "ready-id")
    (param $x $Id)
    (result (%inst $future $Id))
    (%effect pure)
    (%future.ready $Id (local.get $x)))

  (func $ready-int
    (export "ready-int")
    (param $x i64)
    (result (%inst $future i64))
    (%effect pure)
    (%future.ready i64 (local.get $x)))

  (func $maybe-id
    (export "maybe-id")
    (param $x (%inst $option $Id))
    (result (%inst $option $Id))
    (%effect pure)
    (local.get $x))

  (func $maybe-int
    (export "maybe-int")
    (param $x (%inst $option i64))
    (result (%inst $option i64))
    (%effect pure)
    (local.get $x))

  (func $compute
    (export "compute")
    (param $fail i32)
    (result (%inst $result $Id i32))
    (%effect pure)
    (call $calculate.run (local.get $fail)))

  (func $sqrt
    (export "sqrt")
    (param $n f64)
    (result i32)
    (%effect pure)
    (block $done
      (result i32)
      (drop (br_if $done (i32.const 0) (f64.lt (local.get $n) (f64.const 0))))
      (return (i32.trunc_sat_f64_s (f64.sqrt (local.get $n))))
    )))
