# Benchmark Corpus Shared Schema Evidence

> Recovery note 2026-05-02: this historical file proves benchmark corpus seed
> rows under the schema. The referenced stress filters and VM/JIT producer have
> since been restored by `CORRECTNESS_CORPUS_RECOVERY_EXECUTABLE_STRESS_2026-05-02.md`
> and `RUNNER_PRODUCER_RECOVERY_2026-05-02.md`; wrapper library producer work
> remains in `bd-9xmu.5.9`.

Date: 2026-05-01
Bead: `bd-9xmu.5.6` / `runner-005`
Workset: `WORKSET_2026-04-30_REFERENCE_RUNNERS_AND_PERF_SCAFFOLD.md`

## Outcome

Published first benchmark corpus seed under the locked runner schema:

- `runner_samples/benchmark_corpus_2026-05-01.csv`

The seed reuses correctness stress rows as workloads:

- `NR-NUM-002` numeric edge stress for VM and JIT rows;
- `NR-COERCE-001` coercion/error stress for a VM row;
- `NR-UDT-001` UDT semantic subset for a JIT row that explicitly records VM
  fallback.

## Boundary

The CSV is schema/benchmark-corpus evidence. It demonstrates row shape,
workload naming, fallback classification, timing/size fields, and digest/claim
boundary population. It does not claim cross-machine speed and does not claim
future direct native artifact support.

## Verification

Passed:

```text
python - <<'PY'
from pathlib import Path
header = Path('docs/evidence/native_ready/runner_samples/native_ready_runner_schema_header_v1.csv').read_text().strip().split(',')
rows = Path('docs/evidence/native_ready/runner_samples/benchmark_corpus_2026-05-01.csv').read_text().strip().splitlines()
assert rows[0].split(',') == header
seen = set()
for row in rows[1:]:
    cells = row.split(',')
    assert len(cells) == len(header)
    data = dict(zip(header, cells))
    seen.add(data['workload_id'])
    assert data['mode'] == 'benchmark'
    assert data['backend'] in {'vm', 'jit'}
    assert data['artifact_size_bytes'].isdigit()
    assert data['fallback_used'] in {'true', 'false'}
    assert data['fallback_reason']
    assert data['result_digest']
    assert data['claim_boundary']
assert {'NR-NUM-002', 'NR-COERCE-001', 'NR-UDT-001'} <= seen
PY
cargo check --workspace
```
