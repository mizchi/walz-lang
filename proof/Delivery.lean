import Lean

/-!
The ready-only readable-end model. States describe ownership of an endpoint,
not mutation of a shared GC object. Pending I/O and task cancellation are absent.
-/
namespace GcwatDelivery

inductive Future (α : Type) where
  | ready (value : α)
  | closed

def awaitFuture : Future α → Option (α × Future α)
  | .ready value => some (value, .closed)
  | .closed => none

def closeFuture : Future α → Option (Future α)
  | .ready _ => some .closed
  | .closed => none

theorem await_delivers_once (value : α) :
    awaitFuture (.ready value) = some (value, .closed) ∧
    awaitFuture (Future.closed : Future α) = none := by
  constructor <;> rfl

theorem closed_future_cannot_close_again :
    closeFuture (Future.closed : Future α) = none := rfl

inductive Stream (α : Type) where
  | open (items : List α)
  | closed

def next : Stream α → Option (Stream α × Option α)
  | .open [] => some (.open [], none)
  | .open (item :: rest) => some (.open rest, some item)
  | .closed => none

def closeStream : Stream α → Option (Stream α)
  | .open _ => some .closed
  | .closed => none

theorem stream_end_keeps_an_owner :
    next (.open [] : Stream α) = some (.open [], none) := rfl

theorem closed_stream_cannot_read : next (Stream.closed : Stream α) = none := rfl

theorem next_consumes_head (item : α) (rest : List α) :
    next (.open (item :: rest)) = some (.open rest, some item) := rfl

theorem close_invalidates_stream (items : List α) :
    closeStream (.open items) = some .closed ∧
    next (Stream.closed : Stream α) = none := by
  constructor <;> rfl

def readMany : Nat → List α → List α × List α
  | 0, items => ([], items)
  | count + 1, items =>
      match next (.open items) with
      | some (.open rest, some item) =>
          let (read, remaining) := readMany count rest
          (item :: read, remaining)
      | _ => ([], items)

theorem readMany_preserves_order (count : Nat) (items : List α) :
    readMany count items = (items.take count, items.drop count) := by
  induction count generalizing items with
  | zero => rfl
  | succ count ih =>
      cases items with
      | nil => rfl
      | cons item rest => simp [readMany, next, ih]

-- EOF carries no claim about a separate completion future's Result.
theorem eof_does_not_resolve_completion (completion : Future (Except ε Unit)) :
    (next (.open [] : Stream α), completion) =
      (some (.open [], none), completion) := rfl

example : readMany 3 [40, 2] = ([40, 2], []) := rfl
example : closeStream (Stream.closed : Stream Nat) = none := rfl

end GcwatDelivery
