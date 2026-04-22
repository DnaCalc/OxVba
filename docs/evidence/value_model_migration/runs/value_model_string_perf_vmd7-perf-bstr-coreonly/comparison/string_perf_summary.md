# Value Model String Performance Run

- Run ID: vmd7-perf-bstr-coreonly
- Baseline ref: pre-value-model-migration-2026-04-20
- Baseline commit: dd1c295b2a3d3a1530dd034d9bb4a6b4c38ea57a
- Candidate ref: HEAD
- Candidate commit: 9fa9ab7776370444b8476141f7527f95ca7f5c06
- Iterations: 2
- Backends: vm, jit
- Workload manifest: C:\Work\DnaCalc\OxVba\docs\evidence\value_model_migration\runs\value_model_string_perf_vmd7-perf-bstr-coreonly\generated_sources\workload_manifest.csv

| Backend | Workload | Baseline ms | Candidate ms | Delta ms | Delta % |
|---|---|---:|---:|---:|---:|
| vm | small_strings | 55291.68 | 51858.58 | -3433.1 | -6.21 |
| vm | long_strings | 459.45 | 497.98 | 38.53 | 8.39 |
| vm | many_strings | 401.79 | 381.84 | -19.95 | -4.97 |
| vm | code_strings | 564.84 | 464.54 | -100.3 | -17.76 |
| jit | small_strings | 513.74 | 533.43 | 19.69 | 3.83 |
| jit | long_strings | 361.72 | 550.45 | 188.73 | 52.18 |
| jit | many_strings | 466.78 | 467.1 | 0.32 | 0.07 |
| jit | code_strings | 151756.02 | 111355.45 | -40400.57 | -26.62 |
