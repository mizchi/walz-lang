import Lean

/- Rules used by the compiler's separate must-range pass. These establish the
unsigned arithmetic rules, not correctness of extraction, inlining, the Rust
interpreter, or emitted Wasm. No source assumption or unsafe API is added. -/
namespace WalzRanges

theorem unsigned_byte (index length : Nat) (bound : index < length) :
    index ≤ length ∧ 1 ≤ length - index := by omega

theorem compose (limit start length index width : Nat)
    (start_ok : start ≤ limit) (range_ok : length ≤ limit - start)
    (index_ok : index ≤ length) (width_ok : width ≤ length - index) :
    start + index ≤ limit ∧ width ≤ limit - (start + index) := by omega

theorem offset_does_not_wrap (limit start length index : Nat)
    (limit_word : limit < 4294967296)
    (start_ok : start ≤ limit) (range_ok : length ≤ limit - start)
    (index_ok : index ≤ length) :
    (start + index) % 4294967296 = start + index := by
  apply Nat.mod_eq_of_lt
  omega

-- A wrapped sum comparison alone cannot establish a valid range.
example : ((-1 : BitVec 32) + 4).ult 4 = true ∧
    (-1 : BitVec 32).ult 4 = false := by decide

end WalzRanges
