# WrappedComServer Excel early-bound COM-0008 evidence

Date: 2026-06-17
Bead: `bd-9h5j`
Matrix row: `COM-0008`

## Scope

This evidence covers the clean `WrappedComServer` generated TypeLib from an
Office/VBA client. The Excel smoke references the generated `.tlb` with
`VBProject.References.AddFromFile`, compiles typed VBA against the generated
`Calculator` class, and invokes the wrapped server through the generated
dispatch interface metadata.

This is not formula/UDF evidence.

## Command

```powershell
cargo test -p oxvba-build --test wrapped_com_server_smoke -- --ignored --nocapture
```

Result: passed on Windows with Excel installed.

## Verified behavior

- The smoke builds and registers a throwaway `OutputType=ComServer`,
  `BuildTarget=WrappedComServer` project with a creatable `Calculator` class and
  generated TypeLib.
- Excel opens a workbook and adds the generated TypeLib as a VBA project
  reference through `VBProject.References.AddFromFile`.
- The injected VBA module compiles against the generated `Calculator` type.
- The VBA client creates `Dim calc As Calculator` / `Set calc = New Calculator`.
- Early-bound method invocation works: `calc.Add(20, 22)` returns `42`.
- Early-bound property put/get works: `calc.Value = 123` and `calc.Value`
  returns `123`.
- Early-bound object-return invocation works: `Set returned = calc.ReturnSelf()`
  returns an object whose `Add(3, 4)` result is `7`.
- Early-bound array-return invocation works: `calc.Numbers()` returns an array
  whose first two elements are `7` and `8`.
- Early-bound error propagation reaches VBA as external Automation error `440`
  when the wrapped member raises `Err.Raise 5`.
- The same workbook still proves typed `WithEvents` subscription through the
  generated source interface and observes `Changed(77)`.

## Residual

`COM-0008` remains an implemented subset. This Excel/VBA evidence covers typed
project-reference calls through the generated dispatch interface, but not broken
or missing reference repair behavior, broader Office version matrices, richer
Excel-facing error description parity, or vtable/dual-interface calls from VBA.
