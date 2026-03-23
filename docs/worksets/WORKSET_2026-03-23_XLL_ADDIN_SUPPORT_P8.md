# WORKSET: Phase 8 — XLL/Addin Support

**Date:** 2026-03-23
**Phase:** 8
**Status:** Planned
**Depends on:** Phase 7 (DLL wrapper builder), Phase 4 (native export type info), Phase 5 (Engine::invoke_procedure)

---

## Objective

Extend the DLL wrapper builder to support `OutputType=Addin` with XLL-specific entry points (`xlAutoOpen`, `xlAutoClose`, `xlfRegister`) and XLOPER12 marshaling for Excel integration.

---

## Deliverables

### 1. XLL entry point generation in `crates/oxvba-build/src/xll.rs` (new)

- `xlAutoOpen` — registers all declared native exports with Excel via `xlfRegister`
- `xlAutoClose` — unregisters functions and cleans up runtime
- `xlAutoFree12` — frees XLOPER12 memory allocated by the add-in

### 2. XLOPER12 marshaling layer

Separate from core NativeExport marshaling:

- `RuntimeValue → XLOPER12` for return values
- `XLOPER12 → RuntimeValue` for incoming arguments

Type mapping:

| DeclareParamType | XLOPER12 type | C type |
|-----------------|--------------|--------|
| Long/Integer/Byte | `xltypeInt` | `int` |
| Double/Single/Currency/Date | `xltypeNum` | `double` |
| Boolean | `xltypeBool` | `BOOL` |
| String | `xltypeStr` | `XCHAR*` (length-prefixed wide) |
| Variant | `xltypeMulti` or contextual | varies |

### 3. Function registration metadata

Each `NativeExport` item generates an `xlfRegister` call with:

- Function text (exported name)
- Type string (Excel type codes: `"BBB"` for `Double(Double, Double)`)
- Category, argument descriptions (optional metadata from `.basproj`)

### 4. Addin-specific `.basproj` metadata (optional extension)

```xml
<NativeExport Include="CalcBlackScholes">
  <Module>PricingFunctions</Module>
  <Procedure>BlackScholes</Procedure>
  <CallingConvention>Stdcall</CallingConvention>
  <Category>Financial</Category>
  <Description>Calculate Black-Scholes option price</Description>
  <ArgumentDescriptions>Spot price;Strike price;Time to expiry</ArgumentDescriptions>
</NativeExport>
```

---

## Key Existing Code

- No XLL-specific code exists in the codebase currently
- `crates/oxvba-com/src/typelib_catalog.rs` — Excel.Application typelib support (for Application object bridge)
- `crates/oxvba-build/src/dll.rs` (from Phase 7) — base DLL shim generation

---

## Files to Modify/Create

| File | Change |
|------|--------|
| `crates/oxvba-build/src/xll.rs` (new) | XLL shim generation: xlAutoOpen, xlAutoClose, registration |
| `crates/oxvba-build/src/xloper.rs` (new) | XLOPER12 type definitions and marshaling |
| `crates/oxvba-build/src/lib.rs` | Add xll/xloper modules |
| `crates/oxvba-project/src/model.rs` | Optional addin metadata fields on NativeExport (Category, Description, ArgumentDescriptions) |

---

## Execution Steps

1. Define XLOPER12 struct and related types in `xloper.rs` (mirror Excel SDK headers)
2. Implement XLOPER12 ↔ RuntimeValue marshaling for each DeclareParamType
3. Implement `generate_xll_shim` — extends DLL shim with xlAutoOpen/xlAutoClose/xlAutoFree12
4. Implement function registration code generation — for each export, emit xlfRegister call with type string
5. Implement Excel type code mapping: `DeclareParamType → Excel type letter` (B=Double, J=Int, C=String, etc.)
6. Add optional addin metadata to `.basproj` model
7. Integration test: generate XLL source for a simple function, verify it compiles

---

## Closure Conditions

1. `generate_xll_shim` produces compilable source with xlAutoOpen/xlAutoClose
2. Function registration covers all 13 DeclareParamType → Excel type code mappings
3. XLOPER12 marshaling handles numeric, string, and boolean types
4. Optional metadata (Category, Description) is passed through to registration
5. Generated XLL source compiles without errors
