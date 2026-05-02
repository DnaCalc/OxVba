# Native-Ready Umbrella Completion Audit

> Superseded for current planning truth by
> [`NATIVE_READY_RECOVERY_AUDIT_2026-05-02.md`](NATIVE_READY_RECOVERY_AUDIT_2026-05-02.md).
> This file remains historical evidence of the earlier terminal audit. The
> recovery audit reopened phases 3-5 because cited stress tests now filter to
> zero tests and runner evidence is schema/sample-only after `bd-0w46` removed
> RuntimeValue from active Rust source.

Date: 2026-05-01
Bead: `bd-9xmu.6` / terminal audit search gate
Umbrella: `bd-9xmu`
Workset: `WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md`

## Outcome

The Native-Ready umbrella terminal gate is satisfied for this baseline:

- all five child worksets/phases are closed;
- RuntimeValue residuals are explicit compatibility/blocker surfaces, not hidden
  normal semantic carriers;
- fake HIR/MIR/CFG scaffold code is absent from active crates;
- current architecture/IR/bytecode/README docs describe implementation truth;
- correctness stress rows and oracle packet evidence exist;
- VM/JIT/wrapper runner evidence uses the shared result schema;
- future direct native compiler/linker work has a clean prerequisite checklist and
  must not claim current PE/ELF native output.

## Child workset closure audit

Closed child epics under `bd-9xmu`:

- `bd-9xmu.2`: docs truth and archive rebase.
- `bd-pn5i`: RuntimeValue and IR stub cleanout.
- `bd-9xmu.3`: value substrate, numeric, and UDT cleanup.
- `bd-9xmu.4`: correctness corpus and oracle stress.
- `bd-9xmu.5`: reference runners and performance scaffold.

Terminal bead `bd-9xmu.6` records this final audit before umbrella closure.

## Search gates

Commands and results:

```text
rg -n "\bRuntimeValue\b" crates docs --glob '!docs/archive/**' --glob '!docs/**/archive/**' | wc -l
# 3669

rg -l "\bRuntimeValue\b" crates | wc -l
# 58

rg -n "\bRuntimeValue\b" crates | wc -l
# 2706

rg -n "\bRuntimeValue\b" crates --glob '!**/tests/**' | wc -l
# 2252

rg -n "\bRuntimeValue\b" crates --glob '**/tests/**' | wc -l
# 454

rg -n "pub use .*RuntimeValue|pub type .*RuntimeValue|pub struct RuntimeValue|enum RuntimeValue|type RuntimeValue" crates/oxvba-runtime/src crates/oxvba-host/src crates/oxvba-jit/src crates/oxvba-com/src crates/oxvba-hal/src crates/oxvba-vm/src
# crates/oxvba-runtime/src/compat.rs:9:pub use crate::runtime_value::RuntimeValue;
# crates/oxvba-runtime/src/runtime_value.rs:225:pub enum RuntimeValue {

rg -n "CfgIr|VbaHir|VbaMir" crates | wc -l
# 0

rg -n "oxvba[_-]ir|lower_to_hir|VbaHir|VbaMir|CfgIr" crates Cargo.toml Cargo.lock | wc -l
# 0

rg -n "CfgIr|VbaHir|VbaMir" docs --glob '!docs/archive/**' --glob '!docs/**/archive/**' | wc -l
# 9
```

RuntimeValue matches remain by design in compatibility modules, legacy bridge
methods, tests, residual blocker notes, and historical evidence/log artifacts.
They are governed by
`RUNTIMEVALUE_BRIDGE_PUBLIC_API_BLOCKERS_2026-05-01.md` and were copied into
`CURRENT_BLOCKERS.md` as `RV-BRIDGE-001` through `RV-BRIDGE-004` before this
terminal gate closed.

Fake IR crate/API matches are zero. The remaining non-archived docs mentions are
current explanatory or residual-note surfaces (`IR_DESIGN.md`, phase-2 evidence,
and the umbrella/workset terminal gate text), not active implementation claims.

## Documentation truth audit

Checked current authoritative docs:

```text
rg -n "lower_to_hir|oxvba-ir|HIR/MIR|direct native|native AOT|native-pe|native-elf|VbaHir|VbaMir|CfgIr|RuntimeValue" docs/ARCHITECTURE.md docs/IR_DESIGN.md docs/BYTECODE_FORMAT.md docs/README.md
```

Result: matches describe current truth and non-claims:

- no active direct native AOT PE/ELF compiler is claimed;
- source analysis emits bytecode directly, not HIR/MIR/CFG;
- historical fake IR names are described as removed scaffold;
- RuntimeValue residuals are described as compatibility/blocker surfaces;
- direct native work is a future workset after this baseline.

## Runner schema audit

Runner schema and sample artifacts validated:

- `docs/spec/NATIVE_READY_RUNNER_AND_BENCHMARK_SCHEMA_V1.md` is
  `locked-baseline`.
- Canonical header:
  `runner_samples/native_ready_runner_schema_header_v1.csv`.
- VM/JIT rows:
  `runner_samples/vm_jit_runner_rows_2026-05-01.csv`.
- Wrapper rows:
  `runner_samples/wrapper_runner_rows_2026-05-01.csv`.
- Size/timing rows:
  `runner_samples/runner_size_timing_rows_2026-05-01.csv`.
- Benchmark corpus seed:
  `runner_samples/benchmark_corpus_2026-05-01.csv`.

Validation command:

```text
python - <<'PY'
from pathlib import Path
base = Path('docs/evidence/native_ready/runner_samples')
header = (base / 'native_ready_runner_schema_header_v1.csv').read_text().strip().split(',')
for path in sorted(base.glob('*.csv')):
    rows = path.read_text().strip().splitlines()
    assert rows, path
    assert rows[0].split(',') == header, path
    for line in rows[1:]:
        cells = line.split(',')
        assert len(cells) == len(header), (path, len(cells), len(header), line)
        data = dict(zip(header, cells))
        assert data['backend'] in {'vm','jit','wrapper-exe','wrapper-library','native-pe-x64','native-elf-x64'}
        assert data['artifact_kind'] in {'none','oxb','wrapper-exe','wrapper-library','native-exe','native-library'}
        assert data['artifact_size_bytes'].isdigit()
        if data['backend'] == 'jit':
            assert data['fallback_used'] in {'true','false'}
            assert data['fallback_reason']
PY
```

Result: passed.

## Stress/check audit

Passed:

```text
cargo fmt --all
cargo test -p oxvba-vm numeric_stress_rounding_overflow_truncation_edges
cargo test -p oxvba-vm coercion_error_stress_rows_cover_empty_null_cverr_and_assignment_timing
cargo test -p oxvba-host nested_udt
cargo check --workspace
```

## Future native compiler/linker prerequisite checklist

Before starting direct native compiler/linker delivery, a future workset must
honor these prerequisites:

1. Use retained `Variant` as the normal semantic carrier.
2. Treat `RuntimeValue` only as explicit compatibility/test projection until
   `RV-BRIDGE-001` through `RV-BRIDGE-004` are retired or intentionally retained
   with semver policy.
3. Do not resurrect `VbaHir`, `VbaMir`, `CfgIr`, `oxvba-ir`, or
   `lower_to_hir`; introduce a real native-facing IR only with a concrete
   contract and parity tests.
4. Reuse `NATIVE_READY_VALUE_SUBSTRATE_V1.md` for scalar/UDT value rules.
5. Reuse `NATIVE_READY_RUNNER_AND_BENCHMARK_SCHEMA_V1.md` for correctness and
   performance evidence.
6. Populate `native-pe-x64` / `native-elf-x64` rows only after real native
   artifacts execute.
7. Keep UDT native ABI/layout claims separate from the current bounded semantic
   subset until layout/packing/marshaling evidence exists.

## Terminal decision

The umbrella does not claim direct native compilation is implemented. It does
claim that the repository now has an evidence-backed baseline for subsequent
native compiler/linker planning without relying on fake IR or unqualified
RuntimeValue semantic carriers.
