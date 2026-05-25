# conformance/

Conformance assets comparing OxVBA behavior to Office VBA.

- `harness/`: observation harness inputs.
- `golden/`: expected retained-value outputs for OxVBA conformance lanes. The
  active basic-language runner uses semantic `VALUES:` output, not legacy
  integer slot dumps.
- `tests/`: source test cases executed in OxVBA.
- `tests_manifest.csv`: suite ownership for single-file fixtures. The
  `basic-language` suite is the default fast language gate; host/COM,
  known-failing, and value-oracle-pending rows remain tracked outside that gate.
- `divergences/`: divergence/regression fixtures (can be open or closed records).
- `integration/`: tracked multi-module/multi-project integration catalog + fixtures for `oxvba-host` project-manifest execution.

## Basic Language Gate

Run the default language-only corpus without host, COM, wrapper, or add-in
prerequisites:

```powershell
./scripts/run-conformance.ps1 -Backend vm
```

Host/COM-sensitive fixtures remain cataloged in `tests_manifest.csv` as
`host-or-boundary` and are validated by the focused host/COM lanes, for example:

```powershell
cargo test -p oxvba-com --quiet
cargo test -p oxvba-host --test project_integration_suite --quiet
```
