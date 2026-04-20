# Value Model String Performance Run

- Run ID: vmsp1-smoke-vm-small
- Baseline ref: pre-value-model-migration-2026-04-20
- Baseline commit: dd1c295b2a3d3a1530dd034d9bb4a6b4c38ea57a
- Candidate ref: HEAD
- Candidate commit: 892b9b35d642f75673a570b88eb3120b093ea6a9
- Iterations: 1
- Backends: vm
- Workload manifest: C:\Work\DnaCalc\OxVba\docs\evidence\value_model_migration\runs\value_model_string_perf_vmsp1-smoke-vm-small\generated_sources\workload_manifest.csv

| Backend | Workload | Baseline ms | Candidate ms | Delta ms | Delta % |
|---|---|---:|---:|---:|---:|
| vm | small_strings | 67644.43 | 62555.57 | -5088.86 | -7.52 |
