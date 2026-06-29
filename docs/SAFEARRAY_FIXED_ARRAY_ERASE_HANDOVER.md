# Handover — Faithful fixed-size vs dynamic array semantics in vm3 (`Erase`, `ReDim`-rejection, `FADF_FIXEDSIZE`)

> Paste this whole file as the opening prompt of a fresh session (or: "Read
> `docs/SAFEARRAY_FIXED_ARRAY_ERASE_HANDOVER.md` and do the work"). It is
> self-contained — you should not need the prior conversation.

## 0. TL;DR

vm3 implements VBA `Erase` as "store `Empty` into the array slot" for **every**
array. That is correct for a **dynamic** array (`Dim a()` + `ReDim`) but **wrong
for a fixed-size array** (`Dim a(1 To 3)`), where real VBA *reinitializes each
element to its type default and keeps the array allocated and usable*. vm3 is now
the sole runtime **and the oracle** (the `oxvba-vm2` crate has been deleted), so it
must be correct, not "vm2-faithful."

The blocker is a **design question you must resolve first**, then implement:

> Is the inability to tell a fixed array from a dynamic array in vm3 an **OxVBA
> typing limitation** or a **VBA type limitation**? We want to follow the *exact*
> SafeArray model. Decide whether to (a) carry a runtime `FADF_FIXEDSIZE` flag on
> the array **value** (approved direction — see §3), or (b) extend the static type
> system to distinguish them — and document why.

## 1. Repo / branch state

- Repo: `C:\Work\DnaCalc\OxVba`. The vm2-retirement work is merged to `master`.
- **vm3 is the SOLE runtime and the oracle.** `crates/oxvba-vm2` is **deleted**.
  Authority order: **live Excel oracle > VBA spec > clean typed design.** Do **not**
  reintroduce "vm2-faithful" as a justification — vm2 is gone.
- The live-oracle harness pattern lives in `crates/oxvba-differential` (oracle.rs)
  and the `com_matrix_*` host tests; the maintainer runs Windows + Excel and can
  capture live behavior. Prefer validating observable semantics against it.

## 2. The bug (confirmed, reproduced)

`OxInst::ArrayErase` in `crates/oxvba-vm3/src/lib.rs` (~line 1304):

```rust
OxInst::ArrayErase { array, .. } => {
    // vm2 lowers `Erase` to "set the array variable to Empty"; match that.
    self.store(array, Variant::empty())?;
}
```

- **Dynamic array** (`Dim a()` + `ReDim a(...)`): VBA `Erase` frees the storage; the
  array becomes uninitialized (`UBound` raises until re-`ReDim`'d). vm3's store-Empty
  is **correct** here.
- **Fixed-size array** (`Dim a(1 To 3) As Long`): VBA `Erase` **reinitializes each
  element to its type default** (Long→0, Double→0, String→`""`, Variant→`Empty`,
  Object→`Nothing`, UDT→zeroed record) and **keeps the array** — `a(2)` after
  `Erase a` is `0`, not an error. vm3's store-Empty is **wrong** here.

Reproduction (vm3 today returns `Err("VBA error 13")` — indexing an `Empty` — should
be `0`):

```vba
Public result As Long
Sub Main()
    Dim arr(1 To 3) As Long
    arr(2) = 7
    Erase arr
    result = arr(2)   ' real VBA: 0 ; vm3 today: error 13
End Sub
```

## 3. The design question (resolve this FIRST, then implement)

**Why vm3 can't currently tell fixed from dynamic:**

- A top-level `Dim a(1 To 3) As Long` is lowered to a `ReDim` at proc entry
  (`bind_dim_filtered`, `crates/oxvba-bind/src/stmt.rs` ~1058-1069), so at runtime it
  is an **ordinary SafeArray — type-identical to a dynamic array.** The fixed-ness
  survives only in the declarator syntax.
- `VarTypeRef::FixedArray { element, len }` (`crates/oxvba-bundle/src/vartype.rs`)
  **exists but is used only for UDT fixed-array FIELDS** (inline record payload),
  produced by `declared_udt_field_type` (`crates/oxvba-symbol/src/scanner.rs` ~1318).
  A top-level fixed array gets `VarTypeRef::Array` (bounds lost).
- So at the `Erase` site, `bind_place` returns `VarTypeRef::Array` for **both** fixed
  and dynamic top-level arrays — indistinguishable by static type.

**Is this an OxVBA limitation or a VBA one?** (Confirm against the SAFEARRAY/oleaut
docs and, ideally, the live oracle — this paragraph is the current best
understanding, not gospel:)

- In VBA's **static type system**, `Dim a(1 To 3) As Long` and `Dim a() As Long` are
  **both** "array of Long" — there is no separate static type. The fixed/dynamic
  distinction is a **storage property carried on the SAFEARRAY value at runtime**:
  the `FADF_FIXEDSIZE` (`0x0010`) feature flag (related: `FADF_STATIC 0x0002`,
  `FADF_AUTO 0x0001`). So **VBA itself models this as a runtime value property, not a
  static type.** That means "follow the exact SafeArray type" = **carry
  `FADF_FIXEDSIZE` on the runtime array value** (approved direction §4). Extending
  the static `VarTypeRef` to carry fixed-ness for top-level arrays is possible but is
  *not* how VBA models it, and would still need a runtime carrier for
  params/copies/`Variant`-held arrays. **Validate this claim, then write the decision
  into a short design note in the commit / a doc.**

## 4. Approved direction + the threading

Carry fixed-ness on the array **value** (runtime `FADF_FIXEDSIZE`), set at
allocation, traveling with copies; `Erase` dispatches on the array's **own** flag —
exactly like real VBA. Because the dispatch is runtime, `Erase` needs no new
compile-time info; it just reads the flag.

Threading points (line numbers approximate — re-grep):

1. `crates/oxvba-runtime/src/safe_array.rs` — add `FADF_FIXEDSIZE = 0x0010`
   (consts ~35-40); add a **stored** fixed bit to the SafeArray / `RawSafeArray`
   header, **preserved across `Clone`** and surfaced in `feature_flags()` (~195,
   ~1094); ensure the COM round-trip (`RawSafeArray.fFeatures`) carries it. Key
   shapes: `SafeArray(NonNull<RawSafeArray>)` (~208), `SafeArrayBound` (~46).
2. `crates/oxvba-bundle/src/coreir.rs` — add `fixed: bool` (or a small `kind` enum)
   to `CoreStmt::ReDim` (~595-604). `CoreStmt::Erase` (~605-608) needs no change.
3. `crates/oxvba-oxir/src/inst.rs` — add `fixed` to `OxInst::ArrayRedim` (~300).
4. `crates/oxvba-oxir/src/elaborate/lower.rs` — thread `fixed` through the `ReDim`
   lowering (~644-685). The `Erase` lowering (~687-720) already handles **compound**
   targets (UDT member arrays) via materialize-and-write-back — **leave that**; only
   the runtime dispatch changes.
5. `crates/oxvba-bind/src/stmt.rs` — the fixed-`Dim` allocation (`bind_dim_filtered`
   ~1058-1069) and the UDT fixed-array-field init (`emit_udt_record_init` ~1168-1176)
   emit `ReDim` with `fixed: true`; user `ReDim` (`bind_redim` ~767) emits
   `fixed: false`.
6. `crates/oxvba-vm3/src/lib.rs` — `ArrayRedim` (~1222) builds the SafeArray with the
   fixed flag; `ArrayErase` (~1304) dispatches on the array's **own** flag: **fixed →
   rebuild a fresh array of the SAME bounds + element type + fixed flag,
   default-initialized** (≡ `ReDim`-to-current-bounds, which vm3 already
   default-inits correctly for every element type) and store back; **dynamic → store
   `Empty`** (current behavior).

## 5. Broader scope — "follow the exact SafeArray type"

`FADF_FIXEDSIZE` affects more than `Erase`. Decide how much to take on (one coherent
bead at a time):

1. **`Erase`** — the immediate driver (§2-4).
2. **`ReDim` of a fixed-size array** — in VBA this is an error ("Can't `ReDim` a
   fixed array" — *verify exact error number/timing; it is a compile-time error in
   real VBA*). Tracking fixed-ness lets vm3 reject it instead of silently
   re-dimensioning.
3. **COM marshaling** — when arrays cross to COM (`Declare` / COM calls), the
   `RawSafeArray.fFeatures` should carry `FADF_FIXEDSIZE` for fidelity. Check the COM
   bridge in `oxvba-runtime` / `oxvba-host`.

Minimum = correct `Erase`. Optimal-long-term may include ReDim-rejection + COM-flag
fidelity. The maintainer wants the optimal design — but ship it as separate,
reviewed beads.

## 6. Already done — do NOT redo

- vm2 **deleted** (W12); review found + fixed a real regression (the compound-`ByRef`
  copy-out had dropped vm2's `VariantChanged` change-detection guard — restored as
  `OxInst::VariantChanged`).
- W13: orphaned `linearize`/`Bundle`/`BundlePackage` + the `.oxb` emission removed;
  the build is vm3-only (emits `.oxi`). `Bundle`/`Op` were **kept** (vm3's built-in
  library `vba_library_bundle()` uses them).
- `Erase` of a **compound** (UDT member) array now **elaborates** (materialize-and-
  write-back). This handover is about the **runtime reset-vs-deallocate** semantics,
  which is still wrong for fixed arrays.
- Test `erase_compound_member_array_deallocates` exists
  (`crates/oxvba-differential/tests/compound_place_vm3.rs`, test 8) — it pins the
  **dynamic** member-array deallocate behavior. When fixed reset lands, **add**
  fixed-array reset tests (top-level + UDT field, every element type) and **keep**
  the dynamic test.

## 7. Acceptance criteria

- Fixed-array `Erase` resets elements (Long/Integer/Byte/Double/Currency/Date→0,
  Boolean→False, String→`""`, fixed-string `* N`→*verify*, Variant→`Empty`,
  Object→`Nothing`, UDT→recursively zeroed) **and keeps the array usable** (reads
  after `Erase` succeed).
- Dynamic-array `Erase` still deallocates.
- The fixed flag **travels** through assignment and `ByVal`/`ByRef` parameter passing
  (copies preserve it).
- (If in scope) `ReDim` of a fixed array errors per VBA; COM marshaling carries
  `FADF_FIXEDSIZE`.
- Golden unchanged (`crates/oxvba-differential/vm3_golden.snap`) unless a corpus
  program legitimately changes — re-bless (`OXVBA_BLESS_GOLDEN=1`) and report.
- Full gate: `cargo build --workspace` + `cargo test --workspace` green; `cargo
  clippy` clean on touched crates.
- Validate `Erase`/`ReDim`-rejection behavior against the **live Excel oracle** where
  possible (the live oracle is the authority).

## 8. Working discipline (the standing /goal)

Work one coherent bead at a time. **At the end of each bead, do a fresh-eyes review**
(blunders, oversights, logical errors, bugs, omissions), rework until clean, **then
commit**, then continue. Commit messages via PowerShell here-string, ending:
`Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`. The bead tracker is `br`
(namespaces `bd-9sed` = vm3 epic, `bd-aprs` = frontend); this fixed-array work
surfaced after W14, so create a new bead for it (or proceed as a focused task and
note it).

## 9. First moves for the new session

1. Read this doc, then `crates/oxvba-vm3/src/lib.rs` (`ArrayErase`/`ArrayRedim`) and
   `crates/oxvba-runtime/src/safe_array.rs` (struct, `feature_flags`, `Clone`, the
   COM round-trip).
2. Confirm the real SAFEARRAY semantics — `FADF_FIXEDSIZE`, `Erase` reset-vs-free,
   `ReDim`-rejection — against the oleaut/VBA docs and, ideally, the live oracle.
3. Resolve §3 explicitly (OxVBA limitation vs VBA limitation; runtime-value-flag vs
   type-system extension) and record the decision.
4. Implement §4 (and optionally §5), test per §7, fresh-eyes review, commit.
