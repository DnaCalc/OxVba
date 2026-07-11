# CORE-1 Pinned Linux x64 CI Environment Contract

Date: 2026-07-11
Bead: `bd-59co.2.2.8`
Base: `c0ff1de7a61dce2df6c83eff730582f2fd6f969b`
Implementation commit: `d9a471b5703c79e06d1702ff292165a5b41fa9eb`
Hardening commit: `22fc9de46828dc8595313c89c3207dc5601a49c7`
Post-handoff fixture repair: `730ae06b4bc7d5733d7aa94ce187752f8f7dc48d`
Clause: `CONF-QUALITY-001`
Matrix route: `CORE-READINESS/CORE-BASELINE-CROSS-PLATFORM-GATES`

## Result and claim boundary

The repository now has an executable, versioned Linux x64 CI contract at
`ci/linux-x64/contract-v1.json`. The three Linux workflow jobs use a full-SHA
checkout action and run repository commands inside one architecture-specific
OCI manifest. Rust, Cargo, PowerShell, Kani and Wasmtime identities are exact;
locale and time zone are fixed; cross-run action caches are absent; controlled
environment-source hashes are LF-canonical and reproducible on Windows and
Linux checkouts.

This bead does **not** claim that the Linux baseline ran. The contract's
`execution_evidence_state` remains `planned-blocking`, owned by
`bd-59co.2.2.11`. No Linux container or GitHub Actions runner was available in
this Windows development session, so `-Runtime`, workspace tests and the
canonical Linux profile runner were not executed here.

## Immutable identities

| component | sealed identity |
|---|---|
| GitHub runner release assertion | `ubuntu24/20260705.232`, source commit `7a421938a88d5f98ff2cf22875b5237aa80f54c1`, SBOM SHA-256 `3a0031ca049f21bd6a8af509c4b21fa967e75bd66617fb0786cc9a91042dafdb` |
| scheduling label | `ubuntu-24.04`; scheduling only, never execution authority |
| Linux x64 execution image | `docker.io/library/rust@sha256:4ec71e955e6c08aeb238885083222ddff79d82eb87654a96c76e38e94da1a53b` |
| Docker tag/index provenance | `1.94.1-bookworm`, index `sha256:6ae102bdbf528294bc79ad6e1fae682f6f7c2a6e6621506ba959f9685b308a55` |
| Rust / Cargo | `rustc 1.94.1`, commit `e408947bfd200af42db322daf0fadfe7e26d3bd1`; `cargo 1.94.1`, commit `29ea6fb6a5db279426f4cc4e17aa385f05a0cfbc` |
| Rust distribution manifest | SHA-256 `cc2f04dfc883549d683c8cc2a9393f523a3dfbd931f5d5eaef00303cca64a60d` |
| PowerShell bootstrap | `7.5.7` Linux x64 archive SHA-256 `207a3c0b2f630e8e1226cc9beb651e2e16789f07729197f45fd3ad0902d1c593` |
| checkout action | `actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5` |
| Kani | `0.67.0`, release commit `4feaaad1d6a2378a6ff6caa3b4fc5d6999c7bb5d` |
| Wasmtime | `42.0.1`; Linux archive/binary SHA-256 `dd5253...44d59` / `21f8e8...2108b`; Windows archive/binary SHA-256 `daa527...c072` / `b86766...a4b1` |
| deterministic process context | `LANG=C.UTF-8`, `LC_ALL=C.UTF-8`, `TZ=UTC`, Linux `x86_64` |

Official provenance used for the lock:

- Rust 1.94.1 release and distribution manifest:
  `https://blog.rust-lang.org/2026/03/26/1.94.1-release/` and
  `https://static.rust-lang.org/dist/channel-rust-1.94.1.toml`.
- Docker Official Rust image metadata and registry manifest:
  `https://hub.docker.com/_/rust/` and
  `https://registry-1.docker.io/v2/library/rust/manifests/1.94.1-bookworm`.
- GitHub runner release and fresh-VM policy:
  `https://github.com/actions/runner-images/releases/tag/ubuntu24/20260705.232`
  and `https://docs.github.com/en/actions/reference/runners/github-hosted-runners`.
- GitHub full-SHA action guidance:
  `https://docs.github.com/en/actions/choosing-solutions-and-products/secure-your-repository/security-guides/security-hardening-for-github-actions#using-third-party-actions`.
- PowerShell, Kani and Wasmtime releases:
  `https://github.com/PowerShell/PowerShell/releases/tag/v7.5.7`,
  `https://github.com/model-checking/kani/releases/tag/kani-0.67.0`, and
  `https://github.com/bytecodealliance/wasmtime/releases/tag/v42.0.1`.

`validate-linux-ci-environment.ps1 -VerifyExternalProvenance` re-read these
official registries/APIs on 2026-07-11 and matched every recorded release,
commit, index, manifest and release-asset digest. Routine governance remains
offline and validates the sealed bytes rather than depending on network state.

## Workflow and reset policy

The `linux-ready`, `wasm-hal-ready` and optional `formal-kani` jobs all have:

- the same per-architecture OCI manifest, not a mutable container tag;
- the same full 40-character checkout commit and clean checkout with persisted
  credentials disabled;
- the checksum-verified PowerShell bootstrap before any PowerShell gate;
- a runtime preflight before repository work;
- no `actions/cache`, `rust-cache`, mutable Rust action or cross-run state path;
- explicit `C.UTF-8` and UTC process settings.

GitHub cannot select a hosted VM by disk-image digest. The workflow therefore
uses `ubuntu-24.04` only as a scheduler, pins the current official runner-image
release commit and SBOM digest in the contract, and requires runtime
`ImageOS=ubuntu24` plus `ImageVersion=20260705.232`. If GitHub has rolled that
image, the preflight fails rather than silently accepting a different host.
The digest-pinned job container is the execution environment authority.

The reset contract is:

```text
github-hosted-new-vm-per-job;fresh-digest-pinned-job-container;clean-checkout;no-actions-cache;owned-state-under-RUNNER_TEMP;delete-owned-processes-and-state-only
```

At runtime the validator rejects a pre-existing owned sentinel below
`RUNNER_TEMP`, creates and deletes it, verifies the tracked checkout is clean,
and checks the exact OS, architecture, runner release, toolchain and locale.
The PowerShell bootstrap similarly refuses a pre-existing owned tool root.

## Controlled source and fixture hash

Hashes are SHA-256 over strict UTF-8 after CRLF normalization to LF. A lone
carriage return is rejected rather than normalized. That makes the contract
independent of ordinary checkout transport while still rejecting malformed EOL
states and any semantic or byte change to its controlled inputs.

| path | canonical SHA-256 |
|---|---|
| `.github/workflows/ci.yml` | `bdc6ff1f7a0859dafdf4c54f70e151bb118d2e978e83bbec166365d4cd01de52` |
| `scripts/install-pinned-pwsh.sh` | `f822fe27bc75cb435773ad1b9ea1dbd3d28a5a873c0d3c9a612b13d0ff3fe6cf` |
| `scripts/run-hal-conformance-wasm32.ps1` | `0d64e4b56cd7ed9ccba60e45e3233df717530dcd9efb14b32f30f3b01088f570` |
| `scripts/setup-kani.ps1` | `9d84171fb604de2bece6c7efaa990feb7d46632c32e054667e0756145bffe8e8` |
| `scripts/test-linux-ci-environment.ps1` | `df6e9dfe68e4235df602288fe758e427c0f5962cc7a600df235e806d35606c26` |
| `scripts/validate-linux-ci-environment.ps1` | `366a8de58ff65e997e9e8327f8a4616f5a8115d509c92ef5804958d95991cc03` |

The shared `scripts/check-governance.ps1` aggregator is deliberately not hashed:
other accepted gates, including the EOL contract, legitimately extend it. The
Linux validator instead requires exactly one invocation of both Linux contract
checks, so unrelated governance integration does not force an environment
identity change or permit the wiring to disappear.

The canonical SHA-256 of `ci/linux-x64/contract-v1.json` is:

```text
47621bd8c70984908bc3c0b448d33da560410fb54566cab9400f082858df1e2b
```

This is the exact `fixture_hash` for the controller-owned environment row.

## Fail-closed verification

`test-linux-ci-environment.ps1` builds isolated process-unique repositories
below the system temp root. It finds the unique `role=linux-ci` row and
structurally serializes two explicit positive fixtures independent of the
repository's current canonical row: pre-handoff pending identity owned by `.8`,
and sealed contract identity owned by `.11`. It asserts their distinct derived
states, environment IDs, owners and image identities before invoking the
validator. Both pass. Twenty independent mutations fail for the intended reason:

- `ubuntu-latest`, container tag, wrong container digest and runner release
  drift;
- checkout tag, `stable` toolchain action, unpinned Kani command and retained
  cache action;
- locale drift and missing runtime preflight;
- mutable Rust/container contract values, a null boolean authority field and a
  reset policy retaining state;
- forged controlled-source hash;
- bare carriage return, duplicate and mis-cased JSON properties;
- mutable ledger identity and ledger-owner drift.

The ledger alias and owner-drift mutations also address the unique row by field;
they do not search for a literal CSV fragment. The same 20 cases pass when every
negative fixture starts from either an explicit pending row or an explicit
sealed row. The contract parser rejects duplicate properties before `ConvertFrom-Json`,
requires case-exact closed schemas, and verifies both exact values and the
controlled source bytes. Mutation cleanup refuses any path outside the owned
random system-temp directory.

## Checks executed

```text
./scripts/validate-linux-ci-environment.ps1
PASS: contract and controlled source hashes; pending controller handoff.

./scripts/test-linux-ci-environment.ps1 -FixtureLedgerState Pending
PASS: distinct pending/sealed positives and all 20 fail-closed mutations; every negative fixture starts from the pre-handoff canonical form.

./scripts/test-linux-ci-environment.ps1 -FixtureLedgerState Sealed
PASS: distinct pending/sealed positives and all 20 fail-closed mutations; every negative fixture starts from the integrated sealed canonical form.

./scripts/validate-linux-ci-environment.ps1 -VerifyExternalProvenance
PASS: official runner, OCI, Rust, action, PowerShell, Kani and Wasmtime identities.

./scripts/validate-environment-manifest.ps1
PASS: existing three-role canonical manifest remains structurally valid.

PowerShell parser + bash -n + YAML safe parse
PASS: validator/test/setup/HAL scripts, installer and six-job workflow parse.

git diff --check
./scripts/check-staged-commit-scope.ps1
PASS: clean code/contract commit scope.

./scripts/check-governance.ps1
PARTIAL: the new Linux validator and all 20 mutations passed, as did the
pre-existing checks through project-integration-catalog. The base commit then
failed on the inherited stale generated
`docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md`; this bead did not edit or
regenerate that controller-owned artifact.
```

## Observable axes

| axis | evidence |
|---|---|
| result | Static, sealed-handoff and live-official-provenance validations pass; no Linux execution result is claimed. |
| full Err | Not applicable to environment selection. No VBA program ran. |
| side effects | Live checks are read-only HTTP requests. Mutation state is process-unique and removed from the owned system-temp root. |
| lifecycle/order | Checkout -> pinned PowerShell -> runtime identity/reset preflight -> job gate. A mismatched runner or retained sentinel stops before repository gates. |
| transport | Full action commit, runner release/SBOM, OCI index/manifest, release-asset and controlled-source SHA-256 identities are recorded and checked. |
| balance | Linux jobs have no cross-run action cache; owned sentinel state is created and deleted; the job VM/container is fresh and decommissioned by the provider. |

## Controller-owned truth handoff

This worker did not modify `.beads`, the canonical environment/matrix/trace
ledgers, `AUTORUN_STATE.md`, workset truth or generated summaries. The
controller must replace the current `linux-ci` row in
`IDEAL_ENVIRONMENT_MANIFEST_V1.csv` with this exact row (header omitted):

```csv
"linux-x64-ci-rust-1.94.1-bookworm-amd64-v1","linux-ci","core","x64","debian-12-bookworm-amd64@sha256:4ec71e955e6c08aeb238885083222ddff79d82eb87654a96c76e38e94da1a53b","n/a","n/a","n/a","n/a","n/a","C.UTF-8","docker.io/library/rust@sha256:4ec71e955e6c08aeb238885083222ddff79d82eb87654a96c76e38e94da1a53b","github-hosted-new-vm-per-job;fresh-digest-pinned-job-container;clean-checkout;no-actions-cache;owned-state-under-RUNNER_TEMP;delete-owned-processes-and-state-only","ci/linux-x64/contract-v1.json","sha256:47621bd8c70984908bc3c0b448d33da560410fb54566cab9400f082858df1e2b","github-hosted-new-VM-per-job;fresh-job-container;no-actions-cache;record-and-clean-owned-processes-and-RUNNER_TEMP-state-only","n/a-no-Excel-UIA","planned-blocking","bd-59co.2.2.11","Immutable Linux x64 execution contract is sealed; the host label is scheduling only; the canonical baseline transcript remains pending under bd-59co.2.2.11"
```

The row intentionally remains `planned-blocking`; `bd-59co.2.2.11` is the open
execution owner after this bead closes. The controller must also:

1. keep `CORE-READINESS/CORE-BASELINE-CROSS-PLATFORM-GATES` planned and add this
   artifact/contract as environment evidence without claiming a Linux run;
2. preserve `.11` as the residual owner and ensure `.8` trace/evidence references
   resolve to this artifact and both validators;
3. update `AUTORUN_STATE`, close/sync `.8`, refresh derived summaries and run
   truth reconciliation only after integration;
4. make `CORE-1`'s future versioned gate runner (`.9`) update the controlled
   workflow hash whenever it replaces the current `linux-ready` command.

## Residual and blocking boundary

- `bd-59co.2.2.11` must execute the complete Linux x64 baseline in this exact
  runtime identity and attach the GitHub transcript. Local or WSL output is not
  a substitute.
- GitHub exposes a hosted-runner release identity and SBOM, not a selectable VM
  disk digest. The runtime assertion prevents silent roll-forward, but the
  exact hosted release may become unavailable. If it is unavailable before
  `.11`, that bead remains blocked until the lock is deliberately refreshed or
  an ephemeral self-hosted image with a selectable digest is provisioned.
- The job-level container cannot pin the provider's host kernel or CPU
  microcode. Those are scheduler substrate, not claimed execution-image bytes;
  the baseline transcript must record them before release evidence is accepted.
- No compatibility, compiler, VM3, JIT, differential or release row advances to
  verified on this contract-only result.
