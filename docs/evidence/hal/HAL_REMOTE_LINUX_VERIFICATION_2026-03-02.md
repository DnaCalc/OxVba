# HAL Remote Linux Verification (2026-03-02)

Scope: confirm HAL implementation and conformance on Linux host using the existing isolated remote runner envelope.

## Remote Host Envelope

- Host: `ubuntu@94.72.99.81`
- Base directory: `/home/ubuntu/.dnacalc_remote`
- Repo checkout: `/home/ubuntu/.dnacalc_remote/work/OxVba`
- Verified commit: `157d59b`

## Commands

```bash
source /home/ubuntu/.dnacalc_remote/bin/env.sh
/home/ubuntu/.dnacalc_remote/bin/sync_repo.sh
cd /home/ubuntu/.dnacalc_remote/work/OxVba
cargo test -p oxvba-hal
cargo run -q -p oxvba-hal --bin hal-conformance -- --output-dir docs/evidence/hal_remote_linux
```

## Results

- `cargo test -p oxvba-hal`:
  - `38 passed; 0 failed`
  - includes Linux-host native-mode checks:
    - `native_mode_process_and_env_paths_are_callable`
    - `native_mode_filesystem_seek_can_extend_length`
    - `native_mode_time_tokens_are_non_negative`
- HAL conformance generator:
  - passed for all declared profiles/lane checks with expected governance notice on macOS maturity stubs.
  - remote artifacts captured locally as:
    - `docs/evidence/hal/HAL_CONFORMANCE_REMOTE_LINUX_1772434934.md`
    - `docs/evidence/hal/HAL_CONFORMANCE_REMOTE_LINUX_1772434934.jsonl`

## Notes

- macOS execution verification remains deferred.
- remote run stayed within the isolated `.dnacalc_remote` workspace.
