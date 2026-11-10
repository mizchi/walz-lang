/- The v1 structured ownership checker and v2 control-flow certificates model
   available owned slots. Using consumes a resource and introduces a shared,
   nonescaping borrow. Its finally lowering and registry, exception edge
   extraction, effect inference, and Wasm code generation remain TCB. -/
import Lean

/-!
The executable ownership contract kernel. This models availability of owned
slots, not the Wasm heap, parser, extraction, lowering, or resource protocols.
The checker is total; only the untrusted JSON reader below is partial.
-/
namespace Walz

abbrev Context := List Nat
abbrev Loops := List Context
abbrev Output := Option Context

inductive Op where
  | nop
  | take (id : Nat)
  | scope (id : Nat) (body : Op)
  | seq (first second : Op)
  | branch (left right : Op)
  | loop (owners : List Nat) (body : Op)
  | again
  | stop
  deriving Repr

def add (id : Nat) : Context → Context
  | [] => [id]
  | first :: rest => if id ≤ first then id :: first :: rest else first :: add id rest
def remove (id : Nat) (ctx : Context) : Context := ctx.filter (· != id)

def bindMany : List Nat → Context → Option Context
  | [], ctx => some ctx
  | id :: ids, ctx => if id ∈ ctx then none else bindMany ids (add id ctx)

def join : Output → Output → Option Output
  | none, out | out, none => some out
  | some a, some b => if a = b then some (some a) else none

def check : Op → Context → Loops → Option Output
  | .nop, ctx, _ => some (some ctx)
  | .take id, ctx, _ => if id ∈ ctx then some (some (remove id ctx)) else none
  | .scope id body, ctx, loops => do
      if id ∈ ctx then none else do
        let out ← check body (add id ctx) loops
        match out with
        | none => some none
        | some ctx => if id ∈ ctx then none else some (some ctx)
  | .seq first second, ctx, loops => do
      let out ← check first ctx loops
      match out with
      | none => some none
      | some mid => check second mid loops
  | .branch left right, ctx, loops => do
      let a ← check left ctx loops
      let b ← check right ctx loops
      join a b
  | .loop owners body, ctx, loops => do
      let inner ← bindMany owners ctx
      let out ← check body inner (ctx :: loops)
      match out with
      | none => some none
      | some ctx => if owners.any (· ∈ ctx) then none else some (some ctx)
  | .again, ctx, loops => if loops.head? = some ctx then some none else none
  | .stop, ctx, _ => if ctx = [] then some none else none

/- Declarative availability rules, independent of the algorithm's control flow.
   A normal result names the remaining slots; none denotes recur/fail-stop. -/
inductive Valid : Context → Loops → Op → Output → Prop where
  | nop : Valid ctx loops .nop (some ctx)
  | take {id : Nat} (available : id ∈ ctx) : Valid ctx loops (.take id) (some (remove id ctx))
  | scope {id : Nat} (fresh : id ∉ ctx) (body : Valid (add id ctx) loops op out)
      (consumed : ∀ rest, out = some rest → id ∉ rest) :
      Valid ctx loops (.scope id op) out
  | seq (first : Valid ctx loops a (some mid)) (second : Valid mid loops b out) :
      Valid ctx loops (.seq a b) out
  | seqStop (first : Valid ctx loops a none) : Valid ctx loops (.seq a b) none
  | branch (left : Valid ctx loops a (some rest)) (right : Valid ctx loops b (some rest)) :
      Valid ctx loops (.branch a b) (some rest)
  | branchLeftStop (left : Valid ctx loops a none) (right : Valid ctx loops b out) :
      Valid ctx loops (.branch a b) out
  | branchRightStop (left : Valid ctx loops a out) (right : Valid ctx loops b none) :
      Valid ctx loops (.branch a b) out
  | loop (bound : bindMany owners ctx = some inner)
      (body : Valid inner (ctx :: loops) op out)
      (consumed : ∀ rest, out = some rest → ∀ id ∈ owners, id ∉ rest) :
      Valid ctx loops (.loop owners op) out
  | again (restored : loops.head? = some ctx) : Valid ctx loops .again none
  | stop : Valid [] loops .stop none

theorem check_sound (op : Op) :
    ∀ ctx loops out, check op ctx loops = some out → Valid ctx loops op out := by
  induction op with
  | nop => intro ctx loops out h; simp [check] at h; cases h; exact .nop
  | take id =>
      intro ctx loops out h
      simp only [check] at h
      split at h
      next available => cases h; exact .take available
      next => cases h
  | scope id body ih =>
      intro ctx loops out h
      simp only [check] at h
      split at h
      next => cases h
      next fresh =>
        cases hb : check body (add id ctx) loops with
        | none => simp [hb] at h
        | some result =>
          cases result with
          | none =>
              simp [hb] at h
              cases h
              exact .scope fresh (ih _ _ _ hb) (by simp)
          | some rest =>
              simp [hb] at h
              rcases h with ⟨consumed, rfl⟩
              exact .scope fresh (ih _ _ _ hb) (by simpa using consumed)
  | seq first second ia ib =>
      intro ctx loops out h
      cases ha : check first ctx loops with
      | none => simp [check, ha] at h
      | some result =>
          cases result with
          | none =>
              simp [check, ha] at h
              cases h
              exact .seqStop (ia _ _ _ ha)
          | some mid =>
              simp [check, ha] at h
              exact .seq (ia _ _ _ ha) (ib _ _ _ h)
  | branch left right ia ib =>
      intro ctx loops out h
      cases ha : check left ctx loops with
      | none => simp [check, ha] at h
      | some a =>
          cases hb : check right ctx loops with
          | none => simp [check, ha, hb] at h
          | some b =>
              simp only [check, ha, hb, Option.bind_some, bind] at h
              cases a with
              | none =>
                  simp [join] at h; cases h
                  exact .branchLeftStop (ia _ _ _ ha) (ib _ _ _ hb)
              | some a =>
                  cases b with
                  | none =>
                      simp [join] at h; cases h
                      exact .branchRightStop (ia _ _ _ ha) (ib _ _ _ hb)
                  | some b =>
                      simp [join] at h
                      rcases h with ⟨equal, rfl⟩
                      subst b
                      exact .branch (ia _ _ _ ha) (ib _ _ _ hb)
  | loop owners body ih =>
      intro ctx loops out h
      cases hb : bindMany owners ctx with
      | none => simp [check, hb] at h
      | some inner =>
          cases hc : check body inner (ctx :: loops) with
          | none => simp [check, hb, hc] at h
          | some result =>
              cases result with
              | none =>
                  simp [check, hb, hc] at h; cases h
                  exact .loop hb (ih _ _ _ hc) (by simp)
              | some rest =>
                  simp [check, hb, hc] at h
                  rcases h with ⟨consumed, rfl⟩
                  apply Valid.loop hb (ih _ _ _ hc)
                  intro rest' equal id member
                  cases equal
                  exact consumed id member
  | again =>
      intro ctx loops out h
      simp only [check] at h
      split at h
      next restored => cases h; exact .again restored
      next => cases h
  | stop =>
      intro ctx loops out h
      simp only [check] at h
      split at h
      next empty => subst ctx; cases h; exact .stop
      next => cases h

theorem removed_is_unavailable (id : Nat) (ctx : Context) : id ∉ remove id ctx := by
  simp [remove]

theorem double_take_rejected (id : Nat) (ctx : Context) (loops : Loops) :
    check (.seq (.take id) (.take id)) ctx loops = none := by
  simp only [check]
  split
  next => simp [removed_is_unavailable]
  next => rfl

/- Positive and negative witnesses. Kernel reduction checks all of these. -/
example : check (.branch (.take 0) (.take 0)) [0] [] = some (some []) := by decide
example : check (.branch (.take 0) .nop) [0] [] = none := by decide
example : check (.loop [1] (.seq (.take 1) .again)) [0] [] = some none := by decide
example : check (.loop [1] (.seq (.take 0) (.seq (.take 1) .again))) [0] [] = none := by decide
example : check (.scope 0 .nop) [] [] = none := by decide

open Lean in
partial def parseOp (json : Json) : Except String Op := do
  match ← json.getObjValAs? String "op" with
  | "nop" => return .nop
  | "take" => return .take (← json.getObjValAs? Nat "id")
  | "scope" => return .scope (← json.getObjValAs? Nat "id") (← parseOp (← json.getObjVal? "body"))
  | "seq" =>
      let items ← (← json.getObjVal? "items").getArr?
      let ops ← items.toList.mapM parseOp
      return ops.foldr Op.seq .nop
  | "branch" =>
      let items ← (← json.getObjVal? "arms").getArr?
      let ops ← items.toList.mapM parseOp
      match ops with
      | [] => throw "empty branch"
      | first :: rest => return rest.foldl Op.branch first
  | "loop" => return .loop (← json.getObjValAs? (List Nat) "owners") (← parseOp (← json.getObjVal? "body"))
  | "again" => return .again
  | "stop" => return .stop
  | other => throw s!"unknown ownership operation: {other}"

/- Finite control-flow certificates for jumps and exceptional exits. Contexts
   are availability sets, including transient owned operand-stack slots. The
   extractor supplies edges; this kernel checks every reachable transition. -/
inductive GraphStep where
  | nop | bind (id : Nat) | take (id : Nat) | absent (id : Nat) | finish
  deriving Repr, DecidableEq

def stepCheck : GraphStep → Context → Option Context
  | .nop, ctx => some ctx
  | .bind id, ctx => if id ∈ ctx then none else some (add id ctx)
  | .take id, ctx => if id ∈ ctx then some (remove id ctx) else none
  | .absent id, ctx => if id ∈ ctx then none else some ctx
  | .finish, ctx => if ctx = [] then some [] else none

inductive StepValid : Context → GraphStep → Context → Prop where
  | nop : StepValid ctx .nop ctx
  | bind {id : Nat} (fresh : id ∉ ctx) : StepValid ctx (.bind id) (add id ctx)
  | take {id : Nat} (live : id ∈ ctx) : StepValid ctx (.take id) (remove id ctx)
  | absent {id : Nat} (gone : id ∉ ctx) : StepValid ctx (.absent id) ctx
  | finish : StepValid [] .finish []

theorem stepCheck_sound (op : GraphStep) (ctx out : Context)
    (h : stepCheck op ctx = some out) : StepValid ctx op out := by
  cases op with
  | nop => simp [stepCheck] at h; subst out; exact .nop
  | bind id =>
      simp only [stepCheck] at h
      split at h
      next => cases h
      next fresh => cases h; exact .bind fresh
  | take id =>
      simp only [stepCheck] at h
      split at h
      next live => cases h; exact .take live
      next => cases h
  | absent id =>
      simp only [stepCheck] at h
      split at h
      next => cases h
      next gone => cases h; exact .absent gone
  | finish =>
      simp only [stepCheck] at h
      split at h
      next empty => subst ctx; cases h; exact .finish
      next => cases h

structure GraphNode where
  op : GraphStep
  next : List Nat
  context : Option Context

structure GraphCert where
  entry : Nat
  nodes : Array GraphNode

def annotation (g : GraphCert) (i : Nat) : Option Context :=
  (g.nodes[i]?).bind (·.context)

def nodeCheck (g : GraphCert) (n : GraphNode) : Bool :=
  n.next.all (· < g.nodes.size) && match n.context with
  | none => true
  | some ctx => ctx.eraseDups.length == ctx.length && match stepCheck n.op ctx with
    | none => false
    | some out => (if n.op = .finish then n.next.isEmpty else !n.next.isEmpty) &&
        n.next.all (fun j => annotation g j == some out)

def graphCheck (g : GraphCert) (inputs : Context) : Bool :=
  (annotation g g.entry == some inputs) && g.nodes.toList.all (nodeCheck g)

/- A successful certificate gives a valid availability transition at each
   annotated node, and the exact output at every successor. -/
theorem graph_node_sound (g : GraphCert) (inputs : Context)
    (h : graphCheck g inputs = true) {i : Nat} {n : GraphNode} {ctx : Context} (lookup : g.nodes[i]? = some n)
    (hc : n.context = some ctx) :
    ∃ out, StepValid ctx n.op out ∧ ∀ j ∈ n.next, annotation g j = some out := by
  have member : n ∈ g.nodes.toList := by
    exact Array.mem_toList_iff.mpr (Array.mem_of_getElem? lookup)
  have hn : nodeCheck g n = true := by
    have all := (Bool.and_eq_true_iff.mp h).2
    exact List.all_eq_true.mp all n member
  simp only [nodeCheck, hc] at hn
  cases hs : stepCheck n.op ctx with
  | none => simp [hs] at hn
  | some out =>
      refine ⟨out, stepCheck_sound _ _ _ hs, ?_⟩
      simp only [hs] at hn
      have edges := (Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp (Bool.and_eq_true_iff.mp hn).2).2).2
      intro j hj
      simpa using List.all_eq_true.mp edges j hj

inductive GraphPath (g : GraphCert) : Nat → Prop where
  | entry : GraphPath g g.entry
  | edge (path : GraphPath g i) (lookup : g.nodes[i]? = some n)
      (target : j ∈ n.next) : GraphPath g j

/- Every path, including arbitrary loop iterations and exception edges, keeps
   its certified availability state. No bounded unrolling is used. -/
theorem graph_path_sound (g : GraphCert) (inputs : Context)
    (h : graphCheck g inputs = true) (path : GraphPath g i) :
    ∃ ctx, annotation g i = some ctx := by
  induction path with
  | entry =>
      exact ⟨inputs, by simpa [graphCheck] using (Bool.and_eq_true_iff.mp h).1⟩
  | edge path lookup target ih =>
      rcases ih with ⟨ctx, hc⟩
      have hc' : _ := hc
      simp only [annotation, lookup, Option.bind_some] at hc'
      rcases graph_node_sound g inputs h lookup hc' with ⟨out, _, edges⟩
      exact ⟨out, edges _ target⟩

open Lean in
def parseGraph (json : Json) : Except String GraphCert := do
  let entry ← json.getObjValAs? Nat "entry"
  let nodes ← (← json.getObjVal? "nodes").getArr?
  let nodes ← nodes.mapM fun n => do
    let op ← match ← n.getObjValAs? String "op" with
      | "nop" => pure GraphStep.nop
      | "bind" => pure (GraphStep.bind (← n.getObjValAs? Nat "id"))
      | "take" => pure (GraphStep.take (← n.getObjValAs? Nat "id"))
      | "absent" => pure (GraphStep.absent (← n.getObjValAs? Nat "id"))
      | "finish" => pure GraphStep.finish
      | name => throw s!"unknown graph operation: {name}"
    let next ← n.getObjValAs? (List Nat) "next"
    let context ← n.getObjValAs? (Option Context) "context"
    return ({op, next, context} : GraphNode)
  return {entry, nodes}

open Lean in
def verifyJson (json : Json) : Except String Nat := do
  let version ← json.getObjValAs? Nat "version"
  if version != 1 && version != 2 then throw "unsupported contract version"
  let functions ← (← json.getObjVal? "functions").getArr?
  for function in functions do
    let name ← function.getObjValAs? String "name"
    let inputs ← function.getObjValAs? (List Nat) "inputs"
    if inputs.eraseDups.length != inputs.length then throw s!"duplicate owner in {name}"
    match function.getObjVal? "graph" with
    | .ok graph =>
        if version != 2 then throw "graph requires contract version 2"
        if (function.getObjVal? "body").isOk then throw "ambiguous ownership contract"
        let graph ← parseGraph graph
        if !graphCheck graph (inputs.mergeSort (· ≤ ·)) then
          throw s!"ownership graph rejected: {name}"
    | .error _ =>
        let body ← parseOp (← function.getObjVal? "body")
        match check body (inputs.mergeSort (· ≤ ·)) [] with
        | some none | some (some []) => pure ()
        | _ => throw s!"ownership contract rejected: {name}"
  return functions.size

end Walz

def main (args : List String) : IO UInt32 := do
  let [path] := args | IO.eprintln "usage: Ownership.lean contract.json"; return 2
  let contents ← IO.FS.readFile path
  match Lean.Json.parse contents >>= Walz.verifyJson with
  | .ok count => IO.println s!"ownership contracts verified: {count}"; return 0
  | .error message => IO.eprintln message; return 1
