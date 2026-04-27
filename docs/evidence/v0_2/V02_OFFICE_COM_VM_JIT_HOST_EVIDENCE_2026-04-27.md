# V0.2 Office COM VM/JIT/Host Evidence Refresh

Date: 2026-04-27

Bead: `bd-bqm8.7.5`

## Scope

This evidence refresh ties the Office COM corpus matrix and fixture packs back to
active VM/JIT/host coverage. Controlled OxVba COM rows are live-executed. Real
Excel and Access/JET rows are fixture-compiled by default and remain
environment-dependent for live Office/provider execution.

## Evidence Results

| Surface | Command | Result |
| --- | --- | --- |
| Office fixture compiler gates | `cargo test -p oxvba-host formal_v02_7_ -- --nocapture` | pass: 2 passed |
| Controlled late-bound COM VM/JIT/host lane | `cargo test -p oxvba-host --test com_client_end_to_end -- --nocapture` | pass: 68 passed |
| Controlled early/type-library COM lane | `cargo test -p oxvba-host --test com_early_project_end_to_end -- --nocapture` | pass: 119 passed |
| Registered COM smoke lane | `cargo test -p oxvba-host --test com_client_registered_lane -- --nocapture` | pass: 13 passed |

## Matrix Mapping

| Corpus rows | Evidence status |
| --- | --- |
| `OFFICE-COM-001` and `OFFICE-COM-002` | Live controlled COM execution covered by `com_client_end_to_end`; VM/JIT parity rows pass. |
| `OFFICE-COM-003` and `OFFICE-COM-004` | Controlled early/type-library and TestEventServer rows covered by `com_early_project_end_to_end` plus registered smoke coverage. |
| `OFFICE-COM-005` and `OFFICE-COM-006` | Existing Excel oracle scripts/captures remain the authority; not rerun here because they require Excel/VBOM environment prerequisites. |
| `OFFICE-COM-007` through `OFFICE-COM-010` | Excel corpus fixtures compile through `formal_v02_7_excel_com_fixture_pack_exists_and_compiles`; live Excel execution remains environment-dependent. |
| `OFFICE-COM-011` through `OFFICE-COM-014` | Access/JET corpus fixtures compile through `formal_v02_7_access_jet_com_fixture_pack_exists_and_compiles`; live Access/ACE/JET execution remains environment-dependent. |
| `OFFICE-COM-015` | Out-of-scope V0.2; no capability claim. |

## Residual Classification

- Missing Excel, VBOM, Access, ACE, DAO, ADODB, or JET provider dependencies are
  environment skips for V0.2, not default CI failures.
- Real Excel application event sink parity remains `unsupported-v02` outside the
  controlled TestEventServer event lane.
- Non-Windows Office COM parity remains `unsupported-v02`.
- Office forms designer/UI designer behavior remains out of scope for this
  corpus.

## Result

`bd-bqm8.7.5` is complete for refreshed Office COM VM/JIT/host evidence. The
Office COM corpus lane remains in-progress pending the final checklist bead.
