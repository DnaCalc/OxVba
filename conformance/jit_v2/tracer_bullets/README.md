# JIT v2 Tracer Bullet Fixtures

These fixtures are planning-stage inputs for the future JIT v2 differential
harness. They are not part of `scripts/run-conformance.ps1` and must not be used
as active JIT execution evidence until the JIT v2 implementation workset adds
the harness.

All tracer bullets have a VM seed path today:

- host-independent fixtures run through `scripts/run-jit-v2-tracer-fixtures.ps1`
  and `expected_vm_values.csv`;
- hosted COM/type-library/native fixtures run through
  `cargo test -p oxvba-host --test jit_v2_tracer_vm_seed -- --nocapture`.

Authoritative test design:
`docs/spec/JIT_V2_TRACER_BULLET_TEST_PLAN_V1.md`.

Manifest:
`manifest.csv`.
