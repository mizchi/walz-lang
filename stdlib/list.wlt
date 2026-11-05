(module
  (%namespace $list
    (%export (func $map) (func $fold))
    (type $Storage (%type.params $T) (%where (shared $T) (nonunit $T)) (array (mut $T)))
    ;; F is the concrete, shared closure type. Each element calls it once.
    (func $map
      (%type.params $T $U $F)
      (%where (shared $T $U) (nonunit $T $U) (callable $F (param $T) (result $U)))
      (param $xs (%list $T))
      (param $f (ref $F))
      (result (%list $U))
      (%let $length (%list.len $T (local.get $xs))
        (if
          (result (%list $U))
          (i32.eqz (local.get $length))
          (then (%list.new $U))
          (else
            (%let $out
              (array.new (%inst $Storage $U) (%closure.call $F (local.get $f) (%list.get $T (local.get $xs) (i32.const 0))) (local.get $length))
              (block
                (result (%list $U))
                (%loop $map
                  (bind $i i32 (i32.const 1))
                  (if
                    (i32.lt_u (local.get $i) (local.get $length))
                    (then
                      (block
                        (array.set
                          (%inst $Storage $U)
                          (local.get $out)
                          (local.get $i)
                          (%closure.call $F
                            (local.get $f)
                            (%list.get $T (local.get $xs) (local.get $i))))
                        (%recur $map (i32.add (local.get $i) (i32.const 1))))
                    )
                    (else (nop))))
                (%list.from-array $U (%inst $Storage $U) (local.get $out))))
          ))))

    (func $fold
      (%type.params $T $S $F)
      (%where (shared $T $S) (nonunit $T $S) (callable $F (param $S $T) (result $S)))
      (param $xs (%list $T))
      (param $f (ref $F))
      (param $initial $S)
      (result $S)
      (%loop $fold
        (bind $i i32 (i32.const 0))
        (bind $state $S (local.get $initial))
        (result $S)
        (if
          (result $S)
          (i32.eq (local.get $i) (%list.len $T (local.get $xs)))
          (then (local.get $state))
          (else
            (%recur $fold
              (i32.add (local.get $i) (i32.const 1))
              (%closure.call $F
                (local.get $f)
                (local.get $state)
                (%list.get $T (local.get $xs) (local.get $i))))
          ))))
  ))
