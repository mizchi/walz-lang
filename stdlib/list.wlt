(module
  (%use "wlt:core" (type $list))
  (%export (func $list.map) (func $list.fold))
  (type $Storage (%type.params $T) (%where (shared $T) (nonunit $T)) (array (mut $T)))
  ;; F is the concrete, shared closure type. Each element calls it once.
  (func $list.map
    (%type.params $T $U $F)
    (%where (shared $T $U) (nonunit $T $U) (callable $F (param $T) (result $U)))
    (param $xs (%inst $list $T))
    (param $f (ref $F))
    (result (%inst $list $U))
    (%let $length (%list.len $T $xs)
      (if
        (result (%inst $list $U))
        (i32.eqz $length)
        (then (%list.new $U))
        (else
          (%let $out
            (array.new (%inst $Storage $U) (%closure.call $F $f (%list.get $T $xs 0)) $length)
            (block
              (result (%inst $list $U))
              (%loop $map
                (bind $i i32 1)
                (if
                  (i32.lt_u $i $length)
                  (then
                    (block
                      (array.set
                        (%inst $Storage $U)
                        $out
                        $i
                        (%closure.call $F $f (%list.get $T $xs $i)))
                      (%recur $map (+ $i 1))))
                  (else (nop))))
              (%list.from-array $U (%inst $Storage $U) $out))))
      )))

  (func $list.fold
    (%type.params $T $S $F)
    (%where (shared $T $S) (nonunit $T $S) (callable $F (param $S $T) (result $S)))
    (param $xs (%inst $list $T))
    (param $f (ref $F))
    (param $initial $S)
    (result $S)
    (%loop $fold
      (bind $i i32 0)
      (bind $state $S $initial)
      (result $S)
      (if
        (result $S)
        (= $i (%list.len $T $xs))
        (then $state)
        (else
          (%recur $fold (+ $i 1) (%closure.call $F $f $state (%list.get $T $xs $i))))
      ))))
