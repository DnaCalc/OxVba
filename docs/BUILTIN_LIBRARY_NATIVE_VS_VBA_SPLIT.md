# Note: split the built-in library into native- vs VBA-implemented bodies, then rationalize

> **Status: deferred idea — do AFTER the vm3-vs-spec gap-closure program (epic `bd-4ktq`,
> `docs/VM3_VBA_SPEC_GAP_INVENTORY.md`) is complete.** Captured 2026-06-29 from a maintainer
> note. This is a *direction*, not a finalized plan — the design questions below are open.

## Where we are

The **builtins-as-library** program is done: the whole VBA built-in surface (constants, all
library functions, file statements, the `Collection` class) resolves + dispatches as members of
a synthetic internal **"VBA" library bundle**, exactly like a referenced project, via the one
cross-bundle member-call path. `NativeImplId` is now a **body identity**, not a dispatch route.

But **every body in that bundle is native Rust** (`NativeBody::Library(NativeImplId)` →
`oxvba-lib`, or `NativeBody::Method(NativeMethodId)` → the Collection shim). The bundle already
supports *both* body kinds — a bundle proc can carry either a native body **or** ordinary VBA
bytecode (`ProcBody::Bytecode`). Nothing yet uses a VBA-source body for a built-in.

## The idea

Split the library's bodies into two deliberate categories and rationalize from there:

1. **Native (Rust) — keep** for anything that *must* be native:
   - host/OS surface: file I/O, `Shell`/`Environ`/`Command`, `GetSetting` family, `Dir`, time;
   - low-level codecs + bit/format work: `Chr`/`Asc`/`ChrW`/`AscW`, `Hex`/`Oct`, `Format` engine,
     `StrConv`;
   - type coercion / numeric primitives + exact Currency/Decimal math: `CLng`/`CDbl`/…, `Int`/`Fix`,
     `Sqr`/`Log`/`Sin`/…, banker's rounding, overflow;
   - object activation + COM: `CreateObject`/`GetObject`;
   - special forms + structural intrinsics stay compiler-internal (already excepted):
     `IIf`/`Choose`/`Switch`/`Array`/`TypeOf`/`UBound`/`LBound`/`AddressOf`/`CallByName`,
     `VarPtr`/`StrPtr`/`ObjPtr`.

2. **VBA-implemented — migrate/author** for anything cleanly *composable from the native
   primitives*, written as real VBA source in the synthetic bundle:
   - functions that are pure compositions: `IsDate` (→ `CDate` + error trap), `Partition`
     (arithmetic + `Format`), the `FormatNumber`/`FormatCurrency`/`FormatPercent`/`FormatDateTime`
     family (→ `Format` with a synthesized picture), the financial `IPmt`/`PPmt`/`SLN`/`SYD`/`DDB`
     (derive from `Pmt`/`FV`), `WeekdayName`/`MonthName`, `RGB`/`QBColor` (arithmetic);
   - candidates surfaced by the gap audit as **Absent** are prime VBA-body targets (cheap to add,
     correct-by-construction): see the "Absent" rows in `docs/VM3_VBA_SPEC_GAP_INVENTORY.md`.

### Why

- **Smaller native surface** to maintain, audit, and keep `unsafe`-clean.
- **Correct-by-construction + self-testing**: a VBA-authored body *is* VBA and runs on the same VM,
  so it can't diverge from VBA semantics the way a hand-written Rust reimplementation can — and it
  exercises the VM's own primitives (a built-in differential test for free).
- **JIT-uniform**: VBA-bodied built-ins compile through the normal pipeline like any user code; only
  the genuine native primitives need a JIT thunk.
- A natural, low-risk way to **fill the Absent-function gaps** from the audit.

## Rationalization / cleanup to fold in (independent of the split)

- **Delete the dead keyless `oxvba-lib` Collection first-cut** if still present (`pure::collection_*`
  + `NativeImplId::Collection*` + catalog stubs) — superseded by the runtime/eval Collection; keep
  the keyed `NativeMethodId::Collection*`. (See `project_builtins_as_library` memory.)
- **Fix the stale `oxvba-lib` doc comments** claiming Collection "awaits the vm2 object model" /
  "first-cut" — Collection is implemented (runtime + eval + vm3).
- **One error-mapping table**: consolidate the scattered `LibError`/`CollectionError`/`HalError` →
  VBA-error-number maps (the audit's `hal-errors-flattened-to-5`, `collection-keynotfound-error-9-not-5`,
  `sparse-default-error-message` all touch this).
- **Consistent `$`-suffix vs non-`$` return-type handling** and the Null-propagation policy
  (audit `null-not-propagated-string-fns`) — once, in the shared dispatch, not per-function.
- Decide whether the synthetic-bundle module grouping (Strings/Math/DateTime/Information/Conversion/
  FileSystem/Interaction/Financial + classes) is the canonical public shape.

## Open questions (resolve when the work starts)

- **Where do the VBA source bodies live + how are they built?** A checked-in `.bas`/`.basproj` for
  the "VBA" bundle compiled at build time into the `&'static` bundle? Or authored Core IR? This is
  the main design decision.
- **Bootstrapping / cycles**: a VBA-bodied `IsDate` calling native `CDate` is fine; guard against a
  VBA body depending on another not-yet-linked VBA body, and against recursion into the bundle during
  its own link.
- **Performance**: VBA-bodied built-ins on the hot path cost a frame; measure vs the native shortcut
  (`call_extern` currently shortcuts `NativeBody::Library` with no frame). Keep hot primitives native.
- **Error fidelity**: a VBA body raises via the normal `Err.Raise`; confirm the resulting `Err.Number`
  matches what real VBA raises for that built-in.

## Gating

Do this **after** vm3 is spec-complete (the `bd-4ktq` gap-closure inventory is worked down),
so the split lands against correct, fully-covered semantics rather than a moving target — and so the
Absent-function gaps are filled as VBA bodies in one coherent pass rather than piecemeal.
