# WIN-0 Owned Windows Test Resource Policy Evidence

Date: 2026-07-11
Bead: `bd-59co.3.1.5`
Status: support-safety policy and safe synthetic proof complete
Capability credit: none

## Outcome and boundary

`docs/spec/OXVBA_WINDOWS_TEST_OWNERSHIP_POLICY_V1.md` now defines the normative
V1 ownership contract for Windows test mutations. The executable reference
implementation and acceptance suite are:

- `scripts/lib-windows-owned-resource-policy.ps1`;
- `scripts/test-windows-owned-resource-policy.ps1`.

The suite creates real resources only inside a unique synthetic envelope:

- one exact, unique HKCU leaf/value;
- create-only files below one unique system-temporary root; and
- uniquely recorded harmless hidden child `pwsh` processes with a maximum
  self-timeout.

Apartment, reentry, callback, connection, and dialog/UIA resources are journal
representations only. The proof starts no Excel/VBE process, performs no COM
initialization or activation, calls no Advise/Unadvise, and invokes no UI
Automation. It does not implement or certify any Windows capability row.

## Immutable implementation identity

Implementation/test commit:
`c809b18812b51249c59596b943eb5488e22495f6`
(`test(win0): enforce owned Windows resources`).

| artifact | SHA-256 |
|---|---|
| `scripts/lib-windows-owned-resource-policy.ps1` | `f3dc82b66410057c4402c16c4cb1773db05f0ee95a530827c3243ffb483e46bd` |
| `scripts/test-windows-owned-resource-policy.ps1` | `fc67c03ef6b6ffb9f142754980324f7e4f2e17919411536d6d8bac71bb1ba2ea` |
| `docs/spec/OXVBA_WINDOWS_TEST_OWNERSHIP_POLICY_V1.md` before this evidence commit | `47617a103d16d5402b5e1694f114a69293a1469e1b17d0e7a5df4b9d5205c528` |

The journal schema is
`oxvba-windows-owned-resource-journal-v1`, version `1`. Journals use strict
UTF-8 JSON, duplicate-property rejection, exact schemas, gap-free resource and
event counters, fixed-order SHA-256 integrity, adjacent write-through files,
and atomic replacement.

## Safe synthetic result

Command:

```powershell
pwsh -NoProfile -File ./scripts/test-windows-owned-resource-policy.ps1
```

Result:

```text
PASS: Windows owned-resource policy (37 assertions; 28 fail-closed mutations; real HKCU/file/child; logical COM/UIA only)
```

The final run completed on the Windows development host. Every real
child was launched by the policy helper with `UseShellExecute=false`,
`CreateNoWindow=true`, hidden window style, `-NoProfile`, `-NonInteractive`, an
exact script path, an exact activation path, and a bounded self-timeout.

## Prepared-before-mutation proof

The suite inspects the validated main journal and proves that file, registry,
and process `resource-prepared` events precede their `resource-active` events.
The implementation writes the descriptor, before snapshot, expected snapshot,
resource sequence, and prepared event atomically before these mutation calls:

| resource | pre-mutation record | mutation |
|---|---|---|
| file | absent before; expected length and SHA-256 | `FileMode.CreateNew`, write-through flush |
| HKCU value | exact key/value before; kind and base64 expected bytes | exact `SetValue` and key flush |
| process | executable/arguments/activation/parent/timeout; PID `0` | start inert child |

For the process, the suite additionally proves the active record contains the
exact executable path, PID, and process start UTC. The activation-file resource
has a later acquisition sequence than the process, so the inert child cannot
act until PID/start identity is durable. A separate prepared process record is
never assigned a PID; reverse cleanup completes it without process discovery,
proving the pre-start crash path.

## Exact ownership and rollback proof

The main lifecycle acquires, in order, an owned payload file, exact HKCU value,
owned child script, harmless child process, activation file, apartment,
callback, connection, and process-scoped dialog representation. The suite
compares every `resource-cleaned` event with the strictly descending acquisition
sequence. It separately proves that connection cleanup records exact-cookie
Unadvise before callback retirement, with apartment retirement later.

Real cleanup is fail-closed:

- files are removed only when path, expected length, and SHA-256 match;
- registry cleanup restores only the exact allowlisted HKCU value snapshot and
  retains a non-owned neighbor value/key;
- processes are stopped only when recorded PID, process start UTC, and
  executable path all match;
- dialogs require the exact recorded process plus UIA runtime ID and native
  handle; and
- logical connection/callback/apartment records retire in reverse dependency
  order.

The completed journal is hashed, cleanup is called again, and the journal hash
remains byte-identical. The same byte-idempotence check passes for stale-owner
recovery.

## Fail-closed mutation coverage

The 28 negative probes include:

- duplicate run ID collision;
- stale recovery while the exact owner remains live;
- file escape, wildcard, controlled-root, and junction/reparse traversal;
- HKLM, broad HKCU category, non-allowlisted key, and wildcard value name;
- process-by-name, dialog-by-window-class, registry-subtree, and recursive-file
  cleanup intentions;
- an unrecorded PID/resource ID and a mismatched PID/start selector;
- apartment-model ambiguity;
- missing/wildcard callback identity and missing callback/cookie connection
  identity;
- unrecorded dialog process and wildcard UIA identity;
- journal digest tampering, a recomputed-digest repository-root escape, and
  valid JSON with a duplicate property;
- changed owned-file conflict; and
- normal cleanup of a journal whose exact owner has exited.

Each rejection occurs before the forbidden mutation. Digest, schema, duplicate,
identity, and selector failures grant zero cleanup authority.

## Crash, stale journal, and conflict proof

The stale-owner scenario uses a parent-owned, exact recorded harmless child.
After activation, that child creates its own unique journal and one journaled
file, then exits without cleanup. The parent observes exact PID/start exit,
proves the journal/file remain, proves ordinary non-owner cleanup is rejected,
and invokes stale recovery. Recovery validates the full journal, requires the
owner PID/start mismatch, removes only the recorded file, and reaches
`completed`. A second recovery does not rewrite the journal.

The conflict scenario changes a journaled file to bytes matching neither its
before nor expected snapshot. Cleanup records `cleanup-conflict`, leaves those
changed bytes untouched, and preserves neighbor sentinels. After the exact
expected bytes are restored, a retry performs the safe inverse and completes.
No discovery scan or broadened selector is used.

## Zero unrelated drift

Before mutation, the suite captures four independent sentinels:

| sentinel | comparison after main, conflict, stale, and final phases |
|---|---|
| neighbor value in the same HKCU key | key/value existence, kind and encoded bytes exact |
| neighbor file under the outer test root | SHA-256 exact |
| current unowned PowerShell process | PID and process start UTC still live/exact |
| logical object | canonical JSON SHA-256 exact |

All four comparisons pass after cleanup and again at the end. The owned HKCU
value, owned payload/stale files, and exact child PID/start are absent after
their rollback. Test-infrastructure teardown removes its known exact sentinel,
journal, and empty-directory paths; it does not use recursive resource cleanup.

## Validation record and inherited governance gate

| check | result |
|---|---|
| PowerShell AST parse and dot-source of policy helper | pass |
| real file lifecycle smoke plus byte-idempotent cleanup | pass |
| real HKCU value lifecycle smoke | pass |
| real hidden child plus logical apartment/callback/connection/dialog smoke | pass |
| full owned-resource acceptance suite | pass, 37 assertions / 28 rejections |
| `git diff --check` | pass |
| `pwsh -NoProfile -File ./scripts/check-governance.ps1` | branch-baseline stop at `pmr-event-snippets` |

The governance command passes `docs-check`, `active-program-sync`,
`divergences`, `deferred-oracle-gates`, `pmr-followup-sync`, and
`project-integration-catalog`, then reports:

```text
stale generated PMR diagnostic artifact: docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md
```

That generated file has no `.3.1.5` worktree diff and was already stale on this
isolated branch base. Generated/controller truth edits are outside this bead's
authorized scope, so this work does not regenerate or commit it. The primary
integration branch was independently confirmed to pass this PMR snippet check;
the controller must rerun full governance after integrating these two bounded
commits.

## Fresh-eyes review and residual boundary

Fresh-eyes review rechecked accidental broad selectors, path canonicalization,
reparse traversal, journal validation before cleanup, process reuse, executable
identity, prepared-state crash windows, registry-neighbor retention, callback
ordering, stale/live-owner discrimination, conflict behavior, and idempotence.
No blanket process/dialog/registry/file cleanup or unowned mutation remains in
the executable policy path.

Residual work remains downstream: real Excel/VBA, COM, UIA, native, VM3/JIT,
registration, and clean-VM evidence drivers must adopt this policy (or prove an
equivalent exact contract) for their own resources. This support result does
not close or advance those capability lanes.
