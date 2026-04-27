# V0.2 Native Compilation Scaffold Evidence

Date: 2026-04-27

Bead: `bd-bqm8.10.4`

## Result

Added the first executable V0.2 native-compilation validation scaffold:

- Script:
  [run-v02-native-scaffold.ps1](/C:/Work/DnaCalc/OxVba/scripts/run-v02-native-scaffold.ps1)
- Obligation matrix:
  [V02_NATIVE_COMPILATION_OBLIGATIONS_V1.csv](/C:/Work/DnaCalc/OxVba/docs/validation/V02_NATIVE_COMPILATION_OBLIGATIONS_V1.csv)

The scaffold validates wrapper-hosted native path evidence without claiming
standalone native-image or full AOT parity.

## Local Validation

Command:

```powershell
./scripts/run-v02-native-scaffold.ps1 -NoArtifacts -RunId v02-native-scaffold-check
```

Result:

- status: pass
- rows: 4
- failed rows: 0
- CSV artifact:
  `temp/no-artifacts/v02-native-scaffold/v02-native-scaffold-check/V02_NATIVE_SCAFFOLD_RUN_v02-native-scaffold-check.csv`
- Markdown artifact:
  `temp/no-artifacts/v02-native-scaffold/v02-native-scaffold-check/V02_NATIVE_SCAFFOLD_RUN_v02-native-scaffold-check.md`

Gate rows:

| Gate ID | Area | Obligations | Status |
| --- | --- | --- | --- |
| NATIVE-V02-G001 | wrapper source generation | NATIVE-V02-O001; NATIVE-V02-O002; NATIVE-V02-O004; NATIVE-V02-O005; NATIVE-V02-O006; NATIVE-V02-O008 | pass |
| NATIVE-V02-G002 | JIT supported subset | NATIVE-V02-O003; NATIVE-V02-O007 | pass |
| NATIVE-V02-G003 | JIT VM fallback | NATIVE-V02-O003; NATIVE-V02-O007 | pass |
| NATIVE-V02-G004 | artifact provenance | NATIVE-V02-O010 | pass |

## Residual Boundary

This bead provides the executable scaffold. It does not close the native lane
until the final checklist verifies the decision, obligation matrix, scaffold
artifacts, governance checks, and product-claim boundaries together.
