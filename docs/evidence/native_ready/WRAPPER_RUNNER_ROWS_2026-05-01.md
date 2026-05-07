# Wrapper Runner Smoke Row Evidence

> Recovery note: this historical file remains wrapper sample row shape only.
> Active VM/JIT and wrapper EXE production was restored in
> `RUNNER_PRODUCER_RECOVERY_2026-05-02.md`; real wrapper library row production
> was delivered in `WRAPPER_LIBRARY_RUNNER_PRODUCER_2026-05-07.md`.

Date: 2026-05-01
Bead: `bd-9xmu.5.4` / `runner-003`
Workset: `WORKSET_2026-04-30_REFERENCE_RUNNERS_AND_PERF_SCAFFOLD.md`

## Outcome

Added wrapper smoke sample rows under the locked schema:

- `runner_samples/wrapper_runner_rows_2026-05-01.csv`

The sample covers:

- `backend=wrapper-exe` with `artifact_kind=wrapper-exe`, executable artifact
  identity, stdout digest shape, and exit status.
- `backend=wrapper-library` with `artifact_kind=wrapper-library`, library
  artifact identity, exported-return digest shape, and exit status.

## Claim boundary

These rows validate evidence shape only. They do not claim direct native PE/ELF
execution and do not claim speed. Wrapper rows remain distinct from reserved
future `native-pe-x64` / `native-elf-x64` rows.

## Verification

Passed:

```text
python - <<'PY'
from pathlib import Path
header = Path('docs/evidence/native_ready/runner_samples/native_ready_runner_schema_header_v1.csv').read_text().strip().split(',')
rows = Path('docs/evidence/native_ready/runner_samples/wrapper_runner_rows_2026-05-01.csv').read_text().strip().splitlines()
assert rows[0].split(',') == header
for row in rows[1:]:
    cells = row.split(',')
    assert len(cells) == len(header)
    data = dict(zip(header, cells))
    assert data['backend'] in {'wrapper-exe', 'wrapper-library'}
    assert data['artifact_kind'] == data['backend']
    assert data['artifact_path']
    assert data['artifact_size_bytes'].isdigit()
PY
cargo check --workspace
```
