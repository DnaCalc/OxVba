# Host Runner Policy Bootstrap V1

Status: `design-draft`  
Step: `v195`  
Date: 2026-03-02

## Objective

Define how runtime profile/mode/policy is configured at process startup in a reproducible, auditable way.

This addresses `HAL-U-009`.

## Configuration Sources

Precedence (highest first):
1. CLI flags
2. Environment variables
3. Config file
4. Project-file defaults (`DefaultRuntimeProfile` / `DefaultPolicyPreset`) when running a `.basproj`/`.vbp` project
5. Built-in platform defaults

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
oxvba run hello.bas
oxvba run hello.bas --policy strict-ci --allow-dynamic-link false
oxvba run-project . --config host-runner.toml
oxvba-run bundle.oxb --profile windows-stdio --policy interactive-dev
```

## Environment Shape (proposed)

```text
OXVBA_PROFILE=windows-stdio
OXVBA_POLICY_PRESET=interactive-dev
OXVBA_ALLOW_INTERACTION=false
OXVBA_UI_VIRTUALIZATION=scripted-responses
```

## Config File Shape (proposed, TOML)

```toml
[host]
profile = "windows-stdio"
policy_preset = "interactive-dev"
allow_interaction = false
ui_virtualization = "scripted-responses"
```

## Built-in Defaults

When no higher-precedence source provides a profile or policy:

- Windows defaults to `windows-stdio + interactive-dev`
- Linux defaults to `linux-stdio + interactive-dev`
- macOS defaults to `macos-headless + interactive-dev`
- WASM/null lanes continue to use their deterministic platform-selected runtime class with `interactive-dev` as the default policy preset unless explicitly overridden by the host

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
3. project defaults apply only below CLI/env/config and only for project-aware execution.
4. compile-time gate behavior follows selected unsupported-feature mode.

## Open Items

- final runner binary name and command UX.
- where configuration artifacts are persisted in CI and local runs.
