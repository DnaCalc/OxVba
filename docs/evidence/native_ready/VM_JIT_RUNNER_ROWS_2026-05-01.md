# VM/JIT Runner Row Normalization Evidence

> Recovery note 2026-05-02: this historical file proves the original sample CSV
> shape only. Active VM/JIT schema production has since been restored in
> `RUNNER_PRODUCER_RECOVERY_2026-05-02.md` via `bd-9xmu.5.7`.

Date: 2026-05-01
Bead: `bd-9xmu.5.3` / `runner-002`
Workset: `WORKSET_2026-04-30_REFERENCE_RUNNERS_AND_PERF_SCAFFOLD.md`

## Outcome

Added sample VM/JIT runner rows under the locked schema:

- `runner_samples/vm_jit_runner_rows_2026-05-01.csv`

The sample covers:

- VM correctness row with `fallback_used=false` / `fallback_reason=not-applicable`;
- JIT correctness row with `fallback_used=false` / `fallback_reason=cranelift-executed`;
- JIT fallback row with `fallback_used=true` /
  `fallback_reason=unsupported-bytecode-vm-fallback`.

## Contract

JIT fallback rows are valid reference evidence but must not be counted as JIT or
native execution evidence. VM rows use `fallback_reason=not-applicable` to keep
the field populated while preserving backend comparability.

## Verification

Passed:

```text
python - <<'PY'
from pathlib import Path
header = Path('docs/evidence/native_ready/runner_samples/native_ready_runner_schema_header_v1.csv').read_text().strip().split(',')
rows = Path('docs/evidence/native_ready/runner_samples/vm_jit_runner_rows_2026-05-01.csv').read_text().strip().splitlines()
assert rows[0].split(',') == header
for row in rows[1:]:
    cells = row.split(',')
    assert len(cells) == len(header)
    data = dict(zip(header, cells))
    if data['backend'] == 'jit':
        assert data['fallback_used'] in {'true', 'false'}
        assert data['fallback_reason']
PY
cargo check --workspace
```
