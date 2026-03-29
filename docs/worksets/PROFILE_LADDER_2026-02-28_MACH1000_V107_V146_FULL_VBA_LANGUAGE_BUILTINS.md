# PROFILE LADDER: MACH1000 v107..v146 (Full VBA Language + Built-ins)

> Archived historical ladder/status surface.
> This file is retained as execution history for the old MACH1000 profile run, not as active truth.
> Current feature status and closure claims are governed by the validation matrices under `docs/validation/`.
> Do not treat the broad "full VBA language" wording here as an active completion claim.

Timestamp (UTC): 2026-02-28T23:35:00Z  
Owner: AutoRun execution lanes (continuous)  
Primary plan source: `MACH1000_PLAN.md`  
Execution mode: non-stop profile ladder until `v146` terminal gate is passed.

## Objective

Historical objective only. The live validation program now uses domain-specific matrices and ownership mapping to prevent broad closure language from obscuring subset gaps.

Drive OxVba from current executable subset to broad VBA-language and built-in surface closure, with:

- language semantic completion,
- type/coercion parity expansion,
- standard-library expansion,
- interop and oracle-conformance hardening,
- deferred formal foldback and performance stabilization.

## Ladder Range

- Start profile: `v107`
- End profile: `v146`
- Terminal gate: `v146`
- Active ladder file: `docs/worksets/PROFILE_LADDER_2026-02-28_MACH1000_V107_V146_FULL_VBA_LANGUAGE_BUILTINS.md`

## Execution Policies

- Keep continuous AutoRun behavior: implement -> docs -> checks -> commit -> push -> continue.
- Formal failures are non-blocking unless promoted explicitly.
- Remote Kani lanes run async via `scripts/run-formal-kani-remote.ps1`.
- Deferred-gate foldback checkpoints: `v116`, `v126`, `v138`, `v146`.
- Mark `implemented` only when executable evidence exists (not parser-only scaffolding).

## Wave A: Language Semantic Closure (`v107..v116`)

| Profile | Scope | Primary Evidence |
|---|---|---|
| `v107` | `With` full semantic closure (nested/member side effects). | Conformance fixtures + compiler/runtime tests |
| `v108` | `For Each` full semantics (arrays, collections, mutation behavior). | Conformance + oracle probes |
| `v109` | `Select Case` full clause semantics incl. string compare modes. | Conformance matrix |
| `v110` | Label/line-number `GoTo`/`GoSub`/diagnostic parity edge cases. | Conformance diagnostics corpus |
| `v111` | `On Error` mode/state machine completion. | Error-state conformance set |
| `v112` | `Resume`, `Resume <label>`, `Resume Next` exact behavior. | Resume behavior corpus |
| `v113` | `Err` object full surface lifecycle. | Runtime + conformance |
| `v114` | Optional/Named/ParamArray legality matrix completion. | Call-shape conformance |
| `v115` | UDT value semantics (copy/init/assignment/arrays of UDT). | Type and runtime tests |
| `v116` | Class/property/default-member ordering and side-effect parity. | Object semantics corpus + foldback |

## Wave B: Type and Coercion Parity (`v117..v126`)

| Profile | Scope | Primary Evidence |
|---|---|---|
| `v117` | Variant coercion matrix: `Null/Empty/Error` across operations. | Matrix fixtures + oracle topics |
| `v118` | `vbNullString` vs `""` semantic parity closure. | String semantics probes |
| `v119` | Date/Currency conversion, rounding, overflow semantics. | Numeric/date conformance |
| `v120` | Conversion closure: `CSng/CByte/CCur/CDec` parity. | Built-in conversion corpus |
| `v121` | `Set`/`Let` assignment legality and object-reference semantics. | Assignment conformance |
| `v122` | ByRef temporary coercion + copy-back rules. | Formal/unit + oracle topics |
| `v123` | `Option Base` interactions with `Array()` and declared arrays. | Array bounds corpus |
| `v124` | `ReDim Preserve` legality matrix across dimensions. | Array resize legality suite |
| `v125` | `Erase` semantics across fixed/dynamic/Variant arrays. | Erase conformance |
| `v126` | Introspection parity: `IsEmpty/IsNull/IsError/TypeOf...Is`. | Type introspection suite + foldback |

## Wave C: Built-in Library Expansion (`v127..v138`)

| Profile | Scope | Primary Evidence |
|---|---|---|
| `v127` | String expansion: `Space`, `String$`, `Chr/Chr$`, `Asc`, `StrConv`. | String built-in corpus |
| `v128` | `Format/Format$` core behavior matrix. | Formatting conformance |
| `v129` | Date/time built-ins: `Date/Time/Now/Timer/Year/Month/Day/Weekday/MonthName`. | Date/time corpus |
| `v130` | Random semantics: `Rnd/Randomize` seed/reproducibility. | Deterministic random probes |
| `v131` | Numeric extras: `Hex/Oct/Atn/Tan` and conversion edges. | Numeric built-in suite |
| `v132` | Financial expansion: `NPV/IRR/MIRR/Rate/NPer` (+ tolerance policy). | Financial fixture bank |
| `v133` | `Dir` stateful semantics and wildcard behavior. | Host-sensitive conformance |
| `v134` | File I/O library: `Open/Close/Input/Line Input/Print#/Write#/EOF/LOF/Seek/FreeFile`. | File-I/O integration suite |
| `v135` | Host capability policy hardening for `Shell/Environ/Dir`. | Host-policy checks |
| `v136` | `MsgBox/InputBox` host adapter + deterministic test mode. | Host adapter tests |
| `v137` | Runtime intrinsic surface reconciliation and gap closure. | Surface registry + meta checks |
| `v138` | Built-in conformance sweep and stabilization pass. | Full built-in conformance + foldback |

## Wave D: Interop, Oracle, Perf, Terminal (`v139..v146`)

| Profile | Scope | Primary Evidence |
|---|---|---|
| `v139` | `CreateObject/DispatchInvoke` marshalling parity (primitive/object/array/byref). | Interop fixtures |
| `v140` | Type-library signature import MVP (optional/default/byref mapping). | Interop signature tests |
| `v141` | Class lifecycle edge ordering under errors and ref transitions. | Lifecycle scenario corpus |
| `v142` | Default-member/object invocation parity expansion. | Object invocation conformance |
| `v143` | Oracle differential harness against real VBA for uncertain semantics. | Conformance topic fold-in |
| `v144` | Deferred formal foldback from remote Kani lanes into latest reports. | `DEFERRED_GATES.md` + formal reports |
| `v145` | Performance pass for hot paths (string/array/dispatch/file I/O). | Bench artifacts |
| `v146` | Terminal integrated gate for full ladder scope. | Integrated gate + final profile status |

## Gate Cadence

- Per profile: compile/tests/conformance + docs/evidence update.
- Per wave end (`v116`, `v126`, `v138`): reconciliation checkpoint + deferred formal foldback.
- Final (`v146`): terminal integrated gate must pass before stopping.
