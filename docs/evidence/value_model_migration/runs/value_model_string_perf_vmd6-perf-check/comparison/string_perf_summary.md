# Value Model String Performance Run

- Run ID: vmd6-perf-check
- Baseline ref: pre-value-model-migration-2026-04-20
- Baseline commit: dd1c295b2a3d3a1530dd034d9bb4a6b4c38ea57a
- Candidate ref: HEAD
- Candidate commit: 48857b11e9d72a7571a9558925a339ffdb8f95ad
- Iterations: 1
- Backends: vm, jit
- Workload manifest: C:\Work\DnaCalc\OxVba\docs\evidence\value_model_migration\runs\value_model_string_perf_vmd6-perf-check\generated_sources\workload_manifest.csv

| Backend | Workload | Baseline ms | Candidate ms | Delta ms | Delta % |
|---|---|---:|---:|---:|---:|
| vm | small_strings | 123000.67 | 102324.21 | -20676.46 | -16.81 |
| vm | medium_strings | 516.75 | 907.12 | 390.37 | 75.54 |
| vm | long_strings | 420.29 | 702.15 | 281.86 | 67.06 |
| vm | many_strings | 394.68 | 725.59 | 330.91 | 83.84 |
| vm | code_strings | 537.47 | 800.41 | 262.94 | 48.92 |
| jit | small_strings | 514.19 | 656.39 | 142.2 | 27.66 |
| jit | medium_strings | 391.5 | 594.44 | 202.94 | 51.84 |
| jit | long_strings | 412.59 | 482.84 | 70.25 | 17.03 |
| jit | many_strings | 405.25 | 586.99 | 181.74 | 44.85 |
| jit | code_strings | 140712.53 | 140987.04 | 274.51 | 0.2 |
