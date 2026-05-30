# Workset: Default Host Project & Predeclared VBA Library Surface

Date: 2026-05-30
Owner: (unassigned)
Status: proposed
Bead root: (to be assigned)
Related: `docs/EXTERNAL_VBA_CORPUS.md` (F1 `vb*` constants), the VBA 7.1 conditional-
compilation predefinition work, and the external corpus bug-hunt that surfaced it.

## Purpose

Replace the scattered, parser-hardcoded VBA standard-library surface with a single
**default host project** — a descriptor-backed predeclared library that every project
implicitly references, the way real VBA implicitly references the `VBA` type library.

Today the "always available" VBA surface is spread across ad-hoc match arms in the
compiler. Adding `vb*` constants (F1) meant adding yet another table in `resolve.rs`,
next to `Debug`, `Err`, and the intrinsic-function registry. That is the smell this
workset addresses: the VBA library should be **data resolved through the normal
reference/metadata machinery**, not hand-written special cases in the expression parser.

## First-pass / no-compatibility rule

Per project convention ([[feedback_no_backward_compat]]): this is a first-pass boundary
correction toward a clean end state.

- Move the predeclared VBA surface to the new model and **delete** the scattered
  special-cases it replaces; do not keep both.
- Do not preserve the old `intrinsic_vb_constant` / `intrinsic_spec` / `Debug`/`Err`
  parser arms as a compatibility layer once the library model covers them.
- Use git history for archeology; do not retain dead paths.
- Tests should assert resolution **through the library model**, not parser-arm equivalence.

## Current state (inventory)

All hardcoded in `crates/oxvba-compiler/src/resolve.rs`, by category:

| Surface | Today | Location |
| --- | --- | --- |
| `vb*` constants (`vbCrLf`, `vbBinaryCompare`, `vbYesNo`, …) | `intrinsic_vb_constant()` match → literal `BoundExpr` | `resolve.rs` (parse_expr) |
| Intrinsic functions (`Len`, `Mid`, `MsgBox`, `Rnd`, `Chr`, …) | `intrinsic_spec()` name→(arity, surface) registry | `resolve.rs:5512` |
| `Debug.Print` | statement special-case → `BoundStmt::DebugPrint` | `resolve.rs:3115` |
| `Err.Raise/Clear/Number/LastDllError` | statement + member special-cases | `resolve.rs:2911,2919,6258` |
| Conditional-compilation constants (`VBA7`, `Win64`, …) | `builtin_pp_constants()` | `resolve.rs:1204` |

Observations:
- There is **no** model of "the VBA library" as a referenceable thing. There is no
  predeclared-globals project, despite `ReferenceKind::{TypeLibrary, HostInjected, Project}`
  and a working COM-typelib metadata-blob path already existing for *external* references.
- The surface is split across four mechanisms (literal table, call registry, statement
  special-cases, PP-constant table), so coverage is uneven and extension is per-mechanism.
- `Conditional-compilation` constants are a genuinely separate phase (preprocessor) and
  are **out of scope** for the library model — they stay in `builtin_pp_constants`.

## Goals

1. Model the always-available VBA surface as a **default reference** (the "VBA" library)
   carrying descriptors: constants (name→typed value), functions (name, arity, return/param
   facts, host-sensitivity), and predeclared objects (`Debug`, `Err`) with their members.
2. Resolve/typecheck/emit the VBA surface through the **same metadata path** used for COM
   typelib references and project references — not bespoke parser arms.
3. Make the default reference **injectable/overridable**: a host (Excel, Access, CLI,
   headless) can extend it with host globals (`Application`, `ThisWorkbook`, …) or swap the
   host library, without compiler edits.
4. Net-delete the scattered special-cases once covered; single source of truth for "what is
   predeclared".

## Non-goals

- Not changing conditional-compilation (`#If`) constant handling — that stays a preprocessor
  concern in `builtin_pp_constants`.
- Not implementing the *full* VBA library (every `Financial`/`DateTime` function) in one
  pass — the model must make incremental coverage cheap, but completeness is iterative.
- Not adding host-specific object models (Excel/Word) in this workset — only the seam that
  lets a host inject them.
- Not claiming Office/VBA parity.

## Design

### What "default host project" means here

In real VBA every project implicitly references the **VBA** type library (global namespace
`VBA` with modules `Constants`, `Math`, `Strings`, `Information`, `Interaction`, … and the
predeclared objects `Err`, `Debug`, `Collection`). A "default host project" is OxVba's
analog: a built-in library descriptor that is **always present as a reference**, plus a
host-supplied extension slot. User code, the VBA library, and host globals then all resolve
through one descriptor-driven name-resolution path.

### Alternatives considered

**A. Status quo (scattered parser hardcoding).** Keep adding match arms.
- *Pros:* zero refactor; fast for one-off additions (what F1 did).
- *Cons:* four divergent mechanisms; uneven coverage; no host override; the parser owns
  library semantics; every addition is bespoke. Rejected as the end state.

**B. Single in-compiler VBA-library descriptor table (data-driven, still in-compiler).**
One `vba_library()` descriptor (constants + functions + predeclared objects) consumed by
resolve/typecheck/emit; replaces the four scattered mechanisms with one table.
- *Pros:* big consolidation win; moderate effort; single source of truth; keeps everything
  in-process and fast (no project load).
- *Cons:* still compiler-internal, not a *reference*; host extension is a second mechanism
  bolted on; doesn't reuse the existing reference/metadata-blob machinery.

**C. Real injected VBA-library reference (descriptor blob via the reference path).**
Model the VBA library as a `HostInjected`/built-in reference whose metadata blob flows
through the same resolution path as COM typelibs and project references. The default
reference set = `{ VBA library }` (+ optional host library). Resolution order:
local project → injected VBA library → host library.
- *Pros:* this is the actual "default host project"; one resolution path for user code,
  library, host globals, and COM; host swap/extend is first-class (just another injected
  reference); aligns with `ReferenceKind` + the COM metadata-blob infra that already exists.
- *Cons:* largest; must define a descriptor schema that covers value-constants and
  predeclared objects (the COM blob is interface/method-centric — constants and module-level
  functions need representation); must keep the hot path fast (cache the built-in blob, no
  per-compile rebuild).

### Recommendation

**Stage B → C.** Land B first (consolidate the four mechanisms into one descriptor table —
immediately deletes the scattered arms and fixes coverage), then promote that descriptor to
a real injected reference (C) so host extension and the unified resolution path fall out.
B is a strict, low-risk improvement and a natural substrate for C; C is the end state the
user asked for. Each stage is independently shippable and net-deletes code.

Rationale for staging rather than jumping to C: the descriptor *schema* (how to represent
typed value-constants and predeclared objects like `Debug`/`Err` with members) is the hard
design question. B forces us to nail the schema against real consumers (resolve/typecheck/
emit) before also taking on the reference-injection plumbing. Doing both at once couples two
unknowns.

## Boundary principles

- **Compiler owns neutral facts**, not library policy: it resolves names against descriptors
  and emits calls/values; it does not enumerate `vbYesNo = 4` in match arms.
- **The VBA library is data** (a descriptor blob), versioned to the dialect (VBA 7.1).
- **The host owns its globals**: `Application`/`ThisWorkbook`/etc. are a host-supplied
  reference, never compiler-baked.
- **Conditional compilation stays separate** (preprocessor phase, `builtin_pp_constants`).
- **Determinism/host-sensitivity is a descriptor fact** (carry the existing
  `IntrinsicSurface::{DeterministicCore, HostSensitive}` notion into the schema).

## Phased plan

**Phase 0 — schema design.** Define the VBA-library descriptor: value-constants
(name → typed literal: string/i32/etc.), functions (name, min/max arity, return/param type
facts, host-sensitivity), predeclared objects (`Debug`, `Err`) with members. Decide how a
descriptor value lowers to `BoundExpr` (constants) vs a call (functions) vs a member route
(objects). Write it down; review before coding.

**Phase 1 (B) — consolidate.** Build `vba_library()` descriptor; route `parse_expr`/
intrinsic resolution/`Debug`/`Err` through it; **delete** `intrinsic_vb_constant`, fold the
`intrinsic_spec` registry and the `Debug`/`Err` special-cases into the descriptor. Re-run the
full compiler suite (861+) and the external corpus; keep the F1 regression tests green
(now asserting resolution via the library model).

**Phase 2 (C) — promote to injected reference.** Represent the library descriptor as a
built-in `HostInjected` reference; inject it by default into every manifest; route resolution
through the reference path (local → VBA library → host). Cache the built-in blob.

**Phase 3 — host extension seam.** Let a host register an additional library reference
(host globals). Demonstrate with a trivial host global (e.g. a stub `Application`) resolved
purely via the injected reference, no compiler edit.

**Phase 4 — coverage + cleanup.** Expand constant/function coverage as needed by the corpus;
assert the absence of the deleted special-case surfaces; update docs.

## Risks / trade-offs

- **Hot-path cost:** the built-in blob must be built once and cached; a per-compile rebuild
  would regress compile time. Mitigation: `OnceLock` the descriptor.
- **Schema scope creep:** representing predeclared objects (`Debug`/`Err`) is more than
  constants. Mitigation: Phase 0 pins the schema against exactly today's consumers; defer
  objects to a sub-phase if needed (constants + functions first, then `Debug`/`Err`).
- **Resolution-order regressions:** introducing a default reference changes name lookup.
  Mitigation: explicit, tested order (local project shadows library shadows host? — confirm
  VBA semantics: user code can shadow library names) and a regression for shadowing.
- **Coupling to COM blob shape:** the COM metadata blob is interface-centric; forcing
  constants into it may distort it. Mitigation: a distinct library-descriptor type in
  Phase B; only unify at the *resolution* layer in C, not the storage layer.

## Open questions / decision points

1. **Shadowing:** can user-defined names shadow VBA-library names (e.g. a local `Len`)?
   Confirm against real VBA and encode the resolution order accordingly.
2. **Schema home:** does the library descriptor live in `oxvba-compiler` (compile-time) or
   `oxvba-com`/a new crate (so host + compiler share it)? Leaning compiler-owned in B,
   reconsider for C if the host needs to read it.
3. **`Debug`/`Err` representation:** model as predeclared objects with member descriptors,
   or keep as lowering special-cases that the descriptor merely *declares* exist? (Affects
   how much of Phase 1 deletes vs. re-routes.)
4. **Dialect versioning:** one VBA 7.1 library, or a version axis alongside the
   conditional-compilation target? (Likely one, matching the VBA7 predefinition.)
5. **Conditional-compilation constants:** confirm they stay in `builtin_pp_constants` and are
   *not* folded into the library (they are a different phase). Recommended: yes, separate.

## Test strategy

- Re-run the full `oxvba-compiler` suite and the external corpus after each phase.
- Keep F1's `resolve_intrinsic_vb_constants_to_literals` green, re-pointed at the library
  model.
- Add: shadowing regression; host-extension resolution (Phase 3) with no compiler edit;
  absence assertions for the deleted special-case surfaces (Phase 4).
- Differential spot-checks against real VBA for any constant/function whose value or
  host-sensitivity is uncertain.
