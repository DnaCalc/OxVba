# CORE-1 Fixture-Isolated Carrier Balance Protocol

Date: 2026-07-11
Bead: `bd-59co.2.2.5`
Matrix route: `CORE-READINESS/CORE-BASELINE-BALANCE-LIFECYCLE`
Clauses: `CONF-QUALITY-001|RUNTIME-VALUE-001|SEC-BOUNDARY-001`

## Result

`oxvba-differential` now has a versioned, named-fixture subprocess protocol for
carrier-balance evidence. Each child process runs exactly one registered fixture,
measures the existing runtime live counters before and after that fixture, and
emits exactly one deterministic report. Concurrent fixtures therefore do not
share the process-global counter state.

This is a measurement-protocol result, not a cleanup fix. The known policy-error
BSTR residual remains open under `bd-59co.2.2.6`.

## Protocol

The child executable is `oxvba_balance_fixture`. It accepts
`--fixture <stable-id>` and writes one stdout line:

```text
OXVBA_BALANCE_V1<TAB>{JSON}
```

The JSON schema identity is `oxvba.balance-fixture/v1`. Required report fields
are:

- stable fixture identity and executor identity;
- completion shape and deterministic result projection;
- complete final `Err` fields: number, source, description and LastDllError;
- signed BSTR, object-box, SAFEARRAY and record-buffer deltas;
- a named `related` delta map reserved for later carrier choke points.

Parsing is bounded at 64 KiB and fails closed for a missing or duplicate
protocol line, an unsupported schema/executor, an empty fixture identity or a
missing balance measurement. An unknown fixture exits nonzero and names the
rejected identity.

## Named observations

| fixture | result/full Err | BSTR | object | SAFEARRAY | record | related |
|---|---|---:|---:|---:|---:|---|
| `carrier-string` | completed; `alpha-beta`; Err 0 | 0 | 0 | 0 | 0 | `{}` |
| `carrier-array` | completed; Long 60; Err 0 | 0 | 0 | 0 | 0 | `{}` |
| `carrier-object` | completed; Long 42; Err 0 | 0 | 0 | 0 | 0 | `{}` |
| `carrier-record` | completed; Long 41; Err 0 | 0 | 0 | 0 | 0 | `{}` |
| `host-policy-error` | raised; Err 5; source `VBAProject`; `operation blocked by host policy` | **+1** | 0 | 0 | 0 | `{}` |

The policy fixture is the existing
`conformance/jit_v2/tracer_bullets/tb08_native_declare_shared_abi.bas` source.
Its isolated +1 BSTR result confirms the exact residual for `.6`; this bead does
not normalize, suppress or repair it.

## Six observable axes

| axis | evidence |
|---|---|
| result | Clean fixtures complete with deterministic string/Long projections; the policy fixture deterministically raises rather than being recast as a compile failure. |
| full Err | Clean fixtures report zeroed Err. The policy fixture reports number 5, source `VBAProject`, the full policy-denied description and LastDllError 0. |
| side effects | Clean fixtures perform only in-memory carrier work. The native call in the policy fixture is denied before external invocation; the protocol itself writes only its stdout report. |
| lifecycle/order | Parent spawns one fixture per child, captures the report, waits for exit and reaps it. The object fixture explicitly releases its local object. Serial and concurrent runs produce identical reports. |
| transport | One prefixed JSON line on captured stdout, with success/non-success process status kept separate from VBA completion shape. Missing, duplicate and unknown-fixture cases fail closed. |
| balance | Every report names all four current counter families plus `related`. Four clean fixtures are zero on every family; the policy fixture is isolated as BSTR +1 with all other families zero. |

## Checks

Environment: Windows x64 `10.0.26200`, Rust/Cargo `1.94.1`.

```text
cargo test -p oxvba-differential balance_fixture_subprocess_protocol -- --nocapture
PASS: 1 protocol integration test; clean and known-residual reports, parser and unknown-fixture rejection verified.

cargo test -p oxvba-differential balance_fixture_parallel_isolation -- --nocapture
PASS: serial and concurrent child reports are identical.

cargo test -p oxvba-differential balance_fixture_parallel_isolation -- --test-threads=1 --nocapture
PASS: the same internal serial/concurrent comparison is stable under a single-thread parent harness.
```

A focused diagnostic run of `vm3_golden_snapshot` remains red on the named
`tb08_native_declare_shared_abi.bas` BSTR +1 residual, as expected until
`bd-59co.2.2.6`. Its failure now names the exact fixture instead of reporting an
anonymous process-global outcome.

## Residual boundary

- `bd-59co.2.2.6` owns the policy-error BSTR cleanup and the later zero-balance
  expectation.
- CORE-9 may consume this protocol for broader differential/conformance evidence;
  this bead does not claim structural VM3/JIT or Excel/VBA parity.
- No runtime allocation/free choke point, `oxvba-runtime` file or SAFEARRAY code
  changed for this protocol.
