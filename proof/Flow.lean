import Lean

/- Checks the bit-vector guard used by the diagnostic analysis, not a
refinement proof of the Rust interpreter or of arbitrary array operations. -/
namespace WltFlow

theorem signed_nonnegative_bound (index length : BitVec 32)
    (nonnegative : index.slt 0 = false) (bound : index.slt length = true) :
    index.ult length = true := by bv_decide

-- Omitting the nonnegative premise admits an out-of-bounds bit pattern.
example : ((-1 : BitVec 32).slt 3 = true) ∧
    ((-1 : BitVec 32).ult 3 = false) := by decide

-- An i32 addition cannot be interpreted as unbounded integer addition.
example : (0xffffffff : BitVec 32) + 1 = 0 := by decide

end WltFlow
