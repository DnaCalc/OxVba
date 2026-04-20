# Value Model Migration Report Input Index

This directory is the stable handoff surface for the final migration report.
Each file below maps directly to a required section in section 12 of the
workset.

| Section | File | Purpose |
|---|---|---|
| 12.1 | [01_EXECUTIVE_RESULT.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/01_EXECUTIVE_RESULT.md) | final pass/fail statement and authority-hierarchy outcome |
| 12.2 | [02_SCOPE_AND_BASELINE.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/02_SCOPE_AND_BASELINE.md) | baseline tag, migration scope, and authoritative references |
| 12.3 | [03_CORRECTNESS_RESULT.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/03_CORRECTNESS_RESULT.md) | old/new correctness matrix summary and divergences |
| 12.4 | [04_DISCRETIONARY_DECISIONS.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/04_DISCRETIONARY_DECISIONS.md) | decisions retained during migration and revisit triggers |
| 12.5 | [05_PERFORMANCE_AND_MEMORY_RESULT.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/05_PERFORMANCE_AND_MEMORY_RESULT.md) | timing, memory, and workload-family summaries |
| 12.6 | [06_FURTHER_MITIGATIONS.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/06_FURTHER_MITIGATIONS.md) | post-correctness optimization follow-ups |
| all | [LATEST_ARTIFACT_MAP.csv](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/LATEST_ARTIFACT_MAP.csv) | canonical pointers to current paired run artifacts |

Update policy:

1. harness beads add or refresh artifact pointers in `LATEST_ARTIFACT_MAP.csv`
2. migration beads update the relevant section placeholder with findings or
   deltas
3. the final report bead composes from these stable inputs rather than scanning
   `runs/` directly.
