# OxVba Windows Test Resource Ownership Policy V1

Date: 2026-07-11
Status: normative support-safety policy
Owning bead: `bd-59co.3.1.5`
Owned/advanced clause: `CONF-MATRIX-001`
Parent/profile context (not advanced): `PROFILE-WIN-001`
Downstream informed constraints (not advanced): `SEC-BOUNDARY-001`, `CONF-QUALITY-001`
Capability credit: none

## 1. Purpose and authority

This policy governs mutable Windows resources created by OxVba test, oracle,
fixture, and evidence runs. Its objective is simple: a run may mutate only an
exact resource it owns, must record enough pre-mutation truth to undo that
mutation, and must leave unrelated machine state unchanged.

The executable reference implementation is
`scripts/lib-windows-owned-resource-policy.ps1`. Its safe synthetic acceptance
suite is `scripts/test-windows-owned-resource-policy.ps1`. A different harness
may conform only if it proves every MUST in this policy with equally exact
evidence. Convenience cleanup, machine-wide discovery, process-name cleanup,
window-class cleanup, recursive deletion, and wildcard deletion are not
equivalent implementations.

This is a support-only control. Passing it advances only the fixture-manifest
conformance obligation `CONF-MATRIX-001`; it does not advance the parent
Windows profile `PROFILE-WIN-001` or the downstream security/quality
constraints `SEC-BOUNDARY-001` and `CONF-QUALITY-001`. It does not implement or
certify COM, native import/export, VM3, JIT, Excel/VBA, or any canonical Windows
capability row, and it grants no such row capability credit.

## 2. Normative terms and ownership tuple

`MUST`, `MUST NOT`, `SHOULD`, and `MAY` are normative.

Every run MUST have an immutable run ID of the form
`oxvba-<UTC-basic-time>-<32 lowercase hexadecimal GUID digits>`. A collision
with an existing journal or run root MUST fail before reuse.

Ownership is not inferred from a friendly name. The minimum ownership tuple is:

| resource | exact ownership tuple |
|---|---|
| run | run ID, owner PID, owner process start UTC, journal path |
| file | canonical absolute path, before snapshot, expected length/SHA-256, creation disposition, and creation-handle volume/file ID |
| registry value | normalized HKCU Registry64 key path, exact value name, kind/bytes, deepest existing ancestor, and per-key Win32 disposition/marker-token records |
| process | resource ID, PID, process start UTC and executable path |
| dialog/UIA | process resource ID, PID/start, UIA runtime ID and native handle |
| apartment | owner process, managed thread, model, initialization owner, pump and reentry policy |
| callback | apartment, session, thunk, owning thread, retention and stale/wrong-thread rules |
| connection | apartment, callback, source, sink, IID, cookie and writeback policy |

A selector that omits any required identity field is unowned. A syntactically
exact selector is still unowned unless it matches one resource in a validated
journal.

## 3. Trust boundary and bootstrap envelope

The caller MUST supply existing canonical repository and temporary roots. The
reference helper binds the repository root to the repository containing the
helper itself; a journal cannot expand it even by recomputing the integrity
digest. The journal path independently binds the unique temporary root and run
ID. The reference helper creates only the deterministic journal and run-root
infrastructure beneath that caller-owned temporary root. This bootstrap
envelope consists of:

- `oxvba-owned-resource-journals/<run-id>.json`;
- `oxvba-owned-resource-runs/<run-id>`; and
- adjacent unique write-through journal temporary files used for atomic
  replacement.

No other mutation is allowed before the first durable journal image. The run
root is unique and collision-checked. Bootstrap infrastructure is not a license
to recursively clean the temporary root; completed journal files are durable
audit records unless an outer, independently owned test fixture removes their
exact paths.

The repository root, temporary root, existing journal/run infrastructure, run
root, journal parent/file, and confined resource path components MUST be
checked for reparse points before use. The check is repeated after the journal
transaction lease is acquired and at the immediately preceding operation
boundary for journal replacement, file creation/deletion, and child launch.
An existing junction at a caller root or infrastructure component is a
fail-closed error. Creating or swapping a junction does not enlarge ownership.

Paths are compared canonically and case-insensitively on Windows. A resource
file MUST be below the declared repository or temporary root, MUST NOT name a
controlled root, MUST NOT contain wildcard or traversal selectors, and MUST NOT
cross a reparse point. A parent directory MUST already exist. The reference
file mutation is create-only and refuses overwrite.

## 4. Durable journal contract

### 4.1 Root schema

The V1 journal is strict UTF-8 JSON with no duplicate properties. Its exact
root properties are:

`schema_id`, `schema_version`, `run_id`, `created_utc`, `updated_utc`,
`owner_pid`, `owner_process_start_utc`, `repository_root`, `temp_root`,
`run_root`, `journal_path`, `registry_view`, `allowed_registry_paths`,
`allowed_executable_paths`, `orchestrator_apartment`, `reentry_policy`,
`state`, `next_resource_sequence`, `next_event_sequence`, `resources`,
`events`, and `journal_digest`.

Unknown, missing, case-changed, or duplicate properties MUST fail validation.
The schema ID is `oxvba-windows-owned-resource-journal-v1`; the version is `1`.
Root states are `active`, `cleaning`, `cleanup-conflict`, and `completed`.
Resource states are `prepared`, `active`, `cleaned`, and `conflict`.

Validation MUST inspect raw `System.Text.Json` `JsonValueKind` values before
PowerShell object conversion. Every root, apartment, resource, descriptor,
snapshot, key-ownership record, and event has an exact case-sensitive property
set. Booleans MUST be JSON booleans, integers MUST be integral JSON numbers in
their declared Int32/Int64 ranges, arrays MUST be JSON arrays with the declared
item kind, and objects MUST not be accepted through scalar coercion. Dependency
references MUST name exactly one earlier resource of the required kind:
apartment before callback, matching apartment and callback before connection,
and process before dialog. Lifecycle events MUST agree with resource/root
states and timestamps, and each cleanup cycle—including conflicts—MUST proceed
in descending acquisition order.

`journal_digest` is SHA-256 over the fixed-order JSON-normalized journal payload
excluding the digest field. It detects truncation, accidental modification,
and non-recomputed tampering; it is an integrity checksum, not a substitute for
an authenticated hostile-machine boundary. A bad schema, duplicate property,
counter mismatch, identity mismatch, or digest mismatch MUST cause zero
resource mutation.

### 4.2 Per-journal transaction lease

Every journal mutation MUST hold one stable Windows named mutex derived from
the lowercase canonical journal path and its SHA-256 digest. The reference name
is `Local\OxVba.WindowsOwnedJournal.<digest>`. Acquisition is bounded; timeout
does not grant mutation authority.

One lease covers the entire transaction, not only the JSON write:

1. acquire the named mutex and validate/revalidate the journal;
2. write and durably flush the prepared state;
3. perform the exact real mutation;
4. write and durably flush the active state; and
5. release the mutex.

Cleanup likewise holds the same lease across the durable `cleaning` transition,
every reverse inverse and per-resource terminal write, and the final
`completed` or `cleanup-conflict` transition. Nested helpers MUST receive and
validate the exact live in-process lease object; a copied or fabricated token,
wrong journal, wrong process/thread, released lease, or abandoned lease pending
revalidation grants no mutation authority. Same-thread nested acquisition is
permitted only through the same named mutex and balanced release.

An abandoned mutex is acquired as an explicit recovery condition. For an
existing journal, the full strict schema, identity, digest, dependency, and
lifecycle validation MUST succeed before the lease becomes mutation-capable.
For a new immutable run, exact absence of both its journal and run root plus
reparse validation of every existing parent is the fail-closed revalidation
condition. No infrastructure or resource mutation may occur first.

### 4.3 Atomic durability

Each transition MUST be serialized to an adjacent unique file, flushed with
write-through semantics, and atomically moved over the journal. Resource and
event sequences are strictly increasing and gap-free.

Before a resource mutation, the journal MUST contain a `prepared` record with:

- an immutable resource ID and acquisition sequence;
- its exact descriptor;
- an exact `before` snapshot;
- an exact `expected` post-mutation snapshot; and
- a `resource-prepared` event.

Only after the mutation succeeds may the record become `active`. Cleanup MUST
handle both `prepared` and `active` records because a crash can occur between
the mutation and activation transition.

## 5. Resource-specific policy

### 5.1 Registry values

The reference evidence helper is HKCU-only and x64-only. It MUST run in a
64-bit process on 64-bit Windows. The root and every registry descriptor MUST
spell the view exactly `Registry64`; managed reads/writes use
`RegistryKey.OpenBaseKey(CurrentUser, Registry64)`, and native creation/deletion
uses `KEY_WOW64_64KEY`. Every key MUST be an exact normalized
`HKCU\Software\<namespace>\<identity>...` allowlist entry. Hive and category
roots such as `...\Classes\CLSID`, subtree selectors, value-name wildcards,
HKLM, and case-drifted view records are rejected.

The before/expected snapshots record key existence, value existence, registry
kind, and canonical base64 data. Before any key creation, the durable prepared
descriptor MUST record:

- `existing_ancestor_path`: the deepest key observed to exist, with
  `HKCU\Software` as the lowest permitted boundary; and
- `key_ownership`: every then-absent key through the allowlisted leaf in
  canonical shallow-to-deep order. Each record contains its exact `path`, an
  initial `creation_disposition` of `pending`, and a unique prejournaled
  `marker_name`/`marker_token` pair.

Absence is a creation plan, never proof that this run created a key. Each
planned key MUST be opened/created one at a time with x64 `RegCreateKeyExW`, and
the returned Win32 disposition is authoritative:

- `REG_CREATED_NEW_KEY` permits writing the prejournaled exact `REG_SZ` marker;
  after the marker is flushed, the durable disposition becomes
  `created-owned`;
- `REG_OPENED_EXISTING_KEY` writes no marker and durably becomes
  `opened-existing`; cleanup MUST preserve it; and
- any error or unknown disposition is fail-closed.

Creation outcomes form one durable shallow-to-deep prefix. The exact descriptor
update event for each outcome MUST be durable before proceeding. Value mutation
MUST open an existing exact Registry64 key and MUST NOT implicitly create it.

Cleanup first restores or deletes only the exact value. It then visits only the
recorded `key_ownership` chain deepest-first. An `opened-existing` key is never
deleted. A `pending` or `created-owned` key may be deleted only when its exact
marker is present as a string with the exact token, that marker is the key's
only value, and the key has no subkeys. The marker MUST remain inside the key
until x64 `RegDeleteKeyExW` deletes the key; cleanup never removes the marker as
a separate mutation.

The crash outcomes are deliberately asymmetric:

- before `RegCreateKeyExW`, a pending record with no key causes no mutation;
- if another actor creates the key first, the Win32 opened-existing disposition
  preserves that actor's key;
- a crash after key creation but before its marker leaves an unprovable key,
  which cleanup preserves and reports as a blocking conflict;
- a crash after the exact marker but before the descriptor update remains
  provable from the prejournaled token, so cleanup may remove the otherwise
  empty key; and
- a crash after value rollback but before key deletion resumes from the intact
  marker chain.

A pre-existing ancestor is never in the deletion chain. A marker-owned key
that later contains any neighbor value or subkey is preserved as a cleanup
conflict. The digest and random marker are non-hostile integrity/ownership
evidence, not authentication against a malicious same-user actor that reads
and reproduces the journal token. External mutation racing the final
proof-to-delete interval is outside this cooperative test boundary; native
deletion still fails closed when Windows reports a populated/nonempty key.

HKCU-first means a test MUST use this exact user-scoped route whenever the
behavior can be tested there. Machine registration required by a downstream
capability is provisioning work outside this synthetic helper and requires its
own explicit owner, registry view, exact allowlist, before snapshot, rollback,
and clean-environment evidence. It may not borrow this bead's acceptance.

### 5.2 Files

Files use one canonical absolute path and an absent-to-exact create-only
transition. The journal records expected length and content SHA-256 before
`FileMode.CreateNew`. Its prepared descriptor starts with
`creation_disposition=pending` and empty identity fields. After the new file is
written and flushed, the helper captures the local Windows volume serial and
file ID from that still-open creation handle and durably records those values
with `creation_disposition=created-owned` before activation.

Cleanup opens the exact path with read and delete rights but without sharing
write or delete, rejects directories and reparse points, and keeps that handle
open while it verifies the recorded volume/file ID, length, and SHA-256 and
marks that exact handle for deletion. A path-only or content-only match is not
ownership: a different file instance containing the same bytes is preserved as
a conflict. If the path is already absent, cleanup is idempotent. A pending
record with an existing file is also a conflict because a crash between
`CreateNew` and the durable created-owned identity cannot prove who won the
path.

Recursive deletion, directory-root deletion, wildcard paths, traversal,
reparse traversal, and overwrite are forbidden.

### 5.3 Child processes

A test process MUST be a harmless child of a validated journal writer and its
executable MUST match an exact path allowlist. The reference launcher uses a
hidden, no-window, noninteractive `pwsh` child with a maximum 60-second
self-timeout.

Launch is two phase:

1. persist a `prepared` process record with PID `0`, exact executable and
   argument digest, absent activation path, parent PID, and timeout;
2. start an inert child that waits for that unique activation path;
3. capture and durably persist the child PID and process start UTC, changing the
   process record to `active`; then
4. create the separately journaled activation file.

If recording PID/start fails, the launcher terminates only the local process
handle it just created. If the parent crashes before PID assignment, the inert
unknown child self-expires; recovery MUST NOT discover or kill it by name.

Cleanup may stop a process only when the recorded PID is live with the recorded
start time and the executable path also matches. Creation time is queried
first. A missing PID or different creation time is treated as the original
child already gone and does not permit an executable-path query. Only a live
exact PID/start match proceeds to executable verification and termination; an
inaccessible creation time or executable is a conflict. Process-name, command
line search, service-name, executable-name, job-wide, and all-process cleanup
are forbidden.

### 5.4 Dialog and UI Automation identity

UIA/dialog ownership is process-scoped. The record MUST reference one active
owned process and repeat its PID/start identity, plus one UIA runtime ID, native
window handle, title digest, and allowed action (`observe-only` or
`dismiss-exact`). A global `#32770` scan, window-title search, class selector,
or dismissal outside the recorded process is forbidden.

The synthetic acceptance suite uses a journal-backed dialog representation; it
does not launch Excel/VBE or call UI Automation. Real Excel/VBA oracle runs
remain subject to the PID-scoped modal-handling rules in `AGENTS.md` and MUST
capture an exact owned process/UIA identity before dismissal.

### 5.5 Apartments and reentry

An apartment record MUST declare process ID, owning thread ID, model (`STA`,
`MTA`, or `none`), COM initialization ownership, message-pump ownership,
reentry policy, and maximum nested depth. It MUST match the root orchestrator
declaration. Ambiguous or implicit apartment/reentry state is non-conforming.

The synthetic suite uses `logical-only-no-com`; it does not initialize COM.
Production evidence that uses `CoInitializeEx-owned` MUST pair every successful
initialization with same-thread teardown after dependent connections and
callbacks are retired. Wrong-thread use is rejected rather than repaired by an
unowned proxy or thread hop.

### 5.6 Callbacks and connections

A callback MUST reference one owned apartment and declare session/thunk
identity, owning thread, strong retention through unregistration,
wrong-thread rejection, and rejection after retirement.

A connection MUST reference one owned apartment and active callback and record
exact source/sink identities, connection-point IID, nonzero cookie, and
writeback policy. Acquisition order is apartment, callback, connection.
Cleanup order is connection `Unadvise`, callback retirement, then apartment
teardown. No source-wide or cookie-agnostic disconnect is allowed.

The acceptance suite models these lifetimes in the journal only. It performs no
real COM activation, Advise/Unadvise, callback, or reentry.

## 6. Cleanup, conflict, and recovery algorithm

Normal cleanup requires the exact journal owner PID/start or an exact recorded
live child writer. Stale recovery requires the opposite: the owner PID/start
MUST be absent or mismatched. Recovery refuses a still-live exact owner.

Cleanup MUST:

1. acquire the per-journal transaction lease and validate the complete journal,
   raw schema, identity, digest, dependencies, lifecycle, and path boundary;
2. write `cleaning` durably while retaining that lease;
3. visit resources in strictly descending acquisition sequence;
4. compare each real resource with its exact `before` and `expected` snapshot;
5. apply only its resource-specific exact inverse;
6. durably record `resource-cleaned` and the exact action after each inverse;
7. durably record conflicts in that same descending sequence and continue with
   independently safe resources; and
8. finish as `completed`, or `cleanup-conflict` with the exact resource IDs and
   diagnostics, before releasing the lease.

If current state equals `before`, the inverse is already complete. If it equals
`expected`, the exact inverse is safe. If it equals neither, cleanup MUST NOT
guess or overwrite: it marks `conflict`, preserves the changed resource, and
requires manual reconciliation. A later retry may proceed if an owner restores
the exact expected state.

Calling cleanup again on `completed` MUST return without rewriting the journal.
This byte-idempotence applies to normal and stale cleanup.

An outer test teardown MUST surface cleanup/validation failures. It may remove
only journals and run roots that have reached validated `completed` state and
whose run roots are empty. Any `cleanup-conflict`, corrupt journal, or residual
entry preserves its journal and recovery root; teardown must not erase the
prerequisites needed for an exact retry merely to make the test directory look
clean.

An abandoned but valid journal is the sole stale-recovery authority. An
abandoned named mutex is not authority by itself; the journal must be strictly
revalidated before its acquired lease can mutate. Recovery
does not scan registry hives, directories, processes, windows, ROT entries, or
connection points for things that look related. A corrupt or ambiguous stale
journal is a blocker and grants no cleanup authority.

## 7. Explicitly forbidden cleanup shapes

The following are non-conforming even when used in a test teardown block:

- `Stop-Process`/kill by name, executable wildcard, or enumeration;
- blanket dialog dismissal, global window-class scans, or unscoped UIA action;
- recursive registry-key deletion, hive/category deletion, or wildcard values;
- recursive/wildcard file deletion or deletion outside exact repo/temp bounds;
- COM unregistration not tied to an exact recorded value/owner;
- callback retirement before dependent connections are unadvised;
- apartment teardown while a callback/connection is still owned; and
- cleanup of any registration, process, dialog, file, callback, or connection
  absent from the validated journal.

## 8. Required acceptance observables

Conformance evidence MUST show:

- unique-run collision refusal;
- strict raw JSON kind/range/array/object validation; property-case, unknown,
  duplicate, counter, dependency, lifecycle, identity, and digest fail-closed
  behavior;
- one unforgeable per-journal lease across complete prepare/mutate/activate and
  cleanup transactions, plus abandoned-mutex revalidation;
- at least 24 deterministic contending recorded writers with gap-free resource
  sequences, unique records/targets, no lost atomic-move temporary, and a
  writer-versus-cleanup race with no unjournaled mutation or residue;
- prepared-before-mutation ordering for file, registry, and child process;
- child PID/start/executable recorded before activation;
- exact HKCU value, confined file, and hidden harmless child creation/cleanup;
- exact Registry64 view/disposition/token records and these registry paths:
  crash before create, external empty key, another actor winning creation,
  create-before-marker ambiguity, marker-before-descriptor recovery, normal
  rollback, value rollback before key deletion, pre-existing key, and populated
  marker-owned ancestor conflict;
- repository-root, temporary-root, journal-infrastructure, run-root, confined
  traversal, escape, wildcard, controlled-root, and reparse rejection;
- blanket process/dialog/registry/file selector rejection;
- explicit apartment/reentry/callback/connection/dialog identities;
- connection cleanup before callback and apartment cleanup;
- strictly descending resource cleanup/conflict sequences and exact root/resource
  lifecycle agreement;
- conflict refusal that leaves changed owned data untouched;
- live-owner recovery refusal and dead-owner stale recovery;
- byte-identical repeated normal and stale cleanup; and
- exact preservation of neighboring registry, file, process, and logical
  sentinels.

The reference suite MUST use a unique synthetic HKCU leaf, a unique temporary
root, and a uniquely recorded harmless child. Its apartment, callback,
connection, and dialog records MUST remain logical/file-backed. It MUST NOT
start Excel/VBE or invoke real COM/UIA.

## 9. Matrix and downstream boundary

This bead owns and advances only `CONF-MATRIX-001`. `PROFILE-WIN-001` is its
parent/profile context, while `SEC-BOUNDARY-001` and `CONF-QUALITY-001` are
downstream constraints informed by the policy; none of those three clauses is
advanced by this support result.

This policy is prerequisite support for resource-mutating evidence around
`WAC-SAFETY-MUTATION`, `WAC-TARGET-DEV-ENV`, `WCC-LATE-OUTPROC-ERROR`,
`WCE-INCOMING-APARTMENT`, `WCE-INCOMING-LIFECYCLE`, `WCS-SERVER-SAFETY`,
`WNI-CALLBACK-NESTED`, and `WNE-NATIVE-REPRO-DEPLOY`. It supplies no result,
Err, effect, transport, balance, Excel/VBA parity, or release-environment
evidence for those rows.

Downstream fixtures and oracle drivers still need to adopt this policy for
their own mutations. A clean synthetic policy run cannot substitute for a
clean release VM, real x64 artifact, or capability-specific test.
