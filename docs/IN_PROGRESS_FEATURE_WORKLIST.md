# In-Progress Capability Register

Date: 2026-07-10
Status: current profile-level register
Authority: derived from the system contract, current architecture, 2026-07-10 review and three current worksets

## Purpose

This register answers which major capability profiles remain in progress and where their delivery is owned. Detailed subset truth belongs in the canonical matrices required by each workset; historical IP/MACH ladder narration is not maintained here.

## Current profiles

### CORE-TOOLCHAIN — Core VBA compiler and dual runtimes

Status: `in-progress`
Profile: `PROFILE-CORE-001`
Owner: [`worksets/WORKSET_2026-07-10_POST_JIT_CORE_CONFORMANCE_AND_READINESS.md`](worksets/WORKSET_2026-07-10_POST_JIT_CORE_CONFORMANCE_AND_READINESS.md)

Major open outcomes:

- fail-closed, provenance-aware source/compiler analysis;
- complete typed calls, project references and diagnostics;
- sealed verified OxIR/OxImage artifact;
- sound shared runtime/helper/session ownership;
- member-complete VBA base library;
- complete VM3 verified-OxIR execution;
- ideal JIT lowering/calling/error/recursion architecture;
- persistent JIT sessions/cache;
- structural VM3/JIT and current Excel/VBA evidence;
- green ordinary/safety/governance gates.

Closure: all required core contract clauses and matrix rows are green for the declared Linux/Windows-x64/64-bit-Excel target.

### WINDOWS-INTEROP — Windows VBA/COM/native compatibility

Status: `in-progress`
Profile: `PROFILE-WIN-001`
Owner: [`worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md`](worksets/WORKSET_2026-07-10_JIT_WINDOWS_COM_NATIVE_INTEROP_AND_BINARY_EXPORT.md)

Major open outcomes:

- authoritative typelib/reference service;
- one verified VM3/JIT interop plan;
- JIT late/early COM client;
- synchronous connection-point events;
- VM3/JIT late and early/dual serving with outgoing events;
- JIT Declare, pointers and callbacks;
- x64 and actual 64-bit Excel certification;
- native-boundary safety/lifecycle.

Closure: every mandatory Windows compatibility row works under both VM3 and JIT with controlled and real Excel/native evidence.

### NATIVE-TOOLING — Wrapped and genuine native outputs

Status: `in-progress`
Profile: Windows portion of `PROFILE-TOOL-001`
Owner: Windows interop workset, epics WIN-11/WIN-12

Major open outcomes:

- JIT-backed wrapper EXE/library/COM server;
- loader-lock-safe initialization;
- explicit native export manifest and external ABI;
- genuine x64 DLL and EXE outputs;
- native clients, relocation/ASLR, debug maps and clean deployment.

Closure: wrapper and native output classes pass separate honest artifact gates.

### IDE-FOUNDATION — Compiler-backed language services

Status: `not implemented on clean stack`
Profile: `PROFILE-IDE-001`
Owner: [`worksets/WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md`](worksets/WORKSET_2026-07-10_LANGUAGE_SERVICES_CLEAN_STACK_BASELINE.md)

Major open outcomes:

- compiler AnalysisResult fact stream;
- immutable semantic snapshots and real workspaces;
- basic direct semantic query API;
- complete source/OxImage/library/host/COM/Declare/generated reference coverage;
- thin pinned LSP projection;
- embedded-host and editor smoke paths;
- invalidation, cancellation, performance and lifecycle evidence.

Closure: every basic direct/LSP/reference row is green and no current surface claims the deleted service.

### EXTENDED-PROFILES — Forms, debugging, security and additional platforms

Status: explicit future/inactive profiles
Profile: `PROFILE-EXT-001`
Owner: no accepted current delivery workset

These areas are not implicitly complete and do not silently block the three current profiles. Promotion requires an accepted workset tied to the relevant system clauses.

## Historical register migration

The former March 2026 IP-01..IP-11/IP-08B narrative and MACH-1000 gate state are historical provenance. Their valid implementation and evidence remain in code, matrices and evidence artifacts; unfinished residuals must be imported into the current worksets before they are active delivery truth.

See [`spec/DEPRECATION_LEDGER_2026-07-10.md`](spec/DEPRECATION_LEDGER_2026-07-10.md).
