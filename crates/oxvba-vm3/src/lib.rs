//! `oxvba-vm3` — the typed-CFG interpreter of OxIR.
//!
//! vm3 is a fresh **executor core** (typed register file + place model,
//! block-threaded dispatch, frame/linkage + ByRef-aliasing, error/Resume routing,
//! object-lifecycle / Terminate-drain timing, RaiseEvent/WithEvents, COM-event pump)
//! re-expressed against OxIR's typed basic-block CFG. It does **not** re-implement
//! VBA: it reuses the value/interop/lib substrate (`oxvba-runtime`, `oxvba-lib`,
//! `oxvba-hal`, `oxvba-com`/`oxvba-comhost`) and the shared `oxvba-eval` semantic
//! kernel — refactoring upstream where that improves the whole.
//!
//! vm3 is OxIR's **executable specification**: its observable behaviour defines what
//! OxIR means, and the Cranelift JIT must match it. During the transition, the
//! legacy `oxvba-vm2` (`Op` bundle) remains the **golden oracle** until vm3 reaches
//! full-corpus parity (the "oracle handoff"), after which vm2 is frozen.
//!
//! Executes a typed CFG with a typed register file plus a boxed-`Variant` fallback;
//! typed-scalar ops touch unboxed lanes, dynamic ops box and call the kernel.
