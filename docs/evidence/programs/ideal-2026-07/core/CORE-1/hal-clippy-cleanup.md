# HAL projection strict-Clippy cleanup

Date: 2026-07-11

Bead: `bd-59co.2.2.13`

Baseline: `c0ff1de7a61dce2df6c83eff730582f2fd6f969b`

Matrix route: `CORE-READINESS/CORE-BASELINE-UNSAFE-CLIPPY`

Clauses: `CONF-QUALITY-001|RUNTIME-VALUE-001|SEC-BOUNDARY-001`

Status: targeted HAL repair complete; workspace and cross-platform baseline
certification remain with their existing CORE-1 successors.

## Result

The known `oxvba-hal` strict-Clippy warning is repaired without a lint
suppression. `projection_member_token_by_name` is a valid non-Windows helper:
the non-Windows `DynamicMemberSelector::Name` lowering path uses it to resolve a
projection object's ProgID and member name to a COM member token. On Windows,
the corresponding lowering path resolves through authoritative
`WindowsComBridge` object metadata, so the private helper was compiled but
unreachable and triggered `dead_code`.

The helper and its three helper-only metadata imports are now compiled only for
`not(target_os = "windows")`, matching the existing call-site boundary. Deleting
the helper would have regressed the non-Windows projection route; routing the
Windows path through it would have displaced the existing authoritative bridge
behavior. This source-level platform alignment changes no public interface and
no runtime branch on either platform family:

- Windows excludes a previously private, unreachable helper and unused imports.
- Non-Windows retains the same helper body, imports, call site, result, and fault
  behavior.

The pre-repair acceptance command failed only with `function
projection_member_token_by_name is never used`. The repaired command is clean.

## Observable axes

| Axis | Evidence |
|---|---|
| Result | Windows x64 strict HAL Clippy passes with zero warnings. The focused dynamic-name projection regression passes, and the full HAL suite passes 158 tests. |
| Full Err | n/a. No `HalError`, VBA `Err`, error number, source, description, help fields, LastDllError, or Erl construction changed. Existing unresolved-name and adapter-fault paths are byte-for-byte unchanged. |
| Effects | No host or COM side effect changed. The edit only aligns private source inclusion and helper-only imports with the pre-existing target-specific branch. |
| Lifecycle/order | n/a for behavior change. Projection-state locking, ProgID lookup, metadata construction, invocation, object lifetime, and cleanup order are unchanged on the non-Windows path; the Windows path never called this helper. |
| Transport | `DynamicCallRequest` to `ComInvokeRequest` lowering is unchanged. Windows continues to use `WindowsComBridge`; non-Windows continues to use projection ProgID/type-library metadata. No carrier, ABI, token, or public DTO changed. |
| Balance | n/a. No allocation, reference-count, release, live-counter, or carrier-balance operation changed. |

## Commands

Environment: Windows x64 `10.0.26200.0`; Rust/Cargo `1.94.1`.

| Command | Result |
|---|---|
| `cargo fmt -p oxvba-hal -- --check` | pass |
| `cargo clippy -p oxvba-hal --all-targets -- -D warnings` | pass; zero warnings |
| `cargo test -p oxvba-hal dispatch_invoke_dynamic_projection_resolves_name_selector_for_testdispatch` | pass; 1 passed, 0 failed |
| `cargo test -p oxvba-hal` | pass; 158 passed, 0 failed, 0 ignored |
| `cargo check -p oxvba-hal --all-targets --target x86_64-unknown-linux-gnu` | pass; confirms the retained helper/import/call-site configuration compiles for Linux x64 |

The Linux-target command was an optional cross-target compile check from the
Windows development host. It emitted pre-existing non-Windows warnings in
other runtime, COM, and HAL platform-specific surfaces and is not presented as
a strict or pinned-Linux baseline.

## Residuals

No semantic ambiguity, unsafe-code question, public-interface change, or
bead-owned behavior residual was found. The existing
`dispatch_invoke_dynamic_projection_resolves_name_selector_for_testdispatch`
test remains the focused cross-platform regression anchor; on non-Windows its
named-selector path reaches the retained helper.

Workspace-wide strict Clippy/lifecycle certification remains
`bd-59co.2.2.3`. Pinned Linux execution and terminal cross-platform
reconciliation remain the existing CORE-1 successors; this bead makes no
broader platform-certification claim.
