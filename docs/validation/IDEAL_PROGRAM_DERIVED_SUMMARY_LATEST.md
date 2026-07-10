# Ideal Program Derived Validation Summary

Program: `ideal-2026-07` / `bd-59co`
Manifest: `docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json`
Ownership: `docs/validation/IDEAL_MATRIX_OWNERSHIP_V1.csv`

This file is generated from the manifest-owned canonical matrices. It is a projection, not an independent capability claim.

## Profile totals

| Profile | Matrices | Rows | Planned | In progress | Implemented subset | Implemented full | Verified | Archived |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| core | 5 | 3 | 3 | 0 | 0 | 0 | 0 | 0 |
| windows-x64 | 6 | 8 | 8 | 0 | 0 | 0 | 0 | 0 |
| ide | 4 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |

## Matrix totals

| Matrix | Profile | Role | Owner epic | Rows | Verified | Open | Trace relationships |
|---|---|---|---|---:|---:|---:|---:|
| CORE-READINESS | core | primary | bd-59co.2.1 | 3 | 0 | 3 | 13 |
| VBA-LIBRARY | core | primary | bd-59co.2.5 | 0 | 0 | 0 | 3 |
| OXIR-BACKENDS | core | primary | bd-59co.2.6 | 0 | 0 | 0 | 5 |
| OXIMAGE-CONTRACT | core | primary | bd-59co.2.6 | 0 | 0 | 0 | 4 |
| EXCEL-ORACLE | core | evidence | bd-59co.2.11 | 0 | 0 | 0 | 4 |
| WIN-COM-CLIENT | windows-x64 | primary | bd-59co.3.4 | 2 | 0 | 2 | 7 |
| WIN-COM-EVENTS | windows-x64 | primary | bd-59co.3.6 | 2 | 0 | 2 | 8 |
| WIN-COM-SERVER | windows-x64 | primary | bd-59co.3.7 | 0 | 0 | 0 | 6 |
| WIN-NATIVE-IMPORT | windows-x64 | primary | bd-59co.3.10 | 2 | 0 | 2 | 8 |
| WIN-NATIVE-EXPORT | windows-x64 | primary | bd-59co.3.13 | 2 | 0 | 2 | 7 |
| WIN-ABI-CARRIER | windows-x64 | quality | bd-59co.3.2 | 0 | 0 | 0 | 6 |
| LS-BASELINE | ide | primary | bd-59co.4.1 | 0 | 0 | 0 | 9 |
| LS-REFERENCES | ide | primary | bd-59co.4.7 | 0 | 0 | 0 | 4 |
| LSP-METHODS | ide | projection | bd-59co.4.11 | 0 | 0 | 0 | 5 |
| LS-PERFORMANCE | ide | quality | bd-59co.4.10 | 0 | 0 | 0 | 4 |

## Remaining accepted scope

| Row | Matrix | Capability | Subset | Truth state | Residual disposition | Residual owner |
|---|---|---|---|---|---|---|
| CORE-BASELINE-UNSAFE-CLIPPY | CORE-READINESS | strict clean-build unsafe audit | SafeArray and VBA record unsafe statements | planned | remaining-accepted-scope | bd-2cjy |
| CORE-DIFF-SEMANTICS-FUZZ | CORE-READINESS | structural VM3 JIT differential fuzzing | scalar Variant control-flow call and error hazards | planned | remaining-accepted-scope | bd-h4oh.8 |
| CORE-COMP-NUMERIC-MODE | CORE-READINESS | VBA-compatible NumericMode selection | provable fixed numeric lanes and overflow coercion | planned | remaining-accepted-scope | bd-h4oh.17 |
| WCC-PLAN-LATE | WIN-COM-CLIENT | late-bound COM client | scalar activation invocation and property access | planned | remaining-accepted-scope | bd-59co.3.4 |
| WCC-PLAN-EARLY | WIN-COM-CLIENT | early-bound COM client | scalar typed native-vtable invocation | planned | remaining-accepted-scope | bd-59co.3.5 |
| WCE-PLAN-INCOMING | WIN-COM-EVENTS | incoming COM events | synchronous cancellable scalar ByRef event | planned | remaining-accepted-scope | bd-aprs.8.8.9 |
| WCE-PLAN-OUTGOING | WIN-COM-EVENTS | outgoing COM events | served source-interface event fan-out | planned | remaining-accepted-scope | bd-59co.3.9 |
| WNI-PLAN-DECLARE | WIN-NATIVE-IMPORT | VBA7 Declare import | scalar named-symbol call | planned | remaining-accepted-scope | bd-59co.3.10 |
| WNI-PLAN-CALLBACK | WIN-NATIVE-IMPORT | AddressOf and pointer helpers | typed callback lifetime and pointer carriers | planned | remaining-accepted-scope | bd-9sed.17 |
| WNE-PLAN-WRAPPED | WIN-NATIVE-EXPORT | JIT-backed wrapped outputs | wrapped DLL EXE and COM server package | planned | remaining-accepted-scope | bd-59co.3.12 |
| WNE-PLAN-NATIVE | WIN-NATIVE-EXPORT | genuine native DLL and EXE | scalar Cranelift object and native output slice | planned | remaining-accepted-scope | bd-59co.3.13 |
