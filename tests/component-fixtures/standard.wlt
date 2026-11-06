(module
  (%import "wlt:std"
    (type $future) (type $result) (type $option) (type $text)
    (func $string.u64 $format))
  (import "wlt:host/event-loop@0.1.0" "defer-i64"
    (func $defer (param $ms i32) (param $value i64)
      (result (%inst $future i64))))

  (func $main (export "main") (param $n i64)
    (result (%inst $result $text i32))
    (%effect reads writes allocates host suspends)
    (%let (($pending (call $defer (i32.const 0) (local.get $n)))
           ($value (%future.await i64 (%move $pending)))
           ($label (call $format (local.get $value))))
      (%result.ok (local.get $label))))

  (func $optional (export "optional") (param $present i32)
    (result (%inst $option $text)) (%effect pure)
    (if (result (%inst $option $text)) (local.get $present)
      (then (%option.some (%text.lit "present")))
      (else (%option.none))))

  (func $future (export "future")
    (result (%inst $future (%inst $result $text i32))) (%effect pure)
    (%future.ready (%inst $result $text i32)
      (%result.ok (%text.lit "ready")))))
