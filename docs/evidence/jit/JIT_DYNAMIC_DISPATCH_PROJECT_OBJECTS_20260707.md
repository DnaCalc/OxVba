# JIT Dynamic Dispatch Project-Object Slice - 2026-07-07

Status: implemented-subset

This pass moves JIT dynamic project-object dispatch beyond the previous
compile-time unsupported boundary:

- `ComCallLate` now lowers for runtime `Variant`/`Object` receivers and resolves
  project instances through the existing descriptor-backed project member helper.
- `CallByName` lowers through a new JIT helper that evaluates receiver, member
  name, and call type at runtime, mapping `1/2/4/8` to method/get/let/set and
  seating runtime error 5 for invalid call types.
- The shared helper also has a foreign-object branch that builds the existing
  HAL/COM `DynamicCallRequest` and applies returned ByRef writebacks, but
  source-level JIT COM activation remains outside this evidence slice because
  `CreateObject` is still a separate unsupported JIT lowering boundary.

Coverage added:

- untyped `Dim c` project-class method dispatch;
- `Dim c As Object` project-class method dispatch;
- `CallByName` method and property get/let dispatch on project objects;
- invalid `CallByName` call type error 5.

Validation:

- `cargo fmt --check`
- `cargo test -p oxvba-jit -- --format terse`
- `cargo test -p oxvba-differential --test jit_project_objects -- --nocapture`
- `cargo test -p oxvba-host --test jit_m4_com_projection -- --nocapture`
- `git diff --check`

Residual:

- JIT source-level COM activation (`CreateObject`) and early-bound COM remain
  unsupported boundaries, so portable/live COM dispatch needs a later source-level
  entry test after activation lowering is available.
- Full EXCEPINFO/HRESULT parity for JIT-entered COM dispatch is prepared through
  the shared HAL `Fault::from_hal` path but is not claimed by this project-object
  evidence slice.
