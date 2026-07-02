# vm3 AddressOf Native Callback Thunk Evidence - 2026-07-02

Bead: `bd-9sed.6.1`

## Scope

This closes the bounded synchronous VBA runtime shape:

```vb
Private Declare PtrSafe Function RiffCallPtr4 Lib "user32" Alias "CallWindowProcW" ( _
    ByVal lpPrevWndFunc As LongPtr, _
    ByVal a0 As LongPtr, _
    ByVal a1 As LongPtr, _
    ByVal a2 As LongPtr, _
    ByVal a3 As LongPtr) As LongPtr

ignored = RiffCallPtr4(AddressOf RiffTimerLikeCallback, 11, 22, 33, 44)
```

vm3 now replaces the `AddressOf` `ProcRef` argument with a scoped native thunk address,
lets Windows call that thunk through `CallWindowProcW`, and re-enters the declared VBA
procedure with the four callback arguments typed from the callback declaration.

This is a VBA runtime compatibility target, not vm2 compatibility. Unsupported platforms
and non-synchronous native callback descriptors still fail explicitly rather than
marshalling a `ProcRef` as an integer. Async `SetTimer`/message-pump callback lifetime is
not closed by this slice.

## Implementation

- `crates/oxvba-runtime/src/callback_thunks.rs` adds a 32-slot thread-local Windows x64
  callback table with generated `extern "system"` thunks, scoped registrations, slot reuse
  by `(owner, proc_token)`, and panic containment across the native ABI boundary.
- `crates/oxvba-vm3/src/lib.rs` detects by-value `AddressOf` arguments passed to `LongPtr`
  Declare parameters for the synchronous `CallWindowProcW` descriptor, registers a scoped
  thunk for the duration of the native call, and dispatches the callback back into the
  target procedure.
- `crates/oxvba-host/tests/native_declare_lane_vm3.rs` now asserts vm3 runs the callback
  shape and observes all four arguments.
- `crates/oxvba-host/tests/native_declare_lane.rs` un-ignores the shared Riff-shaped
  native Declare probe.

## Verification

Passed locally on Windows:

```powershell
cargo fmt --all
cargo check -p oxvba-runtime -p oxvba-vm3 -p oxvba-host
cargo test -p oxvba-host --test native_declare_lane_vm3 -- --test-threads=1 --nocapture
cargo test -p oxvba-host --test native_declare_lane riff_shaped_callwindowproc_invokes_address_of_callback -- --exact --test-threads=1 --nocapture
cargo test -p oxvba-host --test native_declare_lane -- --test-threads=1
cargo clippy -p oxvba-runtime -p oxvba-vm3 -p oxvba-host --all-targets -- -D warnings
```

`scripts\meta-check.ps1 -Fast -NoArtifacts` was attempted twice. Both runs reached
`cargo test --workspace` and timed out while the unrelated `oxvba-build` unit test
`tests::wrapped_com_server_build_emits_package_descriptor_and_idl` remained stuck in the
`oxvba_build` test binary. Running that single test directly also timed out after five
minutes. The focused callback lane checks above passed.
