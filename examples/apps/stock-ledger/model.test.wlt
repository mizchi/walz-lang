(module
  (include "parse.wlt")
  (include "stock.wlt")
  (func $error-id
    (param $e (ref $stock.Error))
    (result i32)
    (match $stock.Error
      (result i32)
      (local.get $e)
      (case insufficient () (i32.const 1))
      (case overflow () (i32.const 2))
      (case capacity () (i32.const 3))
    )
  )
  (func $ok-is
    (param $r (result i64 (ref $stock.Error)))
    (param $n i64)
    (result i32)
    (match
      (result i64 (ref $stock.Error))
      (result i32)
      (local.get $r)
      (case ok ($value) (i64.eq (local.get $value) (local.get $n)))
      (case err ($e) (i32.const 0))
    )
  )
  (func $err-is
    (param $r (result i64 (ref $stock.Error)))
    (param $code i32)
    (result i32)
    (match
      (result i64 (ref $stock.Error))
      (result i32)
      (local.get $r)
      (case ok ($n) (i32.const 0))
      (case err ($e) (i32.eq (call $error-id (local.get $e)) (local.get $code)))
    )
  )
  (test "hash collisions preserve distinct Unicode and ASCII keys"
    (let $s
      (call $stock.new (i32.const 8))
      (block
        (result i32)
        (drop (call $stock.add (local.get $s) (text.lit "Aa") (i64.const 4)))
        (drop (call $stock.add (local.get $s) (text.lit "BB") (i64.const 5)))
        (drop (call $stock.add (local.get $s) (text.lit "日本語🦀") (i64.const 7)))
        (i32.and
          (i64.eq (call $stock.get (local.get $s) (text.lit "Aa")) (i64.const 4))
          (i32.and
            (i64.eq (call $stock.get (local.get $s) (text.lit "BB")) (i64.const 5))
            (i32.and
              (i64.eq
                (call $stock.get (local.get $s) (text.lit "日本語🦀"))
                (i64.const 7)
              )
              (i32.eq (call $stock.size (local.get $s)) (i32.const 3))
            )
          )
        )
      )
    )
  )
  (test "insufficient stock leaves the prior quantity unchanged"
    (let $s
      (call $stock.new (i32.const 2))
      (block
        (result i32)
        (drop (call $stock.add (local.get $s) (text.lit "a") (i64.const 10)))
        (i32.and
          (call $err-is
            (call $stock.take (local.get $s) (text.lit "a") (i64.const 11))
            (i32.const 1)
          )
          (i32.and
            (i64.eq (call $stock.get (local.get $s) (text.lit "a")) (i64.const 10))
            (call $ok-is
              (call $stock.take (local.get $s) (text.lit "a") (i64.const 3))
              (i64.const 7)
            )
          )
        )
      )
    )
  )
  (test "capacity rejects a new key but permits an existing key update"
    (let $s
      (call $stock.new (i32.const 1))
      (block
        (result i32)
        (drop (call $stock.add (local.get $s) (text.lit "a") (i64.const 1)))
        (i32.and
          (call $err-is
            (call $stock.add (local.get $s) (text.lit "b") (i64.const 1))
            (i32.const 3)
          )
          (i32.and
            (i64.eq (call $stock.get (local.get $s) (text.lit "b")) (i64.const 0))
            (i32.and
              (i32.eq (call $stock.size (local.get $s)) (i32.const 1))
              (call $ok-is
                (call $stock.add (local.get $s) (text.lit "a") (i64.const 2))
                (i64.const 3)
              )
            )
          )
        )
      )
    )
  )
  (test "u64 addition overflow is transactional and a full take is valid"
    (let $s
      (call $stock.new (i32.const 2))
      (block
        (result i32)
        (drop (call $stock.add (local.get $s) (text.lit "a") (i64.const -1)))
        (i32.and
          (call $err-is
            (call $stock.add (local.get $s) (text.lit "a") (i64.const 1))
            (i32.const 2)
          )
          (i32.and
            (i64.eq (call $stock.get (local.get $s) (text.lit "a")) (i64.const -1))
            (i32.and
              (call $ok-is
                (call $stock.take (local.get $s) (text.lit "a") (i64.const -1))
                (i64.const 0)
              )
              (i64.eq (call $stock.get (local.get $s) (text.lit "a")) (i64.const 0))
            )
          )
        )
      )
    )
  )
  (test "unknown reads and zero takes do not insert, zero add registers a key"
    (let $s
      (call $stock.new (i32.const 1))
      (block
        (result i32)
        (i32.and
          (i64.eq (call $stock.get (local.get $s) (text.lit "unknown")) (i64.const 0))
          (i32.and
            (call $ok-is
              (call $stock.take (local.get $s) (text.lit "unknown") (i64.const 0))
              (i64.const 0)
            )
            (i32.and
              (i32.eq (call $stock.size (local.get $s)) (i32.const 0))
              (i32.and
                (call $ok-is
                  (call $stock.add (local.get $s) (text.lit "a") (i64.const 0))
                  (i64.const 0)
                )
                (i32.eq (call $stock.size (local.get $s)) (i32.const 1))
              )
            )
          )
        )
      )
    )
  )
  (test "parser accepts Unicode, the u32 maximum and exact command arities"
    (i32.and
      (i32.eq
        (match $parse.Command
          (result i32)
          (call $parse.command (text.lit "ADD\t日本語🦀\t4294967295"))
          (case skip () (i32.const 0))
          (case invalid () (i32.const 1))
          (case add ($key $n) (i32.const 2))
          (case take ($key $n) (i32.const 3))
          (case show ($key) (i32.const 4))
        )
        (i32.const 2)
      )
      (i32.and
        (i32.eq
          (match $parse.Command
            (result i32)
            (call $parse.command (text.lit "TAKE\ta\t0001"))
            (case skip () (i32.const 0))
            (case invalid () (i32.const 1))
            (case add ($key $n) (i32.const 2))
            (case take ($key $n) (i32.const 3))
            (case show ($key) (i32.const 4))
          )
          (i32.const 3)
        )
        (i32.and
          (i32.eq
            (match $parse.Command
              (result i32)
              (call $parse.command (text.lit "SHOW\ta"))
              (case skip () (i32.const 0))
              (case invalid () (i32.const 1))
              (case add ($key $n) (i32.const 2))
              (case take ($key $n) (i32.const 3))
              (case show ($key) (i32.const 4))
            )
            (i32.const 4)
          )
          (i32.and
            (i32.eq
              (match $parse.Command
                (result i32)
                (call $parse.command (text.lit ""))
                (case skip () (i32.const 0))
                (case invalid () (i32.const 1))
                (case add ($key $n) (i32.const 2))
                (case take ($key $n) (i32.const 3))
                (case show ($key) (i32.const 4))
              )
              (i32.const 0)
            )
            (i32.eq
              (match $parse.Command
                (result i32)
                (call $parse.command (text.lit "# ignored"))
                (case skip () (i32.const 0))
                (case invalid () (i32.const 1))
                (case add ($key $n) (i32.const 2))
                (case take ($key $n) (i32.const 3))
                (case show ($key) (i32.const 4))
              )
              (i32.const 0)
            )
          )
        )
      )
    )
  )
  (test "parser rejects missing fields, unknown commands, signs and overflow"
    (i32.and
      (i32.eq
        (match $parse.Command
          (result i32)
          (call $parse.command (text.lit "ADD"))
          (case skip () (i32.const 0))
          (case invalid () (i32.const 1))
          (case add ($key $n) (i32.const 2))
          (case take ($key $n) (i32.const 3))
          (case show ($key) (i32.const 4))
        )
        (i32.const 1)
      )
      (i32.and
        (i32.eq
          (match $parse.Command
            (result i32)
            (call $parse.command (text.lit "SHOW\ta\t1"))
            (case skip () (i32.const 0))
            (case invalid () (i32.const 1))
            (case add ($key $n) (i32.const 2))
            (case take ($key $n) (i32.const 3))
            (case show ($key) (i32.const 4))
          )
          (i32.const 1)
        )
        (i32.and
          (i32.eq
            (match $parse.Command
              (result i32)
              (call $parse.command (text.lit "ADD\t\t1"))
              (case skip () (i32.const 0))
              (case invalid () (i32.const 1))
              (case add ($key $n) (i32.const 2))
              (case take ($key $n) (i32.const 3))
              (case show ($key) (i32.const 4))
            )
            (i32.const 1)
          )
          (i32.and
            (i32.eq
              (match $parse.Command
                (result i32)
                (call $parse.command (text.lit "DELETE\ta"))
                (case skip () (i32.const 0))
                (case invalid () (i32.const 1))
                (case add ($key $n) (i32.const 2))
                (case take ($key $n) (i32.const 3))
                (case show ($key) (i32.const 4))
              )
              (i32.const 1)
            )
            (i32.and
              (i32.eq
                (match $parse.Command
                  (result i32)
                  (call $parse.command (text.lit "ADD\ta\t-1"))
                  (case skip () (i32.const 0))
                  (case invalid () (i32.const 1))
                  (case add ($key $n) (i32.const 2))
                  (case take ($key $n) (i32.const 3))
                  (case show ($key) (i32.const 4))
                )
                (i32.const 1)
              )
              (i32.and
                (i32.eq
                  (match $parse.Command
                    (result i32)
                    (call $parse.command (text.lit "ADD\ta\t4294967296"))
                    (case skip () (i32.const 0))
                    (case invalid () (i32.const 1))
                    (case add ($key $n) (i32.const 2))
                    (case take ($key $n) (i32.const 3))
                    (case show ($key) (i32.const 4))
                  )
                  (i32.const 1)
                )
                (i32.and
                  (i32.eq
                    (match $parse.Command
                      (result i32)
                      (call $parse.command (text.lit "ADD\ta\t1\textra"))
                      (case skip () (i32.const 0))
                      (case invalid () (i32.const 1))
                      (case add ($key $n) (i32.const 2))
                      (case take ($key $n) (i32.const 3))
                      (case show ($key) (i32.const 4))
                    )
                    (i32.const 1)
                  )
                  (i32.eq
                    (match $parse.Command
                      (result i32)
                      (call $parse.command (text.lit "ADD\ta\t"))
                      (case skip () (i32.const 0))
                      (case invalid () (i32.const 1))
                      (case add ($key $n) (i32.const 2))
                      (case take ($key $n) (i32.const 3))
                      (case show ($key) (i32.const 4))
                    )
                    (i32.const 1)
                  )
                )
              )
            )
          )
        )
      )
    )
  )
)
