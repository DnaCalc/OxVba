# Runner Size And Timing Field Evidence

Date: 2026-05-01
Bead: `bd-9xmu.5.5` / `runner-004`
Workset: `WORKSET_2026-04-30_REFERENCE_RUNNERS_AND_PERF_SCAFFOLD.md`

## Outcome

Added sample benchmark rows that populate the shared size/timing fields:

- `runner_samples/runner_size_timing_rows_2026-05-01.csv`

The sample demonstrates:

- `artifact_size_bytes` is numeric and uses bytes;
- `iterations` and `warmup_iterations` are explicit;
- `mean_ms`, `min_ms`, and `max_ms` are elapsed milliseconds;
- rows preserve `result_digest` and `claim_boundary` even in benchmark mode.

## Claim boundary

Timing rows are local trend evidence only. They must not be cited as
cross-machine speed claims unless a future evidence packet names workload, host
class, backend pair, iteration policy, threshold, and artifact set.

## Verification

Passed:

```text
python - <<'PY'
from pathlib import Path
header = Path('docs/evidence/native_ready/runner_samples/native_ready_runner_schema_header_v1.csv').read_text().strip().split(',')
rows = Path('docs/evidence/native_ready/runner_samples/runner_size_timing_rows_2026-05-01.csv').read_text().strip().splitlines()
assert rows[0].split(',') == header
for row in rows[1:]:
    cells = row.split(',')
    assert len(cells) == len(header)
    data = dict(zip(header, cells))
    assert data['mode'] == 'benchmark'
    assert data['artifact_size_bytes'].isdigit()
    assert int(data['iterations']) > 0
    assert int(data['warmup_iterations']) >= 0
    for field in ['mean_ms', 'min_ms', 'max_ms']:
        assert float(data[field]) >= 0.0
    assert data['claim_boundary']
PY
cargo check --workspace
```
