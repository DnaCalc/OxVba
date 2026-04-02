# Build Target And Wrapper Boundary v1

Status: `draft`
Date: 2026-04-03
Scope owner: OxVBA project/build system
Canonical path: `docs/spec/BUILD_TARGET_AND_WRAPPER_BOUNDARY_V1.md`

Related docs:
- `docs/spec/BASPROJ_SPEC_V1.md`
- `docs/worksets/WORKSET_2026-04-02_WRAPPER_BUILD_TARGET_AND_NATIVE_HOSTING_EXECUTION.md`

---

## 1. Purpose

Define the explicit boundary between:
- semantic project shape, selected by `.basproj` `OutputType`
- physical packaging/build shape, selected by `.basproj` `BuildTarget`

This separation is required so wrapper/native-hosting lanes do not overload semantic project meaning.

---

## 2. Canonical Rules

1. `.oxb` remains the canonical compiled semantic artifact.
2. `OutputType` controls semantic project behavior.
3. `BuildTarget` controls emitted packaging shape.
4. Wrapper outputs are packaging layers over a canonical `.oxb` payload.
5. Wrapper builders consume existing project/runtime/export metadata; they do not reinterpret VBA semantics independently.

---

## 3. BuildTarget Values

| BuildTarget | Meaning | Current expectation |
|-------------|---------|---------------------|
| `Bundle` | Canonical OxVBA bundle artifact | emits `.oxb`; current stable default |
| `WrapperExe` | Native executable wrapper over a canonical `.oxb` payload | planned delivery lane |
| `WrapperLibrary` | Native DLL/shared-library wrapper over a canonical `.oxb` payload | planned delivery lane |

Default: `Bundle`

---

## 4. Boundary Contract

The wrapper boundary must receive enough information from the canonical OxVBA side to package a hostable artifact without reconstructing semantic meaning:

- project semantic kind (`OutputType`)
- build packaging kind (`BuildTarget`)
- canonical compiled bundle payload
- startup/entry metadata when applicable
- native export descriptors when applicable
- project/runtime policy metadata required at launch
- reference metadata needed for deterministic host bootstrap

This contract intentionally keeps:
- compiler/runtime semantics in the existing OxVBA core
- packaging mechanics in wrapper/native-hosting lanes

---

## 5. Non-Goals

This spec does not by itself define:
- EXE wrapper binary layout
- DLL/shared-library ABI/export layout
- COM server registration details
- XLL entrypoint layout

Those are downstream delivery lanes built on this boundary.
