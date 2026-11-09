export const macros = {
  macros: {
    category: 'Syntax extensions', label: 'Hygienic macros and inferred types',
    description: 'A declarative macro queries the ordinary checker and binds its input once. Generated locals are hygienic; expansion still passes type, effect and ownership checks.',
    features: ['user-macros', 'macro-inference'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (%macro $twice
    (rule (?value:expr)
      (infer (?T ?value))
      (when (Add ?value))
      (%let $saved (type ?T) ?value (+ $saved $saved))))
  (func $main (export "main") (result i64) (%effect pure)
    (%let $saved (type i64) 21
      (%$twice $saved))))`,
  },
  'macro-bindings': {
    category: 'Syntax extensions', label: 'Repeated binders and scope delegation',
    description: 'Both rules declare the same bind interface. Initializers see the caller: the new $a is 41, computed from the outer $a = 40. Only the final body sees the new names.',
    features: ['user-macros', 'macro-bindings'], expectation: { kind: 'run', output: '42n' },
    source: `(module
  (%macro $bindings
    (rule (((?name:ident ?value:expr) ...) ?body:expr)
      (bind ?name ?body)
      (%let ((?name (type i64) ?value) ...) ?body)))
  (%macro $with-values
    (rule (((?name:ident ?value:expr) ...) ?body:expr)
      (bind ?name ?body)
      (%$bindings ((?name ?value) ...) ?body)))
  (func $main (export "main") (result i64) (%effect pure)
    (%let $a (type i64) 40
      (%$with-values (($a (+ $a 1)) ($b (i64.const 1)))
        (+ $a $b)))))`,
  },
  'macro-type-queries': {
    category: 'Syntax extensions', label: 'Public type queries in a macro',
    description: 'option.element reads an inferred element type without depending on internal type syntax. For this value conversion alone, an ordinary call to $option.ok-or can already infer its type arguments.',
    features: ['user-macros', 'macro-inference', 'option', 'result'],
    expectation: { kind: 'run', output: '42n' },
    source: `(module
  (%use "wlt:core" (type $option) (type $result) (func $option.ok-or))
  (%macro $ok-or
    (rule (?value:expr ?error:expr)
      (infer (?T (option.element ?value)) (?E ?error))
      (call (%inst $option.ok-or ?T ?E) ?value ?error)))
  (func $main (export "main") (result i64) (%effect pure)
    (%match (%$ok-or (%option.some i64 42) (i32.const 7))
      (case ok ($value) $value)
      (case err ($code) (i64.const -1)))))`,
  },
  'macro-caller-scope': {
    category: 'Syntax extensions', label: 'Diagnostic: initializer scope',
    description: 'Captured initializers use the caller’s scope, so the second initializer cannot see the new $a. Compute dependencies in an ordinary outer %let, then pass them into the macro.',
    features: ['user-macros', 'macro-bindings', 'macro-diagnostics'],
    expectation: { kind: 'diagnostic', code: 'E_UNKNOWN_LOCAL', message: "caller's scope" },
    source: `(module
  (%macro $bindings
    (rule (((?name:ident ?value:expr) ...) ?body:expr)
      (bind ?name ?body)
      (%let ((?name ?value) ...) ?body)))
  (func $main (export "main") (result i32) (%effect pure)
    (%$bindings (($a (i32.const 20)) ($b (+ $a 22))) $b)))`,
  },
  'macro-repetition': {
    category: 'Syntax extensions', label: 'Diagnostic: trailing repetition',
    description: 'A repeated template term must end its list. The diagnostic points at the misplaced ellipsis. Built-in %result.do supports a final body; a user-defined recursive DSL needs its own binding design.',
    features: ['user-macros', 'macro-diagnostics'],
    expectation: { kind: 'diagnostic', code: 'E_MACRO', message: 'end of its list' },
    source: `(module
  (%macro $sequence
    (rule ((?value:expr ...) ?body:expr)
      (block ?value ... ?body)))
  (func $main (export "main") (result i32) (%effect pure)
    (%$sequence ((nop)) (i32.const 42))))`,
  },
  'macro-delegation': {
    category: 'Syntax extensions', label: 'Diagnostic: forward the complete body',
    description: 'Binding delegation forwards the captured body unchanged. Put a wrapper inside the callee’s binding scope; the compiler does not infer arbitrary reconstructed scopes.',
    features: ['user-macros', 'macro-bindings', 'macro-diagnostics'],
    expectation: { kind: 'diagnostic', code: 'E_MACRO_BINDING', message: 'body unchanged' },
    source: `(module
  (%macro $inner
    (rule (?name:ident ?value:expr ?body:expr)
      (bind ?name ?body) (%let ?name ?value ?body)))
  (%macro $outer
    (rule (?name:ident ?value:expr ?body:expr)
      (bind ?name ?body)
      (%$inner ?name ?value (block (result i32) ?body))))
  (func $main (export "main") (result i32) (%effect pure)
    (%$outer $answer (i32.const 42) $answer)))`,
  },
};
