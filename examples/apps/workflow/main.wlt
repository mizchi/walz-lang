(module
  (include "program.wlt")
  (func $main (export "main") (result (result i32 i32))
    (effects reads writes allocates host suspends)
    (result.let ($final)
      (stream.fold-result
        (call $read-lines)
        (pair.new (ref $graph.Graph) i32 (call $graph.new) (i32.const 0))
        ($state $line)
        (call $app.advance (local.get $state) (local.get $line)))
      (let ($g $lines) (local.get $final)
        (result.ok (local.get $lines))))
  ))
