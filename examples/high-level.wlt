(module
  (%use "wlt:core" (type $list) (type $tuple))
  (type $State
    (struct (field $total (mut i64)) (field $count (mut i32))))
  (func $main (export "main") (result i32) (%effect reads writes allocates)
    (%let $state (%$State::{:total 0 :count 0})
      (block (result i32)
        (%for-each ($amount $weight)
          (%list.new (%inst $tuple i64 i32)
            (%tuple.new i64 i32 10 1)
            (%tuple.new i64 i32 30 1))
          (block
            (%struct.update $State $state ($total $old (+ $old $amount)))
            (%struct.update $State $state ($count $old (+ $old $weight)))))
        (%let {:total $total :count $count} $state
          (+ (i32.wrap_i64 $total) $count))))))
