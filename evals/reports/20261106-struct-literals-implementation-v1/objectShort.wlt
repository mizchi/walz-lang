(module
(%import "wlt:std" (type $text) (type $bytes))
(type $User (struct (field $name $text) (field $age i32) (field $email $text)))
(type $Bucket (struct (field $count (mut i64)) (field $total (mut i64)) (field $max (mut i64))))
(type $State (struct (field $accepted i64) (field $rejected i64) (field $ignored i64)
  (field $info (ref $Bucket)) (field $warn (ref $Bucket)) (field $error (ref $Bucket))))
(type $Pair (struct (field $left i32) (field $right i32)))
(type $Cell (struct (field $value (mut i32))))
(type $Link (struct (field $next (ref null $Link))))
(type $Job (struct (field $id $text) (field $key $bytes) (field $index i32)
  (field $duration i64) (field $deps (mut (ref null $Link)))))
(type $Trace (struct (field $log (mut i32))))
(func $mark (param $trace (ref $Trace)) (param $n i32) (result i32)
(block (result i32) (struct.set $Trace $log (local.get $trace) (i32.add (i32.mul (struct.get $Trace $log (local.get $trace)) (i32.const 10)) (local.get $n))) (local.get $n)))
(func $user (result i32)
(%let $value (%User::{
  :name "Alice"
  :age 30
  :email "alice@example.com"
}) (i32.and (i32.eq (struct.get $User $age (local.get $value)) (i32.const 30)) (i32.and (i32.eq (%text.byte-len (struct.get $User $name (local.get $value))) (i32.const 5)) (i32.eq (%text.byte-len (struct.get $User $email (local.get $value))) (i32.const 17))))))
(func $bucket (result i32)
(%let $value (%Bucket::{
  :count 7
  :total 91
  :max 22
}) (i32.and (i64.eq (struct.get $Bucket $count (local.get $value)) (i64.const 7)) (i32.and (i64.eq (struct.get $Bucket $total (local.get $value)) (i64.const 91)) (i64.eq (struct.get $Bucket $max (local.get $value)) (i64.const 22))))))
(func $nested (result i32)
(%let $value (%State::{
  :accepted 11
  :rejected 2
  :ignored 3
  :info (%Bucket::{
    :count 10
    :total 100
    :max 17
  })
  :warn (%Bucket::{
    :count 4
    :total 19
    :max 8
  })
  :error (%Bucket::{
    :count 1
    :total 5
    :max 5
  })
}) (i32.and (i64.eq (struct.get $State $accepted (local.get $value)) (i64.const 11)) (i32.and (i64.eq (struct.get $Bucket $total (struct.get $State $warn (local.get $value))) (i64.const 19)) (i64.eq (struct.get $Bucket $max (struct.get $State $error (local.get $value))) (i64.const 5))))))
(func $order-body (param $trace (ref $Trace)) (result i32)
(%let $value (%Pair::{
  :right (call $mark (local.get $trace) (i32.const 2))
  :left (call $mark (local.get $trace) (i32.const 1))
}) (i32.and (i32.eq (struct.get $Trace $log (local.get $trace)) (i32.const 21)) (i32.and (i32.eq (struct.get $Pair $left (local.get $value)) (i32.const 1)) (i32.eq (struct.get $Pair $right (local.get $value)) (i32.const 2))))))
(func $cell (result i32)
(%let $value (%Cell::{
  :value 40
}) (block (result i32) (struct.set $Cell $value (local.get $value) (i32.const 41)) (i32.eq (struct.get $Cell $value (local.get $value)) (i32.const 41)))))
(func $job-body (param $id $text) (param $index i32) (param $duration i64) (result i32)
(%let $value (%Job::{
  :id (local.get $id)
  :key (%text.bytes (local.get $id))
  :index (local.get $index)
  :duration (local.get $duration)
  :deps (ref.null $Link)
}) (i32.and (i32.eq (struct.get $Job $index (local.get $value)) (i32.const 3)) (i32.and (i64.eq (struct.get $Job $duration (local.get $value)) (i64.const 40)) (ref.is_null (struct.get $Job $deps (local.get $value)))))))
(func $order (result i32) (call $order-body (struct.new $Trace (i32.const 0))))
(func $job (result i32) (call $job-body (%text.lit "task-7") (i32.const 3) (i64.const 40)))
(func $main (export "main") (result i32) (if (result i32) (i32.and (call $user) (i32.and (call $bucket) (i32.and (call $nested) (i32.and (call $order) (i32.and (call $cell) (i32.and (call $job) (i32.const 1))))))) (then (i32.const 42)) (else (i32.const 0)))))
