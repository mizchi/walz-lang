(module
  (func $fold
    (param $fail i32)
    (result (%result i64 i32))
    (%stream.fold-result i64 i64 i32
      (%stream.cons
        (%result i64 i32)
        (%result.ok i64 i32 (i64.const 40))
        (%stream.cons
          (%result i64 i32)
          (%result.ok i64 i32 (i64.const 2))
          (%stream.empty (%result i64 i32))
        )
      )
      (i64.const 0)
      ($sum $item)
      (if
        (result (%result i64 i32))
        (local.get $fail)
        (then (%result.err i64 i32 (i32.const 7)))
        (else (%result.ok i64 i32 (i64.add (local.get $sum) (local.get $item))))
      )
    )
  )
  (%test "fold returns accumulated value"
    (%match
      (%result i64 i32)
      (result i32)
      (call $fold (i32.const 0))
      (case ok ($n) (i64.eq (local.get $n) (i64.const 42)))
      (case err ($e) (i32.const 0))
    )
  )
  (%test "callback error closes the unconsumed tail"
    (%match
      (%result i64 i32)
      (result i32)
      (call $fold (i32.const 1))
      (case ok ($n) (i32.const 0))
      (case err ($e) (i32.eq (local.get $e) (i32.const 7)))
    )
  )
  (%test "source error is propagated, callback is not entered"
    (%match
      (%result i64 i32)
      (result i32)
      (%stream.fold-result i64 i64 i32
        (%stream.cons
          (%result i64 i32)
          (%result.err i64 i32 (i32.const 9))
          (%stream.empty (%result i64 i32))
        )
        (i64.const 0)
        ($sum $item)
        (%result.ok i64 i32 (i64.const 999))
      )
      (case ok ($n) (i32.const 0))
      (case err ($e) (i32.eq (local.get $e) (i32.const 9)))
    )
  )
  (%test "result.let maps success and preserves errors, including unit"
    (%match
      (%result i64 i32)
      (result i32)
      (%result.let
        ()
        (%result %unit i32)
        i64
        (%result.ok %unit i32)
        (%result.let
          ($value)
          (%result i64 i32)
          i64
          (%result.ok i64 i32 (i64.const 40))
          (%result.ok i64 i32 (i64.add (local.get $value) (i64.const 2)))
        )
      )
      (case ok ($n) (i64.eq (local.get $n) (i64.const 42)))
      (case err ($e) (i32.const 0))
    )
  )
  (%test "empty stream returns the initial state"
    (%match
      (%result i64 i32)
      (result i32)
      (%stream.fold-result i64 i64 i32
        (%stream.empty (%result i64 i32))
        (i64.const 41)
        ($s $i)
        (%result.ok i64 i32 (i64.const 999))
      )
      (case ok ($n) (i64.eq (local.get $n) (i64.const 41)))
      (case err ($e) (i32.const 0))
    )
  )
  (%test "result.let propagates an error without evaluating its body"
    (%match
      (%result i64 i32)
      (result i32)
      (%result.let
        ($v)
        (%result i64 i32)
        i64
        (%result.err i64 i32 (i32.const 17))
        (unreachable)
      )
      (case ok ($n) (i32.const 0))
      (case err ($e) (i32.eq (local.get $e) (i32.const 17)))
    )
  )
  (%test "macro temporaries cannot capture user names"
    (%let $scoped_0
      (i64.const 42)
      (%match
        (%result i64 i32)
        (result i32)
        (%stream.fold-result i64 i64 i32
          (%stream.empty (%result i64 i32))
          (local.get $scoped_0)
          ($s $i)
          (%result.ok i64 i32 (local.get $s))
        )
        (case ok ($n) (i64.eq (local.get $n) (i64.const 42)))
        (case err ($e) (i32.const 0))
      )
    )
  )
  (%test "unit input and error payloads use no dummy binding"
    (%match
      (%result i64 %unit)
      (result i32)
      (%stream.fold-result %unit i64 %unit
        (%stream.cons
          (%result %unit %unit)
          (%result.ok %unit %unit)
          (%stream.empty (%result %unit %unit))
        )
        (i64.const 0)
        ($state)
        (%result.ok i64 %unit (i64.add (local.get $state) (i64.const 1)))
      )
      (case ok ($n) (i64.eq (local.get $n) (i64.const 1)))
      (case err () (i32.const 0))
    )
  )
)
