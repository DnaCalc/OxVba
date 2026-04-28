# XLL Scalar Addin Staging

Date: 2026-04-28
Bead: `bd-xll1.5.4`
Workset: `docs/worksets/WORKSET_2026-04-28_XLL_EXCEL_HOST_VALIDATION_EXECUTION.md`

## Scope

Added a repeatable staging script for the deterministic XLL scalar Addin
fixture:

```powershell
./scripts/stage-xll-scalar-addin.ps1
```

The script builds `examples/xll/scalar_addin` through the normal
`OutputType=Addin` path and stages:

- copied source fixture files,
- build transcript,
- final `.xll` artifact,
- JSON manifest with command, timestamps, artifact size, and SHA-256 hash.

## Validation Run

Command:

```powershell
./scripts/stage-xll-scalar-addin.ps1 -RunId 20260428T000000Z
```

Result:

```text
built examples/xll/scalar_addin -> target\xll-host-validation\scalar_addin\20260428T000000Z\ScalarAddin.xll (2282496 bytes)
xll scalar addin staged: target\xll-host-validation\scalar_addin\20260428T000000Z
artifact: target\xll-host-validation\scalar_addin\20260428T000000Z\ScalarAddin.xll (2282496 bytes)
manifest: target\xll-host-validation\scalar_addin\20260428T000000Z\manifest.json
```

Manifest summary:

- artifact: `target/xll-host-validation/scalar_addin/20260428T000000Z/ScalarAddin.xll`
- bytes: `2282496`
- SHA-256: `7876E957B5D2E30379731FBECE8A21076AD7A84F7382AC32622C9A215BC272A4`
- Excel-host validated: `false`

## Boundary

This is still local package staging evidence. It does not prove that Excel can
load the add-in, call `xlAutoOpen`, complete `xlfRegister`, or invoke worksheet
functions. Those are owned by the next host beads.
