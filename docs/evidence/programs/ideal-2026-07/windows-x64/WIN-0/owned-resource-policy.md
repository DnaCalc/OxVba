# WIN-0 Owned Windows Test Resource Policy Evidence

Date: 2026-07-11
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

The final repair serializes every journal transaction with a path-stable named
mutex, makes every real Registry64 key creation depend on the Win32 disposition
and a prejournaled marker token, rejects reparse roots/infrastructure at use
boundaries, and validates raw JSON kinds and lifecycle/dependency order before
PowerShell coercion. Cleanup remains exact-resource, reverse-order, and
fail-closed.

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
  (`fix(win0): serialize and prove Windows resource ownership`).

Prior normative/evidence repairs are
`1dbb7a7699dd2280eebcb2fb9db6912df36f5bc9` and
`6eb844b56ec53c29c189187b8fe68852815569f9`. The current documentation repair
is intentionally a separate successor to the implementation commit above.

| artifact | SHA-256 |
|---|---|
| `scripts/lib-windows-owned-resource-policy.ps1` | `c28d59e5544a08cc2be5b780ba6b9c199a4cf21c098f7b28824cc9aeee1d7671` |
| `scripts/test-windows-owned-resource-policy.ps1` | `f2ebea2ba3686d5b224d7ab0b57adfc70937d3f074751474901d3b377717d691` |
| `docs/spec/OXVBA_WINDOWS_TEST_OWNERSHIP_POLICY_V1.md` before this evidence update | `359fe1d6224d14aeb7b211fb18785fab6cb187700fadffe134042016c2d77526` |

The journal schema remains
`oxvba-windows-owned-resource-journal-v1`, version `1`. The root now explicitly
binds `registry_view` to exact `Registry64`; registry descriptors carry their
own matching view and per-key ownership records.

## Final validation record

Command:

```powershell
pwsh -NoProfile -File ./scripts/test-windows-owned-resource-policy.ps1
```

Final result after the fresh-eyes hardening pass:

```text
PASS: Windows owned-resource policy (57 assertions; 47 fail-closed mutations; real HKCU/file/child; logical COM/UIA only)
```

The run completed in 141.2 seconds on the Windows x64 development host. An
immediately preceding run of the lease/resource-binding repair also passed 57
assertions and 46 fail-closed mutations in 146.3 seconds; the final extra probe
is a recomputed-digest root/lifecycle inconsistency rejection.

| check | result |
|---|---|
| PowerShell AST parse of both scripts | pass |
| `git diff --check` | pass |
| full owned-resource acceptance suite | pass, 57 assertions / 47 rejections |
| post-run `oxvba-owned-policy-test-*` temporary roots | zero |
| post-run `HKCU\Software\OxVbaOwnedResourcePolicy` in Registry64 | absent |
| post-run recorded writer/abandon/loop child processes | zero |

## Whole-transaction lease and race proof

The named mutex is derived from the lowercase canonical journal path and its
SHA-256 digest:

```text
Local\OxVba.WindowsOwnedJournal.<digest>
```

Acquisition is bounded at 120 seconds. The in-process lease registry binds the
actual token object, journal path/name, PID, managed thread, acquired state,
and abandoned/revalidated state. A fabricated token is rejected. An abandoned
mutex is mutation-disabled until the complete existing journal is strictly
re-read and validated; an absent new run must prove both immutable paths absent
after parent/reparse validation before infrastructure creation.

Each normal mutation holds one lease across validation, durable `prepared`, the
real mutation, durable `active`, and release. Cleanup holds the same lease
across `cleaning`, every reverse inverse and terminal resource write, and final
`completed`/`cleanup-conflict`. Nested helpers validate the exact live lease;
the cleanup race also proves balanced same-thread recursive acquisition.

The deterministic contention scenario launches 24 exact recorded harmless
children. All wait on one journaled gate, then contend to create 24 unique
journaled target files. The suite proves:

- all 24 exact children exit within their contract;
- exactly 24 unique target resources and files exist;
- every resource sequence is gap-free with no overwritten/lost record; and
- no adjacent `.write-*` journal temporary remains.

A 25th recorded child waits on a second gate. The owner retains the same
whole-transaction lease while opening that gate and invoking cleanup. The
writer cannot cross its prepare/mutate boundary; cleanup stops only its exact
PID/start/executable and completes with no target record, target file, child,
or temporary residue.

A separate recorded child acquires the mutex and exits without releasing it.
The next acquisition observes `AbandonedMutexException`; a journal write is
rejected before revalidation, then strict revalidation enables safe cleanup.
The suite finally proves the per-process live-lease registry is empty.

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
creates one key at a time and trusts only the Win32 disposition:

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
| run creates key, crashes before marker | preserve and conflict: create-before-marker ownership is unprovable |
| exact markers written, descriptor outcomes still pending | token proof removes the empty chain |
| another actor wins every creation disposition | record opened-existing, roll back only value, preserve keys |
| normal created-owned value/key rollback | restore value and remove exact marker-owned chain |
| value already rolled back before key deletion | resume through intact markers and remove exact chain |
| pre-existing key with neighbor sentinel | remove owned value only; preserve key and sentinel exactly |
| marker-owned ancestor becomes populated | preserve neighbor and ancestor as conflict; later exact retry succeeds |

The create-before-marker interval is intentionally not claimed as proven crash
cleanup. It is preserved as a blocking ambiguity. In contrast, marker-before-
descriptor is provable because the unique marker token was durable in the
prepared record before creation. The checksum/token model is cooperative: it
does not authenticate against a malicious same-user actor that copies a token,
and it cannot make the external proof-to-delete interval transactional.

## Exact rollback, stale recovery, and unrelated drift

Files use exact absent-to-create-only snapshots and `FileMode.CreateNew`.
Cleanup deletes only the expected length/SHA-256 bytes; changed bytes become a
conflict and remain untouched. Child cleanup requires exact PID, process start
UTC, and executable. The inert activation protocol makes PID/start durable
before the separately journaled activation file exists. No process-name or
command-line discovery is used for cleanup.

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

Four sentinels—neighbor Registry64 value, neighboring file hash, current
unowned PowerShell PID/start, and logical-object digest—remain exact after main,
conflict, stale, race, registry-crash, and final phases. No recursive registry
or file cleanup and no global process/window cleanup is present.

## Fail-closed coverage

The 47 negative probes cover:

- caller-root/infrastructure/run/confined reparse paths, path escape, wildcard,
  and controlled roots;
- fabricated/unrevalidated leases, duplicate run ID, live-owner recovery, and
  normal cleanup by a dead owner's non-owner;
- HKLM, broad category, non-allowlisted Registry64 key, wildcard value,
  external empty key, create-before-marker ambiguity, and populated ancestor;
- blanket process, dialog-class, registry-subtree, and recursive-file cleanup;
- unrecorded or mismatched process identity, apartment mismatch, missing or
  wildcard callback/connection/dialog identity, and invalid cookie;
- string boolean/number coercion, scalar array, property/view case drift,
  unknown/duplicate property, digest/root identity tamper, later dependency,
  missing event, forward cleanup, and root/lifecycle inconsistency; and
- changed owned-file conflict.

Every rejection is asserted before the forbidden mutation or preserves the
ambiguous/drifted resource as an explicit cleanup conflict.

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
abandoned state, forged tokens, registry view/disposition inference,
create/marker crash windows, marker lifetime, pre-existing/populated keys,
reparse swaps, raw coercion, dependency order, cleanup conflict order, handle
lifetime, child/file operation boundaries, exact residue, and destructive
primitive call sites. The final hardening added absent-new-run abandoned
revalidation, conflict-aware lifecycle state validation, operation-boundary
reparse checks, and exception-safe Registry64 handle disposal before the final
passing run.

Residual work is downstream. Real Excel/VBA, COM, UIA, native, VM3/JIT,
registration, and clean-VM drivers must adopt this policy (or prove an
equivalent exact contract) for their own resources. The marker checksum model
does not claim a hostile-machine trust boundary, and create-before-marker
remains an explicit blocking ambiguity. This support result does not close or
advance any downstream capability lane.
