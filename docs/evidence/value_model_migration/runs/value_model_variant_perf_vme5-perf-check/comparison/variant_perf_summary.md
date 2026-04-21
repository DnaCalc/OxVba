# Value Model Variant Performance Run

- Run ID: vme5-perf-check
- Baseline ref: pre-value-model-migration-2026-04-20
- Baseline commit: dd1c295b2a3d3a1530dd034d9bb4a6b4c38ea57a
- Candidate ref: HEAD
- Candidate commit: b63ad7d93f02953e44c1198eb9464f0696ff17f9
- Iterations: 1
- Workload manifest: C:\Work\DnaCalc\OxVba\docs\evidence\value_model_migration\runs\value_model_variant_perf_vme5-perf-check\generated\workload_manifest.csv

| Workload | Baseline ms | Candidate ms | Delta ms | Delta % |
|---|---:|---:|---:|---:|
| scalar_classifier | 842.15 | 2424.01 | 1581.86 | 187.84 |
| numeric_classifier | 646.09 | 735.19 | 89.1 | 13.79 |
| typed_array_results | 589.84 | 550.97 | -38.87 | -6.59 |
| typed_decimal_array_results | 612.09 | 581.86 | -30.23 | -4.94 |
| object_results | 584.56 | 769.14 | 184.58 | 31.58 |
| wide_i64_array_boundary | 541.17 | 642.89 | 101.72 | 18.8 |
| variant_matrix_results | 681.12 | 931.22 | 250.1 | 36.72 |
