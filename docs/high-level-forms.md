# List Traversal, Struct Destructuring, and Field Updates

All three are typed-core sugar. `@for-each` expands into `let`, list operations, and `loop/if/block/recur`; struct pattern let expands into `let/struct.get`; `@struct.update` expands into `let/struct.get/struct.set`. There is no additional runtime or generic iterator. Optimization follows type, effect, ownership, and opaque checking.

See [examples/high-level.walz](../examples/high-level.walz) for an executable example. `walz run examples/high-level.walz` returns `42`. Run `just test-high-level` to verify it.

## `@for-each`

```wat
(@for-each $item $items
  (call $visit $item))

(@for-each ($key $amount) $entries
  (call $add $key $amount))

(@for-each {:category $key :amount $amount} $rows
  (call $add $key $amount))
```

The form is `(@for-each pattern list-expression unit-body)`. The list expression and its length are evaluated only once, then each element is read once in index order starting at 0. An empty list does not evaluate the body. Both the body and the entire expression have type unit. The element type comes from the list type. Elements must be shareable, non-unit values; elements containing owned values or borrows are rejected.

A pattern is `$name`, an existing tuple pattern `($first $second)`, or the struct-field pattern described below. Tuple unit payloads create no bindings under the existing destructuring rules. Each iteration binds fresh local IDs and can shadow same-named outer locals. The new bindings are not visible while evaluating the list expression.

The body undergoes ordinary effect and ownership checking, even for an empty list. Owned values created and consumed within each iteration are allowed; consuming an outer owned value on each iteration is not.

Internal index, length, and loop labels cannot be referenced from source. There are no dedicated `break/continue` forms or label arguments. To exit the whole expression early, use an outer `block` and standard `br`, or `return`. Exceptions or exits skip remaining elements. An outward `@recur` crossing the internal loop is rejected.

```wat
(block $done (result i32)
  (@for-each $n $numbers
    (if (< $n 0)
      (then (br $done $n))
      (else (nop))))
  0)
```

Arrays, views, and streams are not direct targets. Their mutability or ownership is made explicit through existing operations rather than implicitly mixed into traversal syntax.

## Struct Pattern Let

```wat
(@let {:category $key :amount $amount} $row
  (call $add $key $amount))
```

The form is `(@let {:field $binding ...} struct-expression body)`. The target must be a shareable, non-null struct reference of a known type. The target expression is evaluated once; selected fields are read once each in written order before evaluating the body. Only needed fields must be selected. Duplicate fields, duplicate binding names, unknown fields, and non-identifier bindings are errors. Empty patterns are rejected.

Each binding has exactly its field's type. There are no type annotations, nested patterns, or defaults. This form is a single `@let` and cannot be mixed into a multiple-initializer group; it can appear inside such a group.

Mutable fields also bind the value observed at the time of the read. Later assignment to the original field does not change the local value. GC references share their referents without cloning objects. Mutable-field reads have `reads`; immutable fields have the same effect as existing `struct.get`. Opaque type internals cannot be destructured outside their declaring file.

## `@struct.update`

```wat
(@struct.update $State $state
  ($accepted $old (+ $old 1)))
```

The form is `(@struct.update $Type object-expression ($field $old new-value))`. It updates one field in one expression and returns unit. As with `struct.set`, a field can be named or indexed. The type operand also accepts `(@inst $Type T...)`.

Evaluation proceeds as follows:

1. Evaluate the target expression once.
2. Read that field once and bind it to `$old`. A null target traps here, before the update expression.
3. Evaluate the update expression once. `$old` is visible only inside this expression.
4. Write the resulting value once to the same field of the same target.

The target is a shareable struct; the field must be mutable, shareable, and non-unit. The update expression expects the field type. There are no implicit conversions, moves, or partial moves. Immutable fields and external opaque types are rejected. The operation has ordinary `reads writes` effects, and additional effects of the target/update expressions are checked too.

The update expression can use ordinary calls, exceptions, and suspension. The old value is fixed before entering it. If it separately changes the same field, normal completion overwrites that change with the final write. If the update expression ends through an exception, `return`, or outward `br`, there is no final write, and earlier side effects remain. Atomic update, exclusion, and rollback are not provided. Use [atomic cells](atomic-cells.md) for shared linear-memory races.

Core, Core+JSPI, and Component use the same checked IR. Both backends are tested on Node/Chromium for retention of locals during suspension, abort on exceptions, and release of ABI temporary memory. No new Lean proof of sugar semantics preservation is provided; the existing ownership-contract checks apply unchanged.
