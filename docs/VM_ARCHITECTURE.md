# VM_ARCHITECTURE.md

## Current state

The VM crate (`oxvba-vm`) provides a register-slot interpreter with full VBA instruction coverage:

### Instruction set (152 instructions)

The compiler (`oxvba-compiler/src/bytecode.rs`) defines 152 instruction variants across these categories:

- **Load / arithmetic / data movement (15):** `LoadConstI32`, `LoadConstString`, `LoadConstF64`, `AddConstI32`, `AddSlots`, `SubSlots`, `MulSlots`, `DivSlots`, `IntDivSlots`, `ModSlots`, `PowSlots`, `ConcatSlots`, `NegSlot`, `CopySlot`, `IncSlot`
- **Comparisons (6):** `CmpEqSlots`, `CmpNeSlots`, `CmpLtSlots`, `CmpLeSlots`, `CmpGtSlots`, `CmpGeSlots`
- **Boolean composition (3):** `BoolNot`, `BoolAnd`, `BoolOr`
- **Control flow (6):** `CallProc`, `Return`, `JumpIfZero`, `Jump`, `Halt`, `LoadNull`
- **Error handling (11):** `SetOnErrorResumeNext`, `SetOnErrorGoto0`, `SetOnErrorGotoLabel`, `ResumeNext`, `Resume`, `ResumeLabel`, `RaiseError`, `ClearErr`, `LoadErrNumber`, `LoadErrDescription`, `LoadErrSource`
- **String intrinsics (17):** `Len`, `Left`, `Right`, `Mid`, `MidStmt`, `InStr`, `InStrRev`, `LCase`, `UCase`, `Split`, `Join`, `Replace`, `Trim`, `LTrim`, `RTrim`, `StrComp`, `Like`
- **Date/time intrinsics (13):** `DateSerial`, `TimeSerial`, `DateValue`, `TimeValue`, `DateAdd`, `DateDiff`, `Year`, `Month`, `Day`, `DateNow`, `TimeNow`, `Now`, `Timer`
- **Math / conversion intrinsics (24):** `Abs`, `Int`, `Fix`, `Sgn`, `Round`, `Sqr`, `Sin`, `Cos`, `Log`, `Exp`, `Atn`, `Tan`, `Chr`, `Asc`, `Space`, `StringRepeat`, `Hex`, `Oct`, `StrConv`, `Rnd`, `Randomize`, `Format`, `StrReverse`, `Weekday`, `MonthName`
- **Financial intrinsics (8):** `Fv`, `Pv`, `Pmt`, `Npv`, `Irr`, `Mirr`, `Rate`, `NPer`
- **Array / type-checking intrinsics (14):** `ArrayLiteral`, `LBound`, `UBound`, `IsArray`, `VarType` (tag/value), `TypeName`, `IsNumeric` (tag/value), `IsError`, `IsDate`, `IsObject`, `IsNull`, `IsEmpty`
- **Host I/O intrinsics (16):** `FileOpen`, `FileClose`, `FileRead`, `FileWrite`, `FilePrint`, `FileInput`, `FileLineInput`, `FileLoc`, `FreeFile`, `MsgBox`, `InputBox`, `DoEvents`, `Shell`, `Environ`, `Dir`
- **COM / object intrinsics (12):** `CreateObject`, `DispatchInvoke`, `ComSubscribeEvent`, `ComUnsubscribeEvent`, `ComEventCallbackSubscription`, `ComEventCallbackArg`, `ComReleaseEventCallback`, `CollectionAdd`, `CollectionItem`, `CollectionRemove`, `CollectionCount`, `InvokeSymbol`
- **WithEvents intrinsics (5):** `WithEventsGet`, `WithEventsSet`, `WithEventsClearOwner`, `WithEventsFirstOwner`, `WithEventsNextOwner`
- **Assignment validation (1):** `ValidateRuntimeAssignment`

All 152 instructions are implemented in both the interpreter and the JIT backend (155 JIT mapping entries, covering all branches).

### Retained Variant Execution Model

The interpreter operates on retained `Variant` slots defined in
`oxvba-runtime`. `Variant` is the canonical execution and snapshot carrier for
current VM/JIT/host coordination.

A flat `RegisterFile` (vector of `RuntimeSlot` entries backed by `Variant`,
initially 256 and dynamically resized) provides shared register storage.

Important current carrier truth:

- `Variant` stores strings through `BStr`, not plain `String`
- `Variant` stores object identity through `ObjectRef`, whose base object exposes a
  runtime `IUnknown`-style vtable and reference counting
- `RuntimeValue` is now a compatibility/projection carrier for older APIs and
  selected tests; it is not VM register storage or conformance truth
- legacy compat-slot `i32` projections are not an accepted VM observation model
  and must not be used for new conformance gates
- retained `VALUES:` snapshots are the basic-language conformance oracle

### Package-oriented execution entry

The VM keeps bytecode-only APIs for compatibility, but also exposes
`VmExecutionPackage`: a borrowed package view over `Bytecode` plus
`ProcedureRuntimeMetadata`. Package execution and package snapshot helpers load
procedure metadata before running the same interpreter path. This is the first
small VM-side step toward the executable semantic package boundary shared by
future VM/JIT differential harnesses.

### Call stack and register-window frames

Procedure calls use a `Vec<(usize, ErrorFrame)>` call stack:
- `CallProc` saves the return PC and current error frame, clears local error state, and jumps to the target.
- `Return` pops the call stack, restoring the caller's error frame and return address.

Each procedure activation isolates its error state through an `ErrorFrame` carrying: `on_error_resume_next`, `on_error_goto_label_target`, `last_error`, `last_error_pc`, `last_error_description`, and `last_error_source`.

### Semantics module

Pure semantic functions extracted to `crate::semantics` (~560 lines), covering:
- Type coercion and checks (null/error propagation, f64 conversion, truthiness)
- Arithmetic, division with VBA error codes, comparison, negation
- Assignment validation and formatting
- COM object coercion and WithEvents key helpers

Both the interpreter and JIT runtime helpers share these functions.

### Error handling

Full VBA error handling is implemented:
- `On Error Resume Next` / `On Error GoTo 0` / `On Error GoTo <label>`
- `Resume` / `Resume Next` / `Resume <label>`
- `Err.Raise` / `Err.Clear` / `Err.Number` / `Err.Description` / `Err.Source`
- Error state is isolated per procedure frame.

## Feature flags

- `mach_broadword_dispatch` (crate: `oxvba-vm`): placeholder for SWAR/broadword opcode dispatch optimization. Currently returns `false` (no-op). Includes Kani verification proofs.
