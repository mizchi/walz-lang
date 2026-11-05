import Lean

/- Cooperative request/acknowledgment model. Counts describe resources still
owned by a task; transferred results belong to its caller. No liveness or
compiler/runtime refinement theorem is assumed. -/
namespace WltTasks

inductive Phase where
  | queued | running | cancelling | returned | cancelled | failed
  deriving DecidableEq, Repr

structure State where
  phase : Phase
  started : Bool
  reads : Nat
  owners : Nat
  buffers : Nat
  disposals : Nat
  deriving DecidableEq, Repr

def clean (s : State) : Prop :=
  s.reads = 0 ∧ s.owners = 0 ∧ s.buffers = 0 ∧ s.disposals = 0

instance (s : State) : Decidable (clean s) := inferInstanceAs
  (Decidable (s.reads = 0 ∧ s.owners = 0 ∧ s.buffers = 0 ∧ s.disposals = 0))

-- Before entry, arguments still belong to the caller. A terminal cancelled
-- task may not own resources. Exclude fabricated queued states with owners.
def valid (s : State) : Prop :=
  (s.phase = .queued → clean s ∧ s.started = false) ∧
  (s.phase = .cancelled → clean s)

def request (s : State) : State :=
  match s.phase with
  | .queued => {s with phase := .cancelled}
  | .running => {s with phase := .cancelling}
  | _ => s

theorem request_preserves_valid (s : State) (h : valid s) : valid (request s) := by
  cases phase : s.phase <;> simp_all [valid, request, clean]

def acknowledge (s : State) : Option State :=
  if s.phase = .cancelling ∧ clean s then some {s with phase := .cancelled}
  else none

-- The read's buffer cannot be freed while its completion is outstanding.
def freeBuffers (s : State) : Option State :=
  if s.reads = 0 ∧ s.owners = 0 then some {s with buffers := 0} else none

theorem request_is_not_ack (s : State) (h : s.phase = .running) :
    (request s).phase = .cancelling ∧
    (request s).reads = s.reads ∧ (request s).owners = s.owners ∧
    (request s).buffers = s.buffers ∧ (request s).disposals = s.disposals := by
  simp [request, h]

theorem acknowledgment_requires_cleanup (s next : State)
    (h : acknowledge s = some next) : clean next ∧ next.phase = .cancelled := by
  unfold acknowledge at h
  split at h
  next valid =>
    cases Option.some.inj h
    exact ⟨valid.2, rfl⟩
  next invalid => cases h

theorem no_free_during_read (s : State) (h : s.reads ≠ 0) :
    freeBuffers s = none := by simp [freeBuffers, h]

theorem queued_cancel_does_not_start (s : State) (h : s.phase = .queued)
    (notStarted : s.started = false) :
    (request s).phase = .cancelled ∧ (request s).started = false := by
  simp [request, h, notStarted]

theorem completion_wins (s : State) (h : s.phase = .returned) : request s = s := by
  simp [request, h]

theorem request_is_idempotent (s : State) : request (request s) = request s := by
  cases h : s.phase <;> simp [request, h]

-- Counterexample to acknowledging immediately after guest unwind: disposal
-- still owns a host resource, even though the guest holds no handles/buffers.
example : acknowledge ⟨.cancelling, true, 0, 0, 0, 1⟩ = none := by decide
example : acknowledge ⟨.cancelling, true, 0, 0, 0, 0⟩ =
    some ⟨.cancelled, true, 0, 0, 0, 0⟩ := by decide

end WltTasks
