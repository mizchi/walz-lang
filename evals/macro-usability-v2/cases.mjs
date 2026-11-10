export const bindings = `(@macro $bindings
    (rule (((?name:ident ?value:expr) ...) ?body:expr)
      (bind ?name ?body)
      (@let ((?name ?value) ...) ?body)))`;

function optionCase(style) {
  const definitions = style === 'function' ? '' : style === 'inferred-macro'
    ? `(@macro $ok-or (rule (?value:expr ?error:expr)
      (call $option.ok-or ?value ?error)))`
    : `(@macro $ok-or (rule (?value:expr ?error:expr)
      (infer (?T (option.element ?value)) (?E ?error))
      (call (@inst $option.ok-or ?T ?E) ?value ?error)))`;
  const consumer = `(func $read (param $value (@inst $option i64))
    (result (@inst $result i64 i32)) (@effect pure)
    ${style === 'function' ? '(call $option.ok-or' : '(@$ok-or'} $value (i32.const 7)))`;
  return {name:`option-${style}`,required:true,expected:{main:'42',fallback:'-7'},comparison:'option-to-result',definitions,consumer,
    source:`(module
  (@use "walz:core" (type $option) (type $result) (func $option.ok-or))
  ${definitions}
  ${consumer}
  (func $main (export "main") (result i64) (@effect pure)
    (@match (call $read (@option.some i64 42))
      (case ok ($n) $n) (case err ($e) (i64.extend_i32_s $e))))
  (func $fallback (export "fallback") (result i64) (@effect pure)
    (@match (call $read (@option.none i64))
      (case ok ($n) $n) (case err ($e) (- (i64.const 0) (i64.extend_i32_s $e))))))`};
}

export function scalingSource(count,shape) {
  if(!Number.isInteger(count)||count<1||count>512||!['single','separate'].includes(shape))throw new Error('expected 1..512 and single/separate');
  const header=`(@macro $double (rule (?x:expr) (infer (?T ?x))
    (when (Add ?x)) (@let $value (type ?T) ?x (+ $value $value))))`;
  const body=shape==='single'
    ? `(func $main (export "main") (result i64) (@effect pure)
      (block (result i64) ${Array.from({length:count-1},()=>'(drop (@$double (i64.const 21)))').join(' ')} (@$double (i64.const 21))))`
    : `${Array.from({length:count},(_,n)=>`(func $f${n} (param $x i64) (result i64) (@effect pure) (@$double $x))`).join('\n')}
      (func $main (export "main") (result i64) (@effect pure) (call $f0 (i64.const 21)))`;
  return `(module ${header}\n${body})`;
}

export const cases = [
  ...['function','inferred-macro','projected-macro'].map(optionCase),
  {name:'delegated-independent-bindings',required:true,expected:{main:'42'},source:`(module ${bindings}
  (@macro $outer (rule (((?name:ident ?value:expr) ...) ?body:expr)
    (bind ?name ?body) (@$bindings ((?name ?value) ...) ?body)))
  (func $main (export "main") (result i32) (@effect pure)
    (@$outer (($a (i32.const 20)) ($b (i32.const 22))) (+ $a $b))))`},
  {name:'initializer-caller-scope',required:true,expected:{main:'41'},source:`(module ${bindings}
  (func $main (export "main") (result i32) (@effect pure)
    (@let $a (type i32) 40
      (@$bindings (($a (i32.const 1)) ($b $a)) (+ $a $b)))))`},
  {name:'duplicate-binding-rejected',required:true,reject:'E_MACRO_BINDING',source:`(module ${bindings}
  (func $main (export "main") (result i32) (@effect pure)
    (@$bindings (($a (i32.const 1)) ($a (i32.const 2))) $a)))`},
  {name:'sequential-binding-dsl',required:false,expected:{main:'42'},desired:'A let-like user DSL allows a later initializer to reference the earlier introduced name. This is beyond the documented caller-scope contract.',source:`(module ${bindings}
  (func $main (export "main") (result i32) (@effect pure)
    (@$bindings (($a (i32.const 20)) ($b (+ $a 22))) $b)))`},
  {name:'repeated-result-do',required:false,expected:{main:'42',fallback:'-7'},desired:'Repeat Result bindings, exposing successful payloads to the final body and short-circuiting errors.',source:`(module
  (@use "walz:core" (type $result))
  (@macro $bind-results (rule (((?name:ident ?value:expr) ...) ?body:expr)
    (bind ?name ?body) (@result.do (?name ?value) ... ?body)))
  (func $main (export "main") (result i32) (@effect pure)
    (@match (@inst $result i32 i32) (result i32)
      (@$bind-results (($a (@result.ok i32 i32 20)) ($b (@result.ok i32 i32 22))) (@result.ok (+ $a $b)))
      (case ok ($n) $n) (case err ($e) (- 0 $e))))
  (func $fallback (export "fallback") (result i32) (@effect pure)
    (@match (@inst $result i32 i32) (result i32)
      (@$bind-results (($a (@result.err i32 i32 7)) ($b (@result.ok i32 i32 22))) (@result.ok (+ $a $b)))
      (case ok ($n) $n) (case err ($e) (- 0 $e)))))`},
  {name:'wrapped-body-delegation',required:false,expected:{main:'42'},desired:'An outer binding macro wraps the forwarded body in an ordinary block. Current docs promise whole-fragment forwarding only.',source:`(module
  (@macro $inner (rule (?name:ident ?value:expr ?body:expr)
    (bind ?name ?body) (@let ?name ?value ?body)))
  (@macro $outer (rule (?name:ident ?value:expr ?body:expr)
    (bind ?name ?body) (@$inner ?name ?value (block (result i32) ?body))))
  (func $main (export "main") (result i32) (@effect pure)
    (@$outer $answer (i32.const 42) $answer)))`},
  ...[32,128,256,512].flatMap(count=>['single','separate'].map(shape=>({
    name:`scale-${shape}-${count}`,required:shape==='separate'||count===32,expected:{main:'42'},shape,count,
    desired:'Compile and execute a fixed number of macro calls within the existing budgets; vary only their distribution across functions.',source:scalingSource(count,shape),
  }))),
];

// Rejection of an exploratory task is an unmet authoring task. Accepting it and
// producing a wrong result is always a correctness failure, even if optional.
export function classify(item,{checks,executionCorrect}) {
  if(checks.some(c=>c.protocolValid===false || (c.code!==0 && (c.code!==1 || !c.diagnostic))))return {passed:false,achieved:false};
  const accepted=checks.every(c=>c.code===0);
  if(item.reject){const valid=checks.every(c=>c.code===1&&c.diagnostic?.code===item.reject);return {passed:valid,achieved:valid};}
  if(!accepted)return {passed:!item.required,achieved:false};
  return {passed:executionCorrect===true,achieved:executionCorrect===true};
}

export function describeSyntax(item,metrics) {
  return {source:metrics(item.source),consumer:item.consumer?metrics(item.consumer):undefined,
    definitions:item.definitions===undefined?undefined:item.definitions?metrics(item.definitions):{lines:0,tokens:0,maxDepth:0}};
}
