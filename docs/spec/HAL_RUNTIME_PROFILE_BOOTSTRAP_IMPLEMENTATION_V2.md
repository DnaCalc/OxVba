# HAL Runtime Profile Bootstrap Implementation V2

Status: `implemented`
Scope: `v198..v201`
Date: 2026-03-02

## Delivered

1. Runtime-profile abstraction in host runner:
- `RuntimeProfileId` includes `windows-gui`, `windows-headless`, `linux-stdio`, `wasm-wasi-local`, `wasm-browser-sandbox`, `null-floor` (+ `macos-headless` for internal continuity).

2. Deterministic bootstrap resolver:
- implemented in `crates/oxvba-host/src/runner.rs`.
- precedence: `CLI > ENV > config file > defaults`.
- defaults: `windows-headless + deterministic-runtime`.

3. Policy/runtime-class separation:
- runtime profile selects HAL profile + runtime class.
- policy preset and explicit overrides are applied separately.
- resulting policy is deterministic and fingerprinted.

4. CLI integration:
- `oxvba-cli run` supports host-runner options (`--profile`, `--policy`, `--config`, policy override flags).
- optional `--dump-bootstrap` outputs deterministic startup fingerprint.

## Config + Env Surface

Config file (`[host]`) and environment keys follow `HOST_RUNNER_POLICY_BOOTSTRAP_V1.md` intent:
- `profile`, `policy_preset`, `runtime_class`
- policy override keys:
  - `allow_interaction`
  - `allow_process_spawn`
  - `allow_filesystem_mutation`
  - `allow_dynamic_link`
  - `allow_com_activation`
  - `deterministic_mode`
  - `ui_virtualization`
  - `unsupported_feature_mode`
  - `wasm_runtime_class`

## Test Evidence

- `runner::tests::bootstrap_precedence_cli_over_env_over_config`
- `runner::tests::bootstrap_fingerprint_is_deterministic`
- `runner::tests::parse_config_text_reads_host_section_only`
- `oxvba-cli` parse tests for bootstrap flags.
