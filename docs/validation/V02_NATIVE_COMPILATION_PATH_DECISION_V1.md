# V0.2 Native Compilation Path Decision

Date: 2026-04-27

Owning bead: `bd-bqm8.10.2`

## Decision

V0.2 uses a staged hybrid native-compilation path:

- The primary V0.2 product target is a wrapper-hosted compiled artifact. The
  artifact embeds an `.oxb` bundle and uses generated Rust shims for executable,
  DLL, COM server, or XLL surfaces where those shims are explicitly validated.
- Cranelift remains the primary execution accelerator inside the OxVba runtime
  host. It may execute supported bytecode subsets, but V0.2 does not claim a
  standalone direct native-image or full AOT compilation pipeline.
- Unsupported or failing JIT paths must continue to fall back to VM execution
  with the same observable runtime semantics.

## Product Boundary

In scope for V0.2:

- wrapper-generated executable shims that embed compiled `.oxb` bundles
- wrapper-generated DLL, COM server, and XLL source surfaces where validation
  proves the generated ABI skeleton and packaging boundary
- Cranelift-backed execution as an internal acceleration path for supported
  bytecode
- VM fallback as the correctness boundary for unsupported bytecode

Out of scope for V0.2:

- standalone native images that execute without the OxVba runtime host
- full direct AOT lowering for all VBA semantics
- claiming complete Office COM, XLL, or Windows registration parity from shim
  source generation alone
- replacing VM fallback with a JIT-only execution contract

## Rationale

`oxvba-build` already owns wrapper source generation for executable, DLL, COM
server, and XLL surfaces around embedded `.oxb` bundles. This is the nearest
shipping-shaped native artifact path because it can be validated as packaging
and ABI surface work without pretending every VBA semantic has been lowered to
machine code.

`oxvba-jit` already owns a Cranelift execution lane, but its public execution
contract is subset support plus fallback. The engine attempts the retained
RtSlot Cranelift path, then the legacy i32 subset, then the VM interpreter.
That makes Cranelift an accelerator, not the V0.2 product packaging boundary.

Direct native image output is deferred because it needs separate closure on:

- runtime value, `VARIANT`, `BSTR`, and `SAFEARRAY` ownership
- COM and Office host ABI contracts
- exception and `Err` state semantics
- callback, object lifetime, and registration behavior
- backend coverage and target-triple support
- artifact provenance and deployment validation

## Acceptance Gates

The remaining `bd-bqm8.10` beads must provide:

- ABI, packaging, platform, artifact, and deployment obligations for the
  wrapper-hosted path
- an executable validation scaffold that exercises the selected path
- final evidence showing that product claims match the runnable scaffold and
  residual boundaries
