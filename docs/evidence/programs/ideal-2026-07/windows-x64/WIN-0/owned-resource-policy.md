# WIN-0 Owned Windows Test Resource Policy Evidence

Date: 2026-07-14
Bead: `bd-59co.3.1.5`
Status: support-safety policy and safe synthetic proof complete
Owned/advanced clause: `CONF-MATRIX-001`
Parent/profile context (not advanced): `PROFILE-WIN-001`
Downstream informed constraints (not advanced): `SEC-BOUNDARY-001`, `CONF-QUALITY-001`
Capability credit: none

## Outcome and boundary

`docs/spec/OXVBA_WINDOWS_TEST_OWNERSHIP_POLICY_V1.md` defines the normative V1
ownership contract for Windows test mutations. The executable reference and
its acceptance suite are:

- `scripts/lib-windows-owned-resource-policy.ps1`; and
- `scripts/test-windows-owned-resource-policy.ps1`.

The final repair binds every journal transaction to one path-stable named mutex,
the exact validated journal object, its canonical history digest, its immutable
identity/allowlist digest, and an explicit mutation ticket. Journal temporary
files are written, flushed, published, and on failure deleted through the same
retained native handle. Registry markers are set and flushed on the exact
`HKEY` returned by `RegCreateKeyExW`; files record the full modern
`FILE_ID_INFO`; and process cleanup uses one retained native handle from
creation-time/path verification through terminate/wait. Cleanup remains
exact-resource, reverse-order, resumable, nonrecursive, and fail-closed.

This bead owns and advances only `CONF-MATRIX-001`. `PROFILE-WIN-001` is parent
profile context; `SEC-BOUNDARY-001` and `CONF-QUALITY-001` are downstream
constraints informed by this work. This result does not advance those three
clauses and does not implement or certify any Windows capability row.

The suite mutates only a unique synthetic envelope:

- exact values/keys in HKCU Registry64 below unique namespaces;
- create-only files below one unique system-temporary root; and
- exact recorded harmless hidden child `pwsh` processes with bounded
  self-timeouts.

Apartment, reentry, callback, connection, and dialog/UIA resources are logical
journal representations. The suite starts no Excel/VBE process, performs no
COM initialization or activation, calls no Advise/Unadvise, and invokes no UI
Automation.

## Immutable implementation identity

Implementation/test history:

- `c809b18812b51249c59596b943eb5488e22495f6`
  (`test(win0): enforce owned Windows resources`);
- `f82b5c46adad122aa48b72c6d63af538e0f5b48c`
  (`fix(win0): journal registry ancestor ownership`); and
- `0c021cfa567295c51f17dffa125183301ebe0872`
  (`fix(win0): serialize and prove Windows resource ownership`); and
- `9bdebb8f6f0ad19e5b8c10e55bf2c0efbb98a370`
  (`fix(win0): bind cleanup to exact resource identity`); and
- `116b34ea77afb0f08fef487a05422a9973d5b0d3`
  (`fix(win0): harden exact Windows ownership transactions`); and
- `da345b9375b8e59ae4dfffb9cfed907f4395adb2`
  (`fix(win0): preserve pending mutation authority`).

Prior normative/evidence repairs are
`1dbb7a7699dd2280eebcb2fb9db6912df36f5bc9` and
`6eb844b56ec53c29c189187b8fe68852815569f9`. The current normative successor is
`ac1d272e4c3843b6a0744308066193b38a1ffad3` and
`5da284ed7354741444167357ce666d6429af3c47`; this evidence update is
intentionally separate from implementation and specification commits.

| artifact | SHA-256 |
|---|---|
| `scripts/lib-windows-owned-resource-policy.ps1` | `bf935e7c0afeeabc26a56796a517300104a9cc875b193592a9eec95090c2b46e` |
| `scripts/test-windows-owned-resource-policy.ps1` | `ca48dab5bdde2a0a3605c0ef03b0052c1249f56dab98a594b55df9a3251e84fa` |
| `docs/spec/OXVBA_WINDOWS_TEST_OWNERSHIP_POLICY_V1.md` before this evidence update | `05a2b09ebd49bc87d75b5a49999d7d2beada4a79475ed475fd6dbdf6432ce9cf` |

The journal schema remains
`oxvba-windows-owned-resource-journal-v1`, version `1`. The root now explicitly
binds `registry_view` to exact `Registry64`; registry descriptors carry their
own matching view and per-key ownership records. File descriptors carry a
pending/created-owned disposition plus the creation-handle 64-bit volume serial
and 128-bit file ID once ownership is proven.

## Final validation record

Command:

```powershell
pwsh -NoProfile -File ./scripts/test-windows-owned-resource-policy.ps1
```

Final result after the fresh-eyes hardening pass:

```text
PASS: Windows owned-resource policy (81 assertions; 65 fail-closed mutations; real HKCU/file/child; logical COM/UIA only; exact teardown verified)
```

The final run completed in 361.251 seconds on the Windows x64 development host.
An earlier independent controller run of the predecessor revision passed with
69 assertions and 53 fail-closed probes. That historical pass is retained as
independent review evidence, not substituted for the final implementation run.

During repair, one synthetic resume fixture used a noncanonical cleanup-start
detail and was correctly rejected. Two diagnostic stress runs then proved that
24 simultaneous native durable writers exceeded fixture waits and, after gate
release, the unchanged 120-second product lease-acquisition bound; each failing
run preserved prerequisites and completed exact teardown. The final fixture
keeps all product bounds unchanged, uses 12 real contending writer processes,
and gives only the test-local post-activation gate its own bounded wait.

| check | result |
|---|---|
| PowerShell AST parse of both scripts | pass |
| `git diff --check` | pass |
| full owned-resource acceptance suite | pass, 81 assertions / 65 rejections |
| completed run's exact temporary root | absent after validated teardown |
| completed run's exact Registry64 namespace/values | absent after validated teardown |
| completed run's recorded writer/abandon/loop child processes | zero |

## Whole-transaction lease and race proof

The named mutex is derived from the lowercase canonical journal path and its
SHA-256 digest:

```text
Local\OxVba.WindowsOwnedJournal.<digest>
```

Acquisition is bounded at 120 seconds. The in-process lease registry binds the
actual token object, journal path/name, PID, managed thread, acquired state,
exact validated journal object, canonical content digest, immutable
identity/allowlist digest, and pending mutation ticket. A fabricated token,
separate reread object, modified bound object, allowlist expansion, publication
without a ticket, revalidation attempting to discard a pending ticket, and
concurrently replaced signed history are rejected. An
abandoned mutex is mutation-disabled until the complete existing journal is
strictly reread and validated; an absent new run must prove both immutable paths
absent after parent/reparse validation before infrastructure creation.

Each normal mutation holds one lease across validation, durable `prepared`, the
real mutation, durable `active`, and release. Cleanup holds the same lease
across `cleaning`, every reverse inverse and terminal resource write, and final
`completed`/`cleanup-conflict`. Nested helpers validate the exact live lease;
the cleanup race also proves balanced same-thread recursive acquisition.

The deterministic contention scenario launches 12 exact recorded harmless
children. All wait on one journaled gate, then contend to create 12 unique
journaled target files. The suite proves:

- all 12 exact children exit within their contract;
- exactly 12 unique target resources and files exist;
- every resource sequence is gap-free with no overwritten/lost record; and
- no adjacent `.write-*` journal temporary remains.

A 13th recorded child waits on a second gate. The owner retains the same
whole-transaction lease while opening that gate and invoking cleanup. The
writer cannot cross its prepare/mutate boundary; cleanup stops only its exact
PID/start/executable and completes with no target record, target file, child,
or temporary residue.

A separate recorded child acquires the mutex and exits without releasing it.
The next acquisition observes `AbandonedMutexException`; a journal write is
rejected before revalidation, then strict revalidation enables safe cleanup.
The suite finally proves the per-process live-lease registry is empty.

The journal-publication probe retains exact target and replacement handles,
proves their paths cannot be deleted/swapped while delete sharing is denied,
proves a replacement over the held destination fails, and cleans both exact
objects through handle dispositions. A closed-handle cleanup returns a nonzero
error, and the production path surfaces that cleanup failure. The native write
decision also maps successful zero-byte progress and failure-without-last-error
to explicit nonzero error `1117`.

## Strict journal and reparse boundary

Raw `System.Text.Json` validation runs before `ConvertFrom-Json`. Every root,
orchestrator apartment, resource, descriptor, snapshot, registry key-ownership
record, and event has an exact case-sensitive schema. Booleans must be JSON
booleans; Int32/Int64 fields must be integral JSON numbers in range; arrays must
be arrays with declared item kinds; unknown/case-drifted/duplicate properties
are rejected.

Semantic validation then cross-checks canonical roots and identities, exact
Registry64 spelling, gap-free counters, digest, earlier apartment/callback/
process dependencies, registry disposition prefixes, state/timestamp/event
agreement, and descending cleanup outcomes including conflicts. Resigned
journals with a later apartment dependency, missing lifecycle event, forward
cleanup order, or root-state rollback are rejected.

Repository and temporary caller roots, existing journal/run infrastructure,
the unique run root, journal parent/file, and confined paths are checked for
reparse points. Checks repeat after lease acquisition and at journal move, file
create/delete, and child-start boundaries. The suite rejects a repository-root
junction, temporary-root junction, journal-infrastructure junction, run-root
swap to a junction, and confined traversal junction without mutating through
them.

Admission requires a drive-qualified path on the ready local fixed NTFS/ReFS
volume. The suite positively records that volume shape and rejects `C:foo`
drive-relative syntax, alternate data streams, UNC, extended/device namespaces,
and reserved device components before resource mutation.

Adjacent journal temporaries are created with write/delete rights and retained
through native write, flush, repeated boundary validation, and
`SetFileInformationByHandle` rename. Failure cleanup uses disposition on that
same handle; no path-selected temporary deletion remains.

The SHA-256 journal digest detects truncation, accidental modification, and
non-recomputed tampering. It is deliberately non-hostile integrity evidence,
not a MAC or hostile same-user authorization boundary.

## Registry64 disposition and marker proof

All production registry access is explicit HKCU Registry64 and requires a
64-bit process on 64-bit Windows. Managed access opens the Registry64 base;
native creation/deletion uses `RegCreateKeyExW`/`RegDeleteKeyExW` with
`KEY_WOW64_64KEY`.

Before key creation, the prepared descriptor records every planned key with
`creation_disposition=pending` and a unique exact marker name/token. The helper
creates one key at a time and trusts only the Win32 disposition. For a created
key, the exact `HKEY` returned by `RegCreateKeyExW` remains open through
`RegSetValueExW` and `RegFlushKey`; marker publication never reopens the path:

- created-new: write and flush the exact string marker, then durably record
  `created-owned`;
- opened-existing: write no marker, durably record `opened-existing`, and
  preserve the key; and
- any other result: stop fail-closed.

The exact value mutation refuses implicit key creation. Cleanup restores the
value first, then considers keys deepest-first. It deletes only a pending or
created-owned key whose exact string marker/token is present, is its only
value, and has no subkeys. The marker remains inside the key until
`RegDeleteKeyExW`; it is never removed separately.

| registry scenario | observed cleanup result |
|---|---|
| crash before key create | no key and no inferred ownership |
| prepared record, external empty key appears | preserve and conflict: no marker proof |
| exact markers written, descriptor outcomes still pending | token proof removes the empty chain |
| another actor wins every creation disposition | record opened-existing, roll back only value, preserve keys |
| owned key deleted and same path recreated without marker | preserve replacement and parent sentinel as a conflict |
| normal created-owned value/key rollback | restore value and remove exact marker-owned chain |
| value already rolled back before key deletion | resume through intact markers and remove exact chain |
| pre-existing key with neighbor sentinel | remove owned value only; preserve key and sentinel exactly |
| marker-owned ancestor becomes populated | preserve neighbor and ancestor as conflict; later exact retry succeeds |

A process failure after native key creation but before marker flush remains an
unprovable blocking ambiguity; retaining the returned `HKEY` removes the former
path-reopen/replacement interval, not that unavoidable crash point. In
contrast, marker-before-descriptor is provable because the unique marker token
was durable in the prepared record before creation. The checksum/token model is
cooperative: it does not authenticate against a malicious same-user actor that
copies a token, and it cannot make the external proof-to-delete interval
transactional.

## Exact rollback, stale recovery, and unrelated drift

Files use exact absent-to-create-only snapshots and `FileMode.CreateNew`.
The creation handle supplies modern `FILE_ID_INFO`—the full 64-bit volume
serial and 128-bit file ID—before the durable created-owned descriptor update.
Cleanup opens without write/delete sharing, holds that handle across ID and
content verification, and uses handle-based disposition for deletion. Changed
bytes and same-content replacement files with a different identity become
conflicts and remain untouched. The replacement regression holds the deleted
original open while recreating the same path/bytes, making the identity
difference deterministic and non-vacuous. This is a local NTFS/ReFS cooperative
instance guarantee; IDs can be reused after deletion, and no hostile same-user
replacement/authentication guarantee is claimed.

Child cleanup opens one native process handle with query, terminate, and
synchronize rights and retains it through creation-time comparison, executable
path comparison, exact `TerminateProcess`, wait, and close. Missing/PID-reused
outcomes are already gone and never query a path; inaccessible identity fails
closed. The inert activation protocol makes PID/start durable before the
separately journaled activation file exists. A real recorded parent launches an
unrecorded bounded descendant; exact parent cleanup leaves that descendant
live, and the test only observes it until its own natural exit. No process-name,
command-line, descendant, or tree discovery is used for cleanup.

Logical acquisition is apartment, callback, connection, and process-scoped
dialog; validated dependencies require earlier exact resources. Reverse
cleanup records connection unadvise representation before callback retirement
and apartment retirement. These are logical records only, not real COM/UIA
operations.

The stale-owner scenario has an exact recorded child create its own journal and
file, then exit without cleanup. Ordinary non-owner cleanup is rejected. Stale
recovery requires exact owner PID/start mismatch, removes only that recorded
file, reaches `completed`, and is byte-idempotent on retry. Recovery refuses a
still-live exact owner.

A separate journal is durably placed in `cleaning` with one canonical
`cleanup-started` event and two active files. One inverse is then applied before
its terminal event to model a crash. Cleanup resumes that exact cycle, records
the missing inverse as `already-before`, deletes the other exact instance,
emits no second start event, and reaches `completed`.

Four sentinels—neighbor Registry64 value, neighboring file hash, current
unowned PowerShell PID/start, and logical-object digest—remain exact after main,
conflict, stale, race, registry-crash, and final phases. No recursive registry
or file cleanup and no global process/window cleanup is present.

The outer teardown retries exact cleanup but never swallows its result. It
collects only validated completed journals with empty run roots for deletion.
Any validation error, cleanup conflict, residual entry, or non-completed state
is returned as a failing diagnostic while the journal and recovery root remain.

## Fail-closed coverage

The 65 negative probes cover:

- caller-root/infrastructure/run/confined reparse paths, path escape, wildcard,
  controlled roots, drive-relative, ADS, UNC, namespace/device, and reserved
  path forms;
- fabricated/unrevalidated leases, stale reread objects, immutable allowlist
  edits, concurrent signed history, missing mutation tickets, duplicate run ID,
  live-owner recovery, and normal cleanup by a dead owner's non-owner;
- HKLM, broad category, non-allowlisted Registry64 key, wildcard value,
  external empty/same-path replacement keys, and populated ancestor;
- blanket process, dialog-class, registry-subtree, and recursive-file cleanup;
- unrecorded, PID-reused, inaccessible, or executable-mismatched process
  identity, apartment mismatch, missing or wildcard callback/connection/dialog
  identity, and invalid cookie;
- pre-existing files, prepared records followed by an external winner, both
  sides of the create/disposition crash interval, same-content different-file
  replacement, and changed owned-file content;
- string boolean/number coercion, scalar array, property/view case drift,
  unknown/duplicate property, digest/root identity tamper, later dependency,
  missing event, forward cleanup, and root/lifecycle inconsistency.

Every rejection is asserted before the forbidden mutation or preserves the
ambiguous/drifted resource as an explicit cleanup conflict.

## Operational residue reconciliation

The final passing run removed its own exact temporary root, Registry64
namespace, journaled files, and recorded processes. Repair diagnostics had also
preserved seven earlier full-ID roots, as required by the failing teardown:

| test ID | recorded owner PID | exact reconciliation |
|---|---:|---|
| `a29a8c7d3d5c45b19ae6d91d39bdcc6b` | 23564 | malformed-publication journal, zero resources, empty run root |
| `703f192bf91b443cadb35917ff6a2724` | 8 | malformed-publication journal, zero resources, empty run root |
| `7d6960ac83b846ebaced2186e155199f` | 436 | malformed-publication journal, zero resources, empty run root |
| `8ec35cb58fad495994e44bf970babd11` | 9936 | malformed-publication journal, zero resources, empty run root |
| `bc82aa0e68434be1a7c5455012a16fb8` | 5396 | four valid open journals completed through exact stale recovery; remaining journals/resources already completed |
| `ccd5b0ec686b41329773f2279bfbb37e` | 25996 | one pre-disposition file matched its exact recorded path, length and SHA-256; that inverse was applied and its journal completed through stale recovery |
| `da039aa0f3134e3999b00e8345aa5477` | 3940 | one synthetic noncanonical-cycle journal: missing file already absent; remaining file matched modern volume/file ID, length and SHA-256 and was removed through exact-handle cleanup |

For every row, the recorded owner PID/start was exact-dead, no process had a
recorded-owner parent PID, and no live command line referenced the root. All
run roots were empty after the exact recoveries. For the last three roots, the
only non-journal neighbor state was verified exactly as
`neighbor-sentinel.txt = neighbor-file-<test-id>` and the Registry64 value
`neighbor-<test-id> = neighbor-value-<test-id>` below
`HKCU\Software\OxVbaOwnedResourcePolicy\<test-id>`. The values/keys, journal
files, empty leaf/infrastructure directories, and roots were then removed only
by exact path/name, bottom-up and nonrecursively. Post-check reported zero
`$env:TEMP\oxvba-owned-policy-test-*` roots and zero matching Registry64 keys.

Two still-earlier pre-ID test roots were interactively reconciled by the
controller before this final repair. Only prefix-unique identities
`34598cf…` and `d1db626…` remain available; the deleted full GUID suffixes and
transcript were not retained and are not reconstructed here. The controller
used prefix-unique exact enumeration under `$env:TEMP`, verified recorded
owners/children dead, found no remaining journaled payload, and found only each
root's unique neighbor sentinel, empty infrastructure, and matching exact
Registry64 sentinel. It removed the exact value/key and now-empty paths
nonrecursively and verified no matching temp root/key remained. This paragraph
is operational cleanup context, not certification evidence.

## Inherited governance gate

The current branch-local governance run passes `docs-check`,
`active-program-sync`, `divergences`, `deferred-oracle-gates`,
`pmr-followup-sync`, and `project-integration-catalog`, then stops at the
inherited branch-baseline issue:

```text
stale generated PMR diagnostic artifact: docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md
```

That generated controller artifact has no `.3.1.5` worktree diff and is outside
this bead's authorized scope. The primary integration branch was independently
reported to pass this PMR check. The controller must rerun full governance after
integrating these bounded commits; this evidence does not conceal or relabel
the inherited failure.

## Fresh-eyes review and residual boundary

Fresh-eyes review actively checked lost-writer races, recursive lease release,
abandoned state, stale/copied journal authority, mutation tickets, registry
view/disposition inference, create/marker crash windows, marker lifetime,
pre-existing/populated/replaced keys, reparse and journal-temp swaps, zero-write
progress, raw coercion, dependency order, cleanup-cycle resumption, handle
lifetime, descendant behavior, modern file identity, exact residue, and
destructive primitive call sites. The final hardening added immutable
lease/object/history binding, retained-handle publication/cleanup, retained-HKEY
marker flush, modern local file identity, one-handle native process cleanup,
nonrecursive descendant proof, exact cleaning-cycle resumption, fail-closed path
admission, and surfaced temporary cleanup errors before the final passing run.

Residual work is downstream. Real Excel/VBA, COM, UIA, native, VM3/JIT,
registration, and clean-VM drivers must adopt this policy (or prove an
equivalent exact contract) for their own resources. The marker checksum model
does not claim a hostile-machine trust boundary. A process failure after native
key creation but before marker flush and hostile same-user reuse after file
deletion remain explicit limits of the cooperative test boundary. This support
result does not close or advance any downstream capability lane.
