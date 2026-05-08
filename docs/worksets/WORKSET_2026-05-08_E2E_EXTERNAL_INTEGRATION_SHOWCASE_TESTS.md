# Workset — E2E External Integration Showcase Tests

Date: 2026-05-08
Status: evidence-backed showcase pass complete for bounded mixed/late-bound scenarios; strict natural early-bound Access/JET test is red and tracked

## Objective

Create reproducible end-to-end showcase passes for OxVba through external integration interfaces, then collate the results into a single-page HTML tour suitable for a technical executive review.

## Scope

This workset covers a bounded live showcase, not a full parity closure claim:

1. Create a new single `.bas` file, compile it to `.oxb`, run the source path, run the compiled bundle with `oxvba-run`, and verify logs/artifacts/output.
2. Create a new two-file `.basproj` project with modules referencing each other, run it through VM and JIT-enabled targets, build a bundle, run the bundle, and verify output.
3. Introduce deliberate editing errors and verify diagnostics and non-zero command results.
4. Create a new Access/ACE/Jet database integration project with COM reference metadata for ADO/ADOX, activate real Windows COM providers, create an `.accdb`, insert records, query them back, and show returned scalar results.
5. Create a mixed imported-COM Access/ACE/Jet database integration project that imports real ADO/ADOX type libraries, declares `ADOX.Catalog` and `ADODB.Connection` with `As New`, drives supported metadata-backed `ADODB.Connection.Open/Execute` member calls, still uses `DispatchInvoke` for unsupported pieces, and checks the queried values.
6. Add a strict natural early-bound Access/ACE/Jet test using only typed imported COM declarations/member calls for the database flow. This is currently a red ignored test and is the implementation target for the real early-bound lane.
7. Exercise the bounded Immediate Window interface over the project and capture the transcript.
8. Generate a single-page HTML report with truthful technical explanation, boundaries, logs, and artifact links.

## Reproducible Command

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/run-e2e-showcase.ps1
```

The script builds the CLI and launcher, creates fresh sources under a timestamped evidence directory, runs all passes, writes JSON summary and logs, and generates `showcase.html`.

## Latest Evidence

Latest successful run:

- Summary JSON: `docs/evidence/showcase/e2e_external_interfaces_20260508T174518/summary.json`
- Single-page HTML showcase: `docs/evidence/showcase/e2e_external_interfaces_20260508T174518/showcase.html`
- Sources: `docs/evidence/showcase/e2e_external_interfaces_20260508T174518/sources/`
- Logs: `docs/evidence/showcase/e2e_external_interfaces_20260508T174518/logs/`
- Output artifacts: `docs/evidence/showcase/e2e_external_interfaces_20260508T174518/artifacts/`

Run result: 14/14 pass for the showcase runner, including the strict natural-source Access/JET early-bound slice.

## Truth Boundaries

- This workset does not claim direct native AOT compilation. Current executable truth remains bytecode plus VM/JIT/fallback behavior and serialized OxBundle execution.
- The Access/Jet lane is environment-dependent. It requires installed Windows COM providers for ADOX/ADODB and Microsoft ACE OLE DB. The runner records this as blocked if providers are absent rather than fabricating success.
- The late-bound COM database lane uses the currently supported `CreateObject` / `DispatchInvoke` bridge plus `.basproj` COM reference metadata. It does not claim full Office/VBA COM parity.
- The mixed imported-COM database lane uses imported ADO/ADOX typelib metadata for activation and supported member calls. It still uses `DispatchInvoke` for the ADOX `Catalog.Create` and returned-recordset field/value traversal pieces. It is not a true early-bound VBA COM end-to-end test.
- The strict early-bound Access/JET test is `strict_early_bound_project_executes_registered_access_jet_ado_database_subset` in `crates/oxvba-host/tests/com_early_project_end_to_end.rs`. It now runs by default on Windows and passes on the current machine for the bounded natural-source ADO/ADOX slice, including `ADODB.Recordset.Fields("Name")`, `ADODB.Field.Value`, and `rs!Name`/`rs!Score` value-context shorthand.
- The Immediate Window pass covers the bounded V1 interface: procedure invocation/value printing, module retargeting, reset, and transcript behavior.

## Completion Evidence

The latest evidence run proves:

- CLI build produced `target/debug/oxvba-cli.exe` and `target/debug/oxvba-run.exe`.
- Single-file compile created `HelloSlots.oxb` and source execution returned `VALUES:i32:42|string:"single-file-ok"`.
- The compiled single-file bundle executed through `oxvba-run` and emitted slot output containing `Long`.
- Two-module project VM and JIT-enabled runs returned `VALUES:i32:42|i32:84|string:"two-module=84"`.
- Project bundle build created `ShowcaseTwoModule.oxb` and the launcher accepted it.
- Broken assignment and missing module reference both produced non-zero process exits with captured diagnostics.
- Access/ACE/Jet late-bound COM project created `ShowcaseJet.accdb` (184,320 bytes in the latest run), inserted Ada/Grace rows, queried Grace back, and surfaced `string:"Grace"|i32:99`.
- Access/ACE/Jet mixed imported-COM project created `ShowcaseJetEarlyBound.accdb` (184,320 bytes in the latest run), imported `msado15.dll` and `msadox.dll`, declared `ADOX.Catalog` / `ADODB.Connection` with `As New`, executed `ADODB.Connection.Open/Execute` through metadata-backed calls, used `DispatchInvoke` for unsupported pieces, and surfaced `string:"Grace"|i32:99`.
- Strict natural early-bound command now passes: `cargo test -p oxvba-host --test com_early_project_end_to_end strict_early_bound_project_executes_registered_access_jet_ado_database_subset -- --nocapture`.
- Immediate Window transcript showed module query, `? MathHelpers.Add(5, 7)`, `? Scale(9)`, retargeting to `MathHelpers`, `? Add(100, 23)`, and `reset`.
