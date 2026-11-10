# Dictionaries and List Builders

These are ordinary Walz libraries provided by `walz:core`. Type arguments must be shareable and non-unit; type, effect, ownership, and opaque checks run after instantiation. They use GC lifetime management without adding a host registry or new language syntax.

## Dictionaries with Text Keys

```text
(@use "walz:core"
  (type $dict)
  (func $dict.new) (func $dict.len) (func $dict.get)
  (func $dict.set) (func $dict.remove) (func $dict.clear) (func $dict.entries))

(ref (@inst $dict T))
```

| Operation | Result/behavior | Effect bound |
| --- | --- | --- |
| `new<T>()` | An empty dictionary reference | allocates |
| `len<T>(dict)` | Number of distinct keys as i32 | reads |
| `get<T>(dict,key)` | some(T) if present, otherwise none | reads |
| `set<T>(dict,key,value)` | Option of the previous value; none for a new key | reads/writes/allocates |
| `remove<T>(dict,key)` | Option of the removed value; none if absent | reads/writes |
| `clear<T>(dict)` | Removes all entries, retaining bucket capacity | reads/writes |
| `entries<T>(dict)` | Returns a snapshot of all current keys/values as `list<pair<text,T>>` | reads/writes/allocates |

Keys are compared as UTF-8 byte sequences of immutable text. Empty strings, NUL, and Unicode are supported without normalization. Option values still distinguish an absent key from a key storing none.

Internally, this is a chained FNV-1a hash table starting with 16 buckets. A new insertion after load reaches 1 doubles capacity. Updating an existing key does not increase the key count. Collisions use full key comparison. The enumeration order of entries is unspecified.
Updating, removing, or clearing the dictionary after entries completes does not change the keys/values in the returned list. Mutable GC references retain their referent identity. Construction uses a list builder, and the effect bound includes its internal writes.

Lookup depends on key byte length and the number of entries in the same bucket, and is linear in the worst case. Growth rehashes every entry; clear is proportional to bucket capacity; entries is proportional to capacity plus entry count. Capacity limits and allocation failure cause Wasm traps. Dictionaries are shareable mutable GC objects, so aliases observe updates.

## Builders for Immutable Lists

```text
(@use "walz:core"
  (type $list-builder)
  (func $list-builder.new) (func $list-builder.push) (func $list-builder.extend)
  (func $list-builder.len) (func $list-builder.finish) (func $list-builder.clear))

(ref (@inst $list-builder T))
```

`new<T>()` creates an empty builder, `push<T>(b,value)` appends one element, and `extend<T>(b,list)` appends elements in order from the start. `len<T>(b)` returns the number of appended elements. `finish<T>(b)` returns an immutable list in insertion order and resets the builder to empty; `clear<T>(b)` discards elements and resets it to empty.

The effect bounds are allocates for new, reads for len, writes for clear, and reads/writes/allocates for push/extend/finish. Private GC nodes make push constant time; extend and finish are proportional to element count. Clear only drops references to nodes, leaving the GC to reclaim unreachable nodes.

Completed lists are independent of builder reuse and clear. Elements containing mutable GC references retain reference identity without deep-copying their referents. Owned or borrowed elements containing Future/Stream/own/borrow and direct unit elements are rejected.

See the [generated API listing](stdlib-api.md) for complete signatures and [collections.walz](../tests/component-fixtures/collections.walz) for an executable example. `just test-collections` tests against an independent Map, Unicode, collisions/growth/removal, snapshots, and ABI live=0 on Wasmtime and both JS backends.
