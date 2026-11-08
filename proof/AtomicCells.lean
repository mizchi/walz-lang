import Lean

/- Atomic-cell trace kernel. It checks the compiler's finite footprint contract,
   not the Rust parser/extractor, Wasm lowering, engine, or arbitrary JavaScript.
   Traces overapproximate control flow: any agent can execute any admitted site,
   any number of times. Host events must obey the same fixed-cell contract. -/
namespace WltAtomicCells

structure Cell where
  offset : Nat
  width : Nat
  deriving DecidableEq, Repr

structure Site where
  opcode : String
  offset : Nat
  width : Nat
  deriving Repr

structure Certificate where
  memoryBytes : Nat
  cells : List Cell
  accesses : List Site
  deriving Repr

def payloadWidth (opcode : String) : Option Nat :=
  if opcode ∈ ["i32.atomic.load", "i32.atomic.store", "i32.atomic.rmw.add",
    "i32.atomic.rmw.sub", "i32.atomic.rmw.and", "i32.atomic.rmw.or",
    "i32.atomic.rmw.xor", "i32.atomic.rmw.xchg", "i32.atomic.rmw.cmpxchg",
    "memory.atomic.wait32"] then some 4
  else if opcode ∈ ["i64.atomic.load", "i64.atomic.store", "i64.atomic.rmw.add",
    "i64.atomic.rmw.sub", "i64.atomic.rmw.and", "i64.atomic.rmw.or",
    "i64.atomic.rmw.xor", "i64.atomic.rmw.xchg", "i64.atomic.rmw.cmpxchg",
    "memory.atomic.wait64"] then some 8 else none

def Separate (a b : Cell) : Prop :=
  a.offset + a.width ≤ b.offset ∨ b.offset + b.width ≤ a.offset
instance (a b : Cell) : Decidable (Separate a b) := inferInstanceAs (Decidable (_ ∨ _))

def cellCheck (bytes : Nat) (c : Cell) : Bool :=
  decide ((c.width = 4 ∨ c.width = 8) ∧ c.offset % c.width = 0 ∧ c.offset + c.width ≤ bytes)

def layoutCheck (cells : List Cell) : Bool :=
  cells.all fun a => cells.all fun b => decide (a = b ∨ Separate a b)

def siteCheck (cells : List Cell) (s : Site) : Bool :=
  payloadWidth s.opcode == some s.width &&
    cells.any (fun c => c.offset == s.offset && c.width == s.width)

def check (c : Certificate) : Bool :=
  decide (c.memoryBytes ≤ 4294967296 ∧ c.cells.length ≤ 256) &&
  c.cells.all (cellCheck c.memoryBytes) && layoutCheck c.cells &&
  c.accesses.all (siteCheck c.cells)

structure Event where
  agent : Nat
  offset : Nat
  width : Nat
  atomic : Bool
  writes : Bool
  deriving Repr

def Canonical (cells : List Cell) (e : Event) : Prop :=
  ∃ c ∈ cells, e.offset = c.offset ∧ e.width = c.width

def SourceEvent (c : Certificate) (e : Event) : Prop :=
  ∃ s ∈ c.accesses, e.offset = s.offset ∧ e.width = s.width ∧
    e.atomic = (payloadWidth s.opcode).isSome

def HostEvent (c : Certificate) (e : Event) : Prop :=
  e.atomic = true ∧ Canonical c.cells e

def Overlap (a b : Event) : Prop :=
  a.offset < b.offset + b.width ∧ b.offset < a.offset + a.width

/- A conservative race predicate: any unordered conflicting pair involving a
   non-atomic access is forbidden, even without reasoning about happens-before. -/
def DataRace (a b : Event) : Prop :=
  a.agent ≠ b.agent ∧ Overlap a b ∧ (a.writes = true ∨ b.writes = true) ∧
    (a.atomic = false ∨ b.atomic = false)

def RaceFree (trace : List Event) : Prop :=
  ∀ a ∈ trace, ∀ b ∈ trace, ¬ DataRace a b

def UniformCells (trace : List Event) : Prop :=
  ∀ a ∈ trace, ∀ b ∈ trace, Overlap a b → a.offset = b.offset ∧ a.width = b.width

theorem source_allowed (c : Certificate) (hc : check c = true)
    (he : SourceEvent c e) : e.atomic = true ∧ Canonical c.cells e := by
  have sites : c.accesses.all (siteCheck c.cells) = true := by
    exact (Bool.and_eq_true_iff.mp hc).2
  rcases he with ⟨s, hs, ho, hw, ha⟩
  have h := List.all_eq_true.mp sites s hs
  have hwidth : payloadWidth s.opcode = some s.width := by
    simpa only [beq_iff_eq] using (Bool.and_eq_true_iff.mp h).1
  have hcell : c.cells.any (fun c => c.offset == s.offset && c.width == s.width) = true := by
    exact (Bool.and_eq_true_iff.mp h).2
  rcases List.any_eq_true.mp hcell with ⟨cell, hmem, hrange⟩
  have hrange : cell.offset = s.offset ∧ cell.width = s.width := by
    simpa only [Bool.and_eq_true, beq_iff_eq] using hrange
  constructor
  · simpa [hwidth] using ha
  · exact ⟨cell, hmem, ho.trans hrange.1.symm, hw.trans hrange.2.symm⟩

/- Every finite prefix of every interleaving, without a thread/step bound.
   Sync/notify/fence and thread-local GC steps are stuttering steps. -/
inductive Reachable (c : Certificate) : List Event → Prop where
  | initial : Reachable c []
  | stutter : Reachable c trace → Reachable c trace
  | source : Reachable c trace → SourceEvent c e → Reachable c (e :: trace)
  | host : Reachable c trace → HostEvent c e → Reachable c (e :: trace)

theorem invariant (c : Certificate) (hc : check c = true)
    (path : Reachable c trace) : ∀ e ∈ trace, e.atomic = true ∧ Canonical c.cells e := by
  induction path with
  | initial => simp
  | stutter _ ih => exact ih
  | source _ he ih =>
    intro e hm
    rcases List.mem_cons.mp hm with rfl | hm
    · exact source_allowed c hc he
    · exact ih e hm
  | host _ he ih =>
    intro e hm
    rcases List.mem_cons.mp hm with rfl | hm
    · exact he
    · exact ih e hm

theorem race_free (c : Certificate) (hc : check c = true)
    (path : Reachable c trace) : RaceFree trace := by
  intro a ha b hb hrace
  have h1 := (invariant c hc path a ha).1
  have h2 := (invariant c hc path b hb).1
  rcases hrace.2.2.2 with h | h <;> simp_all

theorem uniform_cells (c : Certificate) (hc : check c = true)
    (path : Reachable c trace) : UniformCells trace := by
  have layout : layoutCheck c.cells = true := by
    exact (Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp hc).1).2
  intro a ha b hb overlap
  rcases (invariant c hc path a ha).2 with ⟨ca, hca, hao, haw⟩
  rcases (invariant c hc path b hb).2 with ⟨cb, hcb, hbo, hbw⟩
  have pair := List.all_eq_true.mp (List.all_eq_true.mp layout ca hca) cb hcb
  have pair : ca = cb ∨ Separate ca cb := of_decide_eq_true pair
  rcases pair with same | separate
  · subst cb; exact ⟨hao.trans hbo.symm, haw.trans hbw.symm⟩
  · rcases separate with h | h
    · have := overlap.2
      simp only [hao, haw, hbo] at this
      exact False.elim (Nat.not_lt_of_ge h this)
    · have := overlap.1
      simp only [hao, hbo, hbw] at this
      exact False.elim (Nat.not_lt_of_ge h this)

theorem in_bounds (c : Certificate) (hc : check c = true)
    (path : Reachable c trace) (he : e ∈ trace) :
    (e.width = 4 ∨ e.width = 8) ∧ e.offset % e.width = 0 ∧
      e.offset + e.width ≤ c.memoryBytes := by
  rcases (invariant c hc path e he).2 with ⟨cell, hm, ho, hw⟩
  have cells := (Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp
    (Bool.and_eq_true_iff.mp hc).1).1).2
  have good := List.all_eq_true.mp cells cell hm
  have good : (cell.width = 4 ∨ cell.width = 8) ∧ cell.offset % cell.width = 0 ∧
      cell.offset + cell.width ≤ c.memoryBytes := of_decide_eq_true good
  simpa only [ho, hw] using good

/- Sanity cases and load-bearing rejection witnesses. -/
example : check ⟨65536, [⟨0,4⟩, ⟨8,8⟩], [⟨"i32.atomic.rmw.add",0,4⟩]⟩ = true := by decide
example : check ⟨65536, [⟨0,8⟩, ⟨4,4⟩], []⟩ = false := by decide
example : check ⟨65536, [⟨0,4⟩], [⟨"i32.store",0,4⟩]⟩ = false := by decide
example : check ⟨65536, [⟨0,4⟩], [⟨"i64.atomic.load",0,8⟩]⟩ = false := by decide

/- Dropping host compliance admits an actual conflicting non-atomic pair. -/
example : DataRace ⟨0,0,4,false,true⟩ ⟨1,0,4,true,true⟩ := by simp [DataRace, Overlap]
example : ¬ HostEvent ⟨65536,[⟨0,4⟩],[]⟩ ⟨0,0,4,false,true⟩ := by
  simp [HostEvent]


/- Version 2 adds immutable regions. Initialisation/publication precedes this
   trace; the host must not write those ranges after publication. Dynamic
   offsets below model the successful path of the emitted unsigned bound check.
   Private, unshared memories and worker-local GC are stuttering steps. -/
namespace Readonly
structure Region where
  offset : Nat
  length : Nat
  deriving Repr
structure Load where
  region : Nat
  opcode : String
  width : Nat
  deriving Repr
structure Cert where
  atomicCells : Certificate
  regions : List Region
  loads : List Load
  deriving Repr

def loadWidth (op : String) : Option Nat :=
  if op = "i32.load8_u" then some 1
  else if op = "i32.load" then some 4
  else if op = "i64.load" then some 8
  else if op = "v128.load" then some 16 else none

def regionCheck (bytes : Nat) (r : Region) : Bool :=
  decide (0 < r.length ∧ r.offset + r.length ≤ bytes)

def separated (c : Cell) (r : Region) : Prop :=
  c.offset + c.width ≤ r.offset ∨ r.offset + r.length ≤ c.offset
instance (c : Cell) (r : Region) : Decidable (separated c r) := inferInstanceAs (Decidable (_ ∨ _))

def layout (c : Cert) : Bool :=
  c.atomicCells.cells.all fun cell => c.regions.all fun r => decide (separated cell r)

def loadCheck (regions : List Region) (s : Load) : Bool :=
  loadWidth s.opcode == some s.width &&
    match regions[s.region]? with
    | none => false
    | some r => decide (s.width ≤ r.length)

def check (c : Cert) : Bool :=
  WltAtomicCells.check c.atomicCells &&
  decide (0 < c.regions.length ∧ c.regions.length ≤ 256) &&
  c.regions.all (regionCheck c.atomicCells.memoryBytes) &&
  layout c && c.loads.all (loadCheck c.regions)

def ReadEvent (c : Cert) (e : Event) : Prop :=
  e.atomic = false ∧ e.writes = false ∧
    ∃ r ∈ c.regions, r.offset ≤ e.offset ∧ e.offset + e.width ≤ r.offset + r.length

def Allowed (c : Cert) (e : Event) : Prop :=
  (e.atomic = true ∧ Canonical c.atomicCells.cells e) ∨ ReadEvent c e

def SourceRead (c : Cert) (e : Event) : Prop :=
  ∃ s ∈ c.loads, ∃ r ∈ c.regions,
    c.regions[s.region]? = some r ∧ e.width = s.width ∧
    e.atomic = false ∧ e.writes = (loadWidth s.opcode).isNone ∧
    r.offset ≤ e.offset ∧ e.offset + e.width ≤ r.offset + r.length

theorem source_allowed (c : Cert) (hc : check c = true) (he : SourceRead c e) : Allowed c e := by
  have sites : c.loads.all (loadCheck c.regions) = true := (Bool.and_eq_true_iff.mp hc).2
  rcases he with ⟨s, hs, r, hr, _, _, ha, hw, hlo, hhi⟩
  have checked := List.all_eq_true.mp sites s hs
  have width : loadWidth s.opcode = some s.width := by
    simpa only [beq_iff_eq] using (Bool.and_eq_true_iff.mp checked).1
  right
  exact ⟨ha, by simpa [width] using hw, r, hr, hlo, hhi⟩

/- Source extraction admits only the checked load opcodes and their guard.
   Host reads obey the same ranges; host writes obey the atomic-cell contract. -/
inductive Reachable (c : Cert) : List Event → Prop where
  | initial : Reachable c []
  | stutter : Reachable c trace → Reachable c trace
  | step : Reachable c trace → Allowed c e → Reachable c (e :: trace)

theorem invariant (path : Reachable c trace) : ∀ e ∈ trace, Allowed c e := by
  induction path with
  | initial => simp
  | stutter _ ih => exact ih
  | step _ he ih =>
    intro e hm
    rcases List.mem_cons.mp hm with rfl | hm
    · exact he
    · exact ih e hm

theorem atomic_read_separate (c : Cert) (hc : check c = true)
    (ha : Canonical c.atomicCells.cells a) (hb : ReadEvent c b) : ¬ Overlap a b := by
  have hl : layout c = true := (Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp hc).1).2
  rcases ha with ⟨cell, hcell, ao, aw⟩
  rcases hb with ⟨_, _, r, hr, bs, be⟩
  have hs := List.all_eq_true.mp (List.all_eq_true.mp hl cell hcell) r hr
  have hs : separated cell r := of_decide_eq_true hs
  intro overlap
  rcases hs with h | h
  · simp only [Overlap, ao, aw] at overlap; omega
  · simp only [Overlap, ao, aw] at overlap; omega

theorem race_free (c : Cert) (hc : check c = true) (path : Reachable c trace) : RaceFree trace := by
  intro a ha b hb race
  have aa := invariant path a ha
  have bb := invariant path b hb
  rcases aa with ⟨ha, ac⟩ | ha
  · rcases bb with ⟨hb, _⟩ | hb
    · rcases race.2.2.2 with h | h <;> simp_all
    · exact atomic_read_separate c hc ac hb race.2.1
  · rcases bb with ⟨_, hb⟩ | hb
    · exact atomic_read_separate c hc hb ha ⟨race.2.1.2, race.2.1.1⟩
    · rcases race.2.2.1 with h | h <;> simp_all [ReadEvent]

theorem readonly_in_bounds (c : Cert) (hc : check c = true) (he : ReadEvent c e) :
    e.offset + e.width ≤ c.atomicCells.memoryBytes := by
  have regions : c.regions.all (regionCheck c.atomicCells.memoryBytes) = true :=
    (Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp hc).1).1).2
  rcases he with ⟨_, _, r, hr, _, he⟩
  have valid : 0 < r.length ∧ r.offset + r.length ≤ c.atomicCells.memoryBytes :=
    of_decide_eq_true (List.all_eq_true.mp regions r hr)
  omega

example : check ⟨⟨65536,[⟨0,4⟩],[]⟩,[⟨16,64⟩],[⟨0,"v128.load",16⟩]⟩ = true := by decide
example : check ⟨⟨65536,[⟨0,4⟩],[]⟩,[⟨2,64⟩],[]⟩ = false := by decide
example : check ⟨⟨65536,[],[]⟩,[⟨16,64⟩],[⟨0,"i32.store",4⟩]⟩ = false := by decide
example : check ⟨⟨65536,[],[]⟩,[⟨16,64⟩],[⟨1,"i32.load",4⟩]⟩ = false := by decide
example : DataRace ⟨0,16,4,false,true⟩ ⟨1,16,4,false,false⟩ := by simp [DataRace, Overlap]
example : ¬ Allowed ⟨⟨65536,[],[]⟩,[⟨16,64⟩],[]⟩ ⟨0,16,4,false,true⟩ := by
  simp [Allowed, ReadEvent, Canonical]
end Readonly

open Lean in
def verifyJson (json : Json) (v1 : Bool := true) : Except String Certificate := do
  if v1 then
    if (← json.getObjValAs? Nat "version") != 1 then throw "unsupported atomic contract version"
    for key in ["readonlyRegions", "readonlyAccesses"] do
      if let .ok value := json.getObjVal? key then
        if !(← value.getArr?).isEmpty then throw "readonly access requires version 2"
  let memoryBytes ← json.getObjValAs? Nat "memoryBytes"
  let cells ← (← json.getObjValAs? (Array Json) "cells").mapM fun c => do
    let offset ← c.getObjValAs? Nat "offset"
    let width ← c.getObjValAs? Nat "width"
    return ({offset, width} : Cell)
  let accesses ← (← json.getObjValAs? (Array Json) "accesses").mapM fun s => do
    let opcode ← s.getObjValAs? String "opcode"
    let offset ← s.getObjValAs? Nat "offset"
    let width ← s.getObjValAs? Nat "width"
    return ({opcode, offset, width} : Site)
  let cert := {memoryBytes, cells := cells.toList, accesses := accesses.toList}
  if !check cert then throw "atomic-cell footprint contract rejected"
  return cert

open Lean in
def verifyReadonlyJson (json : Json) : Except String Readonly.Cert := do
  if (← json.getObjValAs? Nat "version") != 2 then throw "unsupported readonly contract version"
  let atomicCells ← verifyJson json false
  let regions ← (← json.getObjValAs? (Array Json) "readonlyRegions").mapM fun r => do
    let offset ← r.getObjValAs? Nat "offset"
    let length ← r.getObjValAs? Nat "length"
    return ({offset, length} : Readonly.Region)
  let sites ← match json.getObjVal? "readonlyAccesses" with
    | .error _ => pure #[]
    | .ok value => value.getArr?
  let loads ← sites.mapM fun s => do
    let region ← s.getObjValAs? Nat "region"
    let opcode ← s.getObjValAs? String "opcode"
    let width ← s.getObjValAs? Nat "width"
    return ({region, opcode, width} : Readonly.Load)
  let c := {atomicCells, regions := regions.toList, loads := loads.toList}
  if !Readonly.check c then throw "readonly footprint rejected"
  return c

def run (args : List String) : IO UInt32 := do
  let [path] := args | IO.eprintln "usage: AtomicCells contract.json"; return 2
  let contents ← IO.FS.readFile path
  let parsed := Lean.Json.parse contents
  if parsed.toOption.bind (fun j => (j.getObjValAs? Nat "version").toOption) == some 2 then
    match parsed >>= verifyReadonlyJson with
    | .ok _ => IO.println "published readonly-region contracts verified"; return 0
    | .error message => IO.eprintln message; return 1
  match parsed >>= (fun j => verifyJson j) with
  | .ok c => IO.println s!"atomic-cell contracts verified: {c.cells.length} cells, {c.accesses.length} sites"; return 0
  | .error message => IO.eprintln message; return 1

end WltAtomicCells
