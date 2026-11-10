export const asyncSamples = {
  delivery: {
    category: 'Ownership and async', label: 'Ready futures and finite streams',
    description: 'Consume readable endpoints explicitly. This example uses already-ready local data; it does not start a producer task or a host event loop.',
    features: ['futures', 'streams', 'ownership', 'result'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (@use "walz:core" (type $future) (type $stream) (type $result))
  (func $main (export "main") (result i64) (@effect suspends)
    (@let (($pending (@future.ready i64 40))
           ($source (@stream.cons (@inst $result i64 i32) (@result.ok 2)
                      (@stream.empty (@inst $result i64 i32))))
           ($base (@future.await i64 (@move $pending))))
      (@match
        (@stream.fold-result (@move $source) $base ($sum $item)
          (@result.ok (+ $sum $item)))
        (case ok ($total) $total)
        (case err ($code) (i64.const -1))))))`,
  },
  cancellation: {
    category: 'Ownership and async', label: 'Closing unused endpoints',
    description: 'Closing abandons receipt and consumes the endpoint. Here the values are local and already ready; cancelling a pending task needs the host adapter.',
    features: ['cancellation', 'futures', 'streams', 'ownership'], expectation: { kind: 'run', output: '42' },
    source: `(module
  (@use "walz:core" (type $future) (type $stream))
  (func $main (export "main") (result i32) (@effect pure)
    (@let (($unused (@future.ready i64 99))
           ($remaining (@stream.cons i64 1 (@stream.empty i64))))
      (block (result i32)
        (@future.close i64 (@move $unused))
        (@stream.close i64 (@move $remaining))
        42))))`,
  },
  async: {
    category: 'Ownership and async', label: 'Host futures and suspension',
    description: 'Await a Promise-backed host future. The playground uses the Core + JSPI adapter so the browser event loop can progress while the guest is suspended.',
    features: ['futures', 'effects'],
    expectation: { kind: 'run', output: '42n' },
    source: `(module
  (@use "walz:core" (type $future))
  (import "walz:host/event-loop@0.1.0" "defer-i64"
    (func $defer (param $ms i32) (param $value i64) (result (@inst $future i64))))
  (func $main (export "main") (result i64) (@effect host suspends)
    (@let $pending (call $defer 1 42)
      (@future.await i64 (@move $pending)))))`,
  },
  resources: {
    category: 'Ownership and async', label: 'Resources, borrows and scoped cleanup',
    description: '@using lends a borrow within a resource scope. This example needs a trusted resource policy (--bindings in the CLI); the browser rejects an untrusted destructor instead of assuming it is safe.',
    features: ['resources', 'ownership'],
    expectation: { kind: 'diagnostic', code: 'E_CHECK', message: 'resource destructor requires a trusted binding' },
    source: `(module
  (@use "walz:core" (type $borrow))
  (type $File (@resource "file" $close))
  (import "files" "open" (func $open (result i32)))
  (import "files" "read" (func $read (param $id i32) (result i32)))
  (import "files" "close" (func $close (param $id i32)))
  (func $read-file (param $file (@inst $borrow $File)) (result i32)
    (@effect host reads)
    (call $read (@resource.handle $file)))
  (func $main (export "main") (result i32) (@effect host reads writes allocates)
    (@using $file (@resource.new $File (call $open))
      (call $read-file $file))))`,
  },
};
