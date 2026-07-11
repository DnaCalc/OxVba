# WIN-14 Executable Rollout Acceptance

Date: 2026-07-11
Rollout bead: `bd-59co.3.15.1`
Status: accepted.

## Outcome and aggregate ownership

WIN-14 is no longer represented by one broad certification task. Its graph now
separates certification control tooling, fixture promotion, clean VM
provisioning/sealing, COM/client/event/serving lanes, Declare/callback lanes,
wrapped/genuine-native outputs, hostile lifecycle lanes, aggregate authorities,
immutable archive and final release certification.

WIN-14 owns only these aggregate/control rows:

| Row | Evidence/residual owner |
|---|---|
| `WAC-CLEAN-CERT-ENV` | `bd-59co.3.15.3` |
| `WCC-EXCEL-AUTHORITY` | `bd-59co.3.15.32` |
| `WAC-EXCEL-COM-CERT` | `bd-59co.3.15.32` |
| `WAC-EXCEL-NATIVE-CERT` | `bd-59co.3.15.33` |
| `WAC-RELEASE-CERT` | `bd-59co.3.15.2` |

Producer rows remain owned by WIN-1 through WIN-13. Certification leaves add
evidence and cannot steal producer ownership or change expected behavior to the
current implementation.

## Preparation and environment leaves

| IDs | Outcomes | Primary commands/artifacts |
|---|---|---|
| `.4` | complete 57-row certification case/evidence manifest | `validate-win14-certification-manifest.ps1`; `certification-cases.json` |
| `.5` | certification-plan/seal extension of WIN-0's single capture tool | capture self-tests; `environment-capture-contract.md` |
| `.6` | owned Excel/VBE oracle supervisor | `run-excel-vba-oracle.ps1 -Suite HarnessSelfTest`; `oracle-supervisor.md` |
| `.7` | resumable serialized Windows profile orchestrator | `run-windows-profile-gates.ps1 -List -ValidateOnly`; `profile-runner.md` |
| `.8` | frozen controlled fixture inventory | fixture manifest validation; `fixtures-manifest.json` |
| `.9` | built/self-tested COM carrier/event/serving fixtures | COM build lane; `fixtures/com/manifest.json` |
| `.10` | built/self-tested Declare/pointer/callback fixtures | native-import build lane; `fixtures/native-import/manifest.json` |
| `.11` | build-only external wrapper/genuine-output clients and negative validators | output-client build lane; `fixtures/native-output-clients/manifest.json` |
| `.12` | provider-neutral Windows x64/Excel64 VM provisioning and candidate identity | VM provision plan/apply; `vm-provision.md` |
| `.13` | Excel64, locale, VBOM and UIA qualification | capture/oracle qualification; `vm-qualification.md` |
| `.14` | three clean reset/deploy/cleanup cycles | `verify-win14-clean-reset.ps1 -Cycles 3`; `clean-reset.md` |
| `.3` | atomic environment seal and provisional-ID replacement | capture `-Seal` plus environment/reset validators; `certification-vm.md` |

WIN-0 owns the generic environment-capture producer. WIN-14 extends and consumes
that interface; it does not create a second environment authority. Provisioning
leaf `.12` records a candidate identity only. The provisional canonical ID is
replaced atomically by `.3` only after qualification and reset proof.

The `.11` fixture lane intentionally has no dependency on the whole WIN-11 or
WIN-12 product epics: it can build clients, negative validators and placeholder
fixtures early, but it cannot execute or credit wrapped or genuine-native
product capability. Product validation remains in `.27` through `.29`.

## COM certification leaves

| IDs | Outcomes | Aggregate route |
|---|---|---|
| `.15` | Excel64 compiler/reference/typelib authority | `WCC-EXCEL-AUTHORITY` |
| `.16` | exact carriers and non-default-locale roundtrip | `WAC-EXCEL-COM-CERT` evidence |
| `.17` / `.18` | late dispatch / early native-vtable clients | COM aggregate; early lane proves zero dispatch fallback |
| `.19` / `.20` | incoming functional/ByRef / apartment-reentry-lifecycle events | COM aggregate; ByRef writeback before native return |
| `.21` / `.22` | late in/out-proc / early-dual-custom serving | COM aggregate; plan equality and no fallback |
| `.23` | outgoing served events and Excel `WithEvents` | COM aggregate |
| `.30` | hostile/fault/repeated COM lifecycles | COM aggregate safety evidence |
| `.32` | aggregate Excel64 COM authority/certification | owns `WCC-EXCEL-AUTHORITY` and `WAC-EXCEL-COM-CERT` completion |

The traceability ledger records all 74 named, aggregate and necessarily covered
bead-row relationships for `.15` through `.31`. Exact producer-row links use
`evidences` and retain their WIN-1 through WIN-13 residual owners; rollout and
oracle support links likewise do not own aggregate delivery rows.

Each lane runs through the profile runner and, where applicable, the owned
Excel/VBA supervisor. It records structural result, complete Err, side effects,
event/lifecycle order, actual transport counters/signatures and all resource
balances.

## Native and output certification leaves

| IDs | Outcomes | Aggregate route |
|---|---|---|
| `.24` | PtrSafe Declare scalar/string/structural/loader/error behavior | native aggregate |
| `.25` / `.26` | pointer+synchronous / retained+nested+disposed callbacks | native aggregate |
| `.27` | wrapped executable/library/COM-server clients | native aggregate with honest wrapped labels |
| `.28` | genuine x64 PE DLL/EXE clients | native aggregate with honest genuine-native labels |
| `.29` | reproducibility policy, ASLR, debug maps and clean deployment | native aggregate |
| `.31` | hostile/fault/stale/repeated native lifecycles | native aggregate safety evidence |
| `.33` | aggregate Excel64 native import/output certification | owns `WAC-EXCEL-NATIVE-CERT` completion |

## Archive and final release

`bd-59co.3.15.34` verifies an immutable replay index over every environment,
source, fixture, product, artifact, log, VBE/UIA and cleanup transcript.
`bd-59co.3.15.2` is reclassified as delivery/conformance and now certifies only
the final `WAC-RELEASE-CERT` aggregate after `.3`, `.32`, `.33`, `.34` and all
WIN-0 through WIN-13 producer epics are green. It cannot credit
`WNE-PROFILE-TOOL-TERMINAL` or `WAC-PROFILE-TERMINAL`; WIN-15 owns them.
The Windows profile has 57 rows: WIN-14 first requires the other 54
pre-terminal rows, then verifies `WAC-RELEASE-CERT`, leaving the two WIN-15
terminal rows downstream (55 of 57 verified at the WIN-14 boundary).

Every leaf is at most 480 minutes and has explicit clauses, matrix/truth route,
dependencies, resource serialization labels, commands, artifact, six-axis
acceptance and blocker/residual behavior. The only immediately executable
post-rollout preparation leaf is `.4`; later preparation becomes ready only as
its WIN-0, tool, fixture and VM prerequisites close. Product certification
leaves remain blocked on both the sealed environment and exact producer epics.

Unavailable VM/Office/licensing/provider state is an explicit blocker. The dev
host and historical evidence never substitute for the sealed certification VM.
Excel modal automation is PID-scoped and registry/file/process cleanup is
owned-only.

## Acceptance record

The final independent WIN-14 review is clean. It rechecked the 54-row
pre-terminal count, all carrier aliases, evidence-only rollout/oracle handoffs,
74 named/necessary certification relationships, retained producer owners, the
build-only `.11` lane and sole canonical environment replacement by `.3`.
WIN-14 is now explicitly a non-owning consumer of the COM client/event/serving,
native-import and profile-tool contracts in the workset, epic, disposition and
trace ledgers. Truth reconciliation passes at 193 rows, 370 relationships and
125 leaves. Governance, path stability, docs, lint, cycles and all 24 negative
validator cases pass.
