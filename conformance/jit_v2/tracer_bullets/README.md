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
  The hosted TB06 through TB08 tests also assert
  `VmPackageIdentityEvidence::interop_descriptor_evidence` for the current
  COM activation/dispatch and native Declare/invoke descriptor expectations.

VM seed readiness is separate from package/JIT readiness. TB01 through TB09
executable JIT work is gated by the package evidence named in
`docs/validation/JIT_V2_TRACER_BULLET_MATRIX_V1.csv`: declared primitive
slot/carrier evidence for TB01; UDT descriptor plus selected layout/init and
lifecycle evidence for TB02; package error-frame evidence for TB03; declared
String/BSTR helper and selected slot/UDT cleanup evidence for TB04; and array
descriptor, SAFEARRAY bounds, element lifetime, selected ownership evidence,
and bounds-error follow-up evidence for TB05; COM boundary descriptor, selected
ObjectRef identity/lifecycle policy, imported COM WithEvents route identity,
projection, and cleanup evidence for TB06/TB07; native ABI, writeback, cleanup,
and error-policy evidence for TB08; and exported-callable projection,
cleanup/error policy, writeback, and unsupported-shape diagnostics for TB09.
Remaining byte-offset/ABI layout, explicit cleanup/deopt execution,
VM-runnable bounds-error and multi-rank coverage, general COM/native/export
projection, and verifier gaps must stay explicit until the JIT workset owns
them.

Authoritative test design:
`docs/spec/JIT_V2_TRACER_BULLET_TEST_PLAN_V1.md`.

Manifest:
`manifest.csv`.
