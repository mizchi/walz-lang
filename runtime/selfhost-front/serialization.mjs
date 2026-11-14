// Frontend correctness v7: decoded syntax crosses no intermediate-text limit.
// Each name fits the Rust validator's string bound; their combined escaped
// spelling still exceeds 1 MiB. The guest also keeps its total export budget.
export const exportName = 'é'.repeat(22500)+'0';
const exports=Array.from({length:8},(_,i)=>`(export "${'é'.repeat(22500)+i}")`).join(' ');
export const programs = [
  ['decoded-export-name', `(module
    (func $entry (export "f") ${exports} (result i32) (i32.const 42)))`, 42],
  ['decoded-export-with-macro-query', `(module
    (@macro $m (rule (?x:expr) (infer (?T ?x)) (i32.const 42)))
    (func $entry (export "f") ${exports} (result i32) (@$m (i32.const 1))))`, 42],
];
export const invalid = [['decoded-export-keeps-return-error', `(module
  (func $entry (export "f") ${exports} (result i32) (i64.const 1)))`, 110]];
