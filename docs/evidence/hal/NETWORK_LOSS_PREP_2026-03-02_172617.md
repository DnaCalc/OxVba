# Network Loss Prep Snapshot

Date: 2026-03-02 17:26:17 (local)
Branch: `master`
HEAD: `0ffd1e2`

## Current Local Work In Progress

Modified:
- `docs/evidence/hal/HAL_IMPLEMENTATION_DEFINED.md`
- `docs/evidence/hal/HAL_UNCERTAINTY_REGISTER.md`
- `docs/evidence/hal/README.md`
- `docs/spec/HAL_CONFORMANCE_SUITE.md`
- `docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.csv`
- `docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md`
- `docs/spec/HAL_DECLARE_ABI_SPEC_V1.md`
- `docs/spec/HAL_DECLARE_EXECUTION_IMPLEMENTATION_V2.md`
- `docs/spec/HAL_FORMALIZATION_PROGRAM.md`
- `docs/spec/HAL_SPEC_CROSSWALK.md`
- `docs/spec/HAL_SPEC_WORKING_DRAFT.md`
- `docs/spec/README.md`

Untracked:
- `docs/evidence/hal/HAL_DECLARE_MARSHAL_AMBIGUITIES_2026-03-02.md`
- `docs/spec/HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md`
- `temp/`

## Last Completed Checks

- `./scripts/check-hal-clause-drift.ps1` passed
- `cargo test -p oxvba-hal conformance_catalog_and_markdown_clause_ids_match` passed
- `cargo test -p oxvba-hal conformance_catalog_scoped_coverage_is_available` passed

## Resume Commands (Post-Network)

```powershell
cd C:\Work\DnaCalc\OxVba

git status --short
./scripts/check-hal-clause-drift.ps1
cargo test -p oxvba-hal conformance_catalog_and_markdown_clause_ids_match
cargo test -p oxvba-hal conformance_catalog_scoped_coverage_is_available
```

## Optional Safety Commit (if needed before any risky operation)

```powershell
git add docs/spec/HAL_DECLARE_ABI_SPEC_V1.md docs/spec/HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md docs/spec/HAL_SPEC_CROSSWALK.md docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.csv docs/spec/HAL_DECLARE_EXECUTION_IMPLEMENTATION_V2.md docs/spec/HAL_CONFORMANCE_SUITE.md docs/spec/HAL_FORMALIZATION_PROGRAM.md docs/spec/HAL_SPEC_WORKING_DRAFT.md docs/spec/README.md docs/evidence/hal/HAL_DECLARE_MARSHAL_AMBIGUITIES_2026-03-02.md docs/evidence/hal/HAL_UNCERTAINTY_REGISTER.md docs/evidence/hal/HAL_IMPLEMENTATION_DEFINED.md docs/evidence/hal/README.md

git commit -m "spec(hal): formalize declare+marshaling contracts and conformance lanes"
```