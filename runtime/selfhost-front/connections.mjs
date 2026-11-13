// Frontend source contracts v2; frozen benchmark corpora remain unchanged.
export const connectionErrors = [
  ['retired-connect-spelling', ';; 接続\n(module\n  (@bind "counter" "next" $next))', 219, '(@bind "counter" "next" $next)'],
  ['implicit-binary-function', '(module\n  (@use "./absent.wasm" (func $next)))', 220, '(@use "./absent.wasm" (func $next))'],
  ['implicit-binary-type', '(module\n  (@use "./absent.wasm" (type $Item)))', 220, '(@use "./absent.wasm" (type $Item))'],
  ['retired-binary-descriptor', '(module\n  (@component.binary "walz:component/old" ""))', 220, '(@component.binary "walz:component/old" "")'],
  ['component-connection', '(module\n  (@connect "counter" "next" $next))', 221, '(@connect "counter" "next" $next)'],
  ['binary-constructor-selection', '(module\n  (@use "./absent.wasm" (component $Counter)))', 221, '(@use "./absent.wasm" (component $Counter))'],
  ['source-constructor-selection', '(module\n  (@use "./absent.walz" (component $Counter)))', 221, '(@use "./absent.walz" (component $Counter))'],
  ['component-entry', '(component (instance $state (instantiate $Counter)))', 221, '(component (instance $state (instantiate $Counter)))'],
  ['library-entry', '(@lib (func $answer (result i32) (i32.const 42)))', 222, '(@lib (func $answer (result i32) (i32.const 42)))'],
];
export const libraryProjects = [
  ['static-library', [
    ['main.walz', '(module (@use "./lib.walz" (func $answer)) (func (export "f") (result i32) (call $answer)))'],
    ['lib.walz', '(@lib (@export (func $answer)) (func $answer (result i32) (i32.const 42)) (func $unused (result f64) (f64.const 1)))'],
  ], 42],
  ['transitive-static-libraries', [
    ['main.walz', '(module (@use "./facade.walz" (func $answer)) (func (export "f") (result i32) (call $answer)))'],
    ['facade.walz', '(@lib (@use "./impl.walz" (func $value)) (@export (func $answer)) (func $answer (result i32) (call $value)))'],
    ['impl.walz', '(@lib (@export (func $value)) (func $value (result i32) (i32.const 42)))'],
  ], 42],
];
export const libraryErrors = [
  ...['(export "answer" (func $answer))', '(start $answer)', '(data (i32.const 0) "initialization")', '(global $state i32 (i32.const 0))', '(func $unused (export "unused"))', '(memory (export "memory") 1)', '(table (export "table") 1 funcref)'].map((field,i)=>[
    'library-abi-'+i, [
      ['main.walz', '(module (@use "./lib.walz" (func $answer)) (func (export "f") (result i32) (call $answer)))'],
      ['lib.walz', `(@lib (@export (func $answer)) (func $answer (result i32) (i32.const 42)) ${field})`],
    ], 223,
  ]),
  ['binary-file-is-not-a-static-source-module', [
    ['main.walz', '(module (@use "./lib.wasm" (func $answer)))'],
    ['lib.wasm', '(module (@export (func $answer)) (func $answer (result i32) (i32.const 42)))'],
  ], 220],
];
