(module
  (%import "wlt:std" (type $own) (type $result))
  (type $Id (%newtype i32))
  (type $Data (array i64))
  (type $Box (struct (field $id $Id) (field $data (ref $Data))))
  (type $Root
    (struct (field $value (mut (ref null $Box))))
  )
  (tag $Failure)
  (func $make
    (export "make")
    (result (ref $Box))
    (%effect allocates)
    (struct.new $Box
      (%newtype.wrap $Id (i32.const 42))
      (array.new $Data (i64.const 42) (i32.const 4096))))

  (func $data
    (export "data")
    (param $box (ref $Box))
    (result (ref $Data))
    (%effect pure)
    (struct.get $Box $data (local.get $box)))

  (func $root
    (export "root")
    (result (ref $Root))
    (%effect allocates)
    (struct.new $Root (ref.null $Box)))

  (func $pin
    (export "pin")
    (param $root (ref $Root))
    (param $box (ref $Box))
    (%effect writes)
    (struct.set $Root $value (local.get $root) (local.get $box)))

  (func $clear
    (export "clear")
    (param $root (ref $Root))
    (%effect writes)
    (struct.set $Root $value (local.get $root) (ref.null $Box)))

  (func $read
    (param $box (ref $Box))
    (result i32)
    (%effect pure)
    (%newtype.unwrap $Id (struct.get $Box $id (local.get $box))))

  (func $maybe-throw
    (param $fail i32)
    (%effect (throws $Failure))
    (if (local.get $fail) (then (throw $Failure)) (else (nop))))

  (func $owned
    (param $box (ref $Box))
    (param $fail i32)
    (result i32)
    (%effect pure)
    (%let $owner (%own.new (ref $Box) (local.get $box))
      (%try
        (result i32)
        (do
          (block
            (result i32)
            (call $maybe-throw (local.get $fail))
            (%match (%move $owner)
              (case own ($value) (call $read (local.get $value))))
          ))
        (catch $Failure
          ()
          (%match (%move $owner)
            (case own ($value) (call $read (local.get $value))))
        ))))

  (%closure $Inspect
    (%capture $owner (%inst $own (ref $Box)))
    (result i32)
    (%effect pure)
    (%match (%move $owner)
      (case own ($value) (return (call $read (local.get $value))))
    ))

  (func $exercise
    (export "exercise")
    (param $box (ref $Box))
    (param $mode i32)
    (result i32)
    (%effect allocates)
    (if
      (result i32)
      (i32.eqz (local.get $mode))
      (then (return (call $read (local.get $box))))
      (else
        (if
          (result i32)
          (i32.eq (local.get $mode) (i32.const 1))
          (then
            (block $done (result i32) (br $done (call $read (local.get $box))))
          )
          (else
            (if
              (result i32)
              (i32.eq (local.get $mode) (i32.const 2))
              (then
                (block $done
                  (result i32)
                  (drop (br_if $done (call $read (local.get $box)) (i32.const 1)))
                  (i32.const 0)))
              (else
                (if
                  (result i32)
                  (i32.eq (local.get $mode) (i32.const 3))
                  (then
                    (%match
                      (%result.do ($a (%result.ok (ref $Box) i32 (local.get $box))) ($b (%result.ok (ref $Box) i32 (local.get $a))) (%result.ok i32 i32 (call $read (local.get $b))))
                      (case ok ($n) (local.get $n))
                      (case err ($e) (i32.const -1))))
                  (else
                    (if
                      (result i32)
                      (i32.lt_s (local.get $mode) (i32.const 6))
                      (then
                        (call $owned
                          (local.get $box)
                          (i32.and (local.get $mode) (i32.const 1))))
                      (else
                        (%closure.call $Inspect
                          (%closure.new $Inspect (%own.new (ref $Box) (local.get $box))))
                      ))))
              ))))
      ))))
