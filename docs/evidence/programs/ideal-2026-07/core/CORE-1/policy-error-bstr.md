# CORE-1 Policy-Error BSTR Ownership Repair

Date: 2026-07-11
Bead: `bd-59co.2.2.6`
Base: `c0ff1de7a61dce2df6c83eff730582f2fd6f969b`
Matrix route: `CORE-READINESS/CORE-BASELINE-BALANCE-LIFECYCLE`
Clauses: `CONF-QUALITY-001|RUNTIME-VALUE-001|SEC-BOUNDARY-001`

## Result

The isolated `host-policy-error` fixture, which executes
`conformance/jit_v2/tracer_bullets/tb08_native_declare_shared_abi.bas`, retains
its selected OxVba host-policy denial and now reports zero drift for every
carrier counter. Four repeated serial child runs and four simultaneous child
runs produced identical reports.

This is evidence for OxVba's own host-policy denial and carrier ownership. It is
not Excel/VBA oracle evidence and makes no VBA-authority claim for the denial.

## Root cause and repair

`StrPtr(textValue)` registered a Windows `PointerEntry::Bstr` by calling
`BStr::clone_raw_bstr`. That allocation incremented the canonical live-BSTR
counter. When the dynamic-link HAL denied the first `lstrlenW` Declare, VM3's
error arm correctly called `free_pins`, and the pointer entry freed the OS BSTR
with `SysFreeString`. Its raw owner bypassed `BStr::drop`, however, so the live
counter never received the matching debit. The observed `+1` was therefore a
real ownership-accounting imbalance even though the OS allocation was freed.

The pointer entry now owns an `Option<BStr>` directly. Null BSTR identity is
preserved, byte-exact cloning preserves embedded NUL and odd-byte payloads, and
both normal and error pin release flow through the canonical `BStr` destructor.
No public API or runtime error contract changed.

The directly coupled mutable `VarPtr(String)` cell keeps its original tracked
`BStr` separate from the writable native `BSTR` cell:

- an unchanged cell drops the original canonical owner;
- under the existing LPBSTR transfer convention, a changed/null cell means the
  native callee consumed the original before writing through the cell, so only
  that original tracked allocation is reconciled;
- a genuine native replacement is freed as a native allocation and does not
  receive a false OxVba counter debit.

The focused test exercises unchanged, consumed-to-null, and native-replacement
paths in its own integration-test process. A wrong extra debit would produce
`-1`; a missed original debit would retain `+1`; the observed delta is zero.

## Observable

| axis | observation |
|---|---|
| result | Completion remains `raised`; result message remains `VBA error 5`. |
| Full Err | `number=5`, `source="VBAProject"`, `description="operation blocked by host policy"`, `last_dll_error=0`. |
| side effects | The selected path is still the deterministic dynamic-link host-policy denial. The balance protocol has no HAL side-effect journal, so this artifact does not independently claim the absence of native side effects. |
| lifecycle/order | `StrPtr` registers the BSTR pin; Declare gathers the pin address; HAL returns policy denial; the error arm releases the named pin; registry removal drops the canonical `BStr`; the fault then propagates with the same Err state. The neighboring successful mock-Declare path also releases after readback. |
| transport | VM3 still routes `OxNativeCallee::Declare` through `DynamicLinkHal::invoke_descriptor_variants`; the subprocess evidence remains one `OXVBA_BALANCE_V1` JSON report per owned child. No COM or JIT transport claim is made. |
| balance | `host-policy-error`: BSTR `0`, object box `0`, SAFEARRAY `0`, record buffer `0`, `related={}` in every serial and parallel run. |

## Checks

Environment: Microsoft Windows NT `10.0.26200.0`, x64 MSVC; Rust/Cargo
`1.94.1`.

```text
cargo test -p oxvba-differential policy_error_bstr_balance -- --nocapture
PASS: 1 named acceptance test; four serial plus four simultaneous reports were identical, retained Full Err 5, and had zero total carrier drift.

cargo test -p oxvba-differential --test balance_fixture_protocol -- --nocapture
PASS: 3 tests; named protocol, repeated/parallel policy balance, and all-fixture parallel isolation.

cargo test -p oxvba-differential --test pointer_bstr_ownership -- --nocapture
PASS: 1 process-isolated test; ordinary, unchanged-cell, native-consumed-to-null, and native-replacement ownership paths balanced.

cargo test -p oxvba-vm3 strptr_pins_and_a_declare_writeback_round_trips_the_string -- --nocapture
PASS: 1 focused successful-Declare/readback test.

cargo test -p oxvba-runtime pointer_helpers::tests -- --nocapture
PASS: 17 pointer-helper neighboring tests.

cargo clippy -p oxvba-runtime --lib --no-deps -- -D warnings
PASS: touched runtime library strict Clippy.

cargo clippy -p oxvba-differential --test pointer_bstr_ownership --no-deps -- -D warnings
PASS: new ownership test strict Clippy.

cargo clippy -p oxvba-differential --test balance_fixture_protocol --no-deps -- -D warnings
PASS: named balance protocol strict Clippy.

cargo fmt --all -- --check
PASS.

.\scripts\check-governance.ps1
BLOCKED OUTSIDE THIS BEAD: repository checks passed through `project-integration-catalog`, then `pmr-event-snippets` rejected the pre-existing stale generated `docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md`. This bead neither owns nor regenerated that canonical/generated PMR surface.

cargo test -p oxvba-differential vm3_golden_snapshot -- --nocapture
BLOCKED OUTSIDE THIS BEAD: the corpus ran, then snapshot comparison first differed at line 17 on the unrelated Excel `Visible` COM diagnostic wording. The golden says “requires authoritative metadata resolution before COM lowering”; current output says “did not resolve through authoritative object metadata”. `bd-59co.2.2.7` owns this cross-platform diagnostic/snapshot repair. No snapshot was blessed or changed here.
```

The focused no-dependency Clippy commands intentionally exclude an already
owned `oxvba-hal` dead-code warning (`projection_member_token_by_name`) printed
while dependencies compile; the touched packages themselves are strict-clean.

## Residuals

- `bd-59co.2.2.7` is the exact blocker for the otherwise unrelated
  `vm3_golden_snapshot` acceptance command. This bead does not alter the COM
  diagnostic or snapshot.
- The repository governance pass is independently red on the stale generated
  PMR diagnostic snippet named above. This bead does not alter canonical PMR or
  generated-summary files.
- `bd-59co.2.2.14` owns the separately identified, currently unmeasured Windows
  `VarPtr(Variant)` BSTR transfer/accounting seam: `OwnedBstr::into_raw` feeds a
  native VARIANT whose eventual `VariantClear` bypasses the OxVba live-counter
  debit. No existing named balance fixture reaches it; tb08 stops at the earlier
  policy-denied `StrPtr` call. This bead does not broaden into that Windows
  VARIANT transport surface.
- No other named fixture reported a carrier imbalance in the all-fixture serial
  or parallel protocol runs.
