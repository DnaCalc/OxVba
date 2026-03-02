# Host Runner Policy Bootstrap V1

Status: `design-draft`  
Step: `v195`  
Date: 2026-03-02

## Objective

Define how runtime profile/mode/policy is configured at process startup in a reproducible, auditable way.

This addresses `HAL-U-009`.

## Configuration Sources

Proposed precedence (highest first):
1. CLI flags
2. Environment variables
3. Config file
4. Built-in defaults

## Required Keys

- `profile` (runtime profile)
- `policy_preset`
- policy overrides:
  - `allow_interaction`
  - `allow_process_spawn`
  - `allow_filesystem_mutation`
  - `allow_dynamic_link`
  - `allow_com_activation`
  - `deterministic_mode`
  - `ui_virtualization`
  - `unsupported_feature_mode`
  - `wasm_runtime_class`

## CLI Shape (proposed)

```text
oxvba-run --profile windows-headless --policy deterministic-runtime
oxvba-run --profile linux-stdio --policy strict-ci --allow-interaction false
oxvba-run --config host-runner.toml --source program.bas
```

## Environment Shape (proposed)

```text
OXVBA_PROFILE=windows-headless
OXVBA_POLICY_PRESET=deterministic-runtime
OXVBA_ALLOW_INTERACTION=false
OXVBA_UI_VIRTUALIZATION=scripted-responses
```

## Config File Shape (proposed, TOML)

```toml
[host]
profile = "windows-headless"
policy_preset = "deterministic-runtime"
allow_interaction = false
ui_virtualization = "scripted-responses"
```

## Audit Fingerprint

Runner must emit a startup fingerprint in logs/artifacts:
- selected profile,
- selected policy preset,
- explicit overrides applied,
- deterministic mode flag,
- runtime class.

## Failure Policy

Invalid configuration:
- fail fast before compile/execute phases,
- emit deterministic diagnostic with invalid key/value details.

## Conformance Expectations

1. identical config inputs produce identical fingerprint outputs.
2. precedence rules are deterministic and tested.
3. compile-time gate behavior follows selected unsupported-feature mode.

## Open Items

- final runner binary name and command UX.
- where configuration artifacts are persisted in CI and local runs.
