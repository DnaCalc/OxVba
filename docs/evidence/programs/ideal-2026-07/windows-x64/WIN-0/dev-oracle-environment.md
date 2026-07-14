# WIN-0 development/oracle environment capture

Program period: **2026-07**. Environment: `win-x64-dev-oracle-2026-07`.

This is the immutable characterization of the current development and Excel/VBA oracle host. It is explicitly `release=false`, `certification_authority=false` and `noncertifying=true`. It cannot replace the clean pinned Windows x64/64-bit Excel certification VM.

## Canonical capture

- Capture: `docs/evidence/programs/ideal-2026-07/windows-x64/WIN-0/dev-oracle-environment.json`
- Schema: `oxvba-windows-x64-environment-capture-v1` version `1`
- Capture SHA-256: `sha256:6616a1302f787f77f1acf022315a92f428f425279ef46d5752666c8ff3e1edf1`
- Host configuration identity: `dev-host-fingerprint-v1@sha256:47f9cfdb1e43709bbdebd461a02021a029c23369a1361706e9d372d6fa081bfa`
- Host fingerprint input SHA-256: `sha256:47f9cfdb1e43709bbdebd461a02021a029c23369a1361706e9d372d6fa081bfa`
- Environment manifest: `docs/validation/IDEAL_ENVIRONMENT_MANIFEST_V1.csv`
- Fixture manifest: `docs/validation/IDEAL_WINDOWS_X64_FIXTURE_MANIFEST_V1.csv`
- Stable controlled-artifact root-contract SHA-256: `sha256:dc282cd1a367b8ed9d6e11163194392437aa3fbd8e149f203e4fd07ca6f985b5`

The host identity hashes observed OS, Office x64, locale/codepage, Rust toolchain and operating-policy facts. It is a configuration fingerprint, not a claim that this physical host is resettable or clean.

## Observed host

| fact | value |
|---|---|
| OS build | `10.0.26200.8655` (`25H2`, `Professional`) |
| Excel/Office | `Microsoft Excel 16.0.20131.20112`; `Current Channel`; 64-bit; PE `AMD64` |
| Office channel identity | `492350f6-3a01-4f97-b9c0-c7c6ddf67d60` |
| Office client culture | `en-us` |
| Office product release IDs | `O365BusinessRetail,VisioPro2019Retail,O365ProPlusRetail,VisioProRetail` |
| Current/UI/system locale | `en-ZA` / `en-US` / `en-US` |
| ANSI/OEM codepage | `1252` / `437` |
| Console input/output codepage | `65001` / `65001` |
| rustc | `rustc 1.94.1 (e408947bf 2026-03-25)<br>binary: rustc<br>commit-hash: e408947bfd200af42db322daf0fadfe7e26d3bd1<br>commit-date: 2026-03-25<br>host: x86_64-pc-windows-msvc<br>release: 1.94.1<br>LLVM version: 21.1.8` |
| Cargo | `cargo 1.94.1 (29ea6fb6a 2026-03-24)` |
| rustup active | `stable-x86_64-pc-windows-msvc (default)` |

## Controlled fixture roots

The hashes below are ordinal, length-prefixed derivations of state-independent roots in the canonical fixture manifest. Build, source and environment state/hash transitions do not invalidate this host evidence, and no pending artifact is claimed to exist.

- Rows: `57`; controlled-artifact root contract: `sha256:dc282cd1a367b8ed9d6e11163194392437aa3fbd8e149f203e4fd07ca6f985b5`.
- Rows using this environment: `12`; capture-root contract: `sha256:8a244673401d18b722285d69111a00a082bc3ae5362041b3c3aaeb45c384df94`.

## Reset, ownership and UIA policy

- Reset role: `noncertifying-host;record-owned-processes;manual-recovery`. Manual recovery is noncertifying; there is no clean-snapshot claim.
- Process ownership: `record-and-clean-owned-PIDs-only;never-kill-unowned`.
- Excel/VBE UIA: `Excel-VBE-UIA-modal-intercept;capture-dialog-token-line;dismiss-owned-dialogs-only`.
- The capture required no running Excel process, launched no Excel/VBE/COM/UIA automation, opened registry keys read-only and performed no Office or registry write.
- The three Rust version observations ran as recorded owned child processes with bounded asynchronous stream draining and a 10-second wait, each synchronously reaped and disposed. The capture tool requested no temp path.
- Mutation verdict: zero Excel/Office or registry mutation, zero residual owned process, and zero capture-owned temp path; the three version readers were the only transient processes.

## Six-axis control evidence

| axis | observation |
|---|---|
| result | Exact V1 JSON was reconstructed from the canonical manifest and matched all observed OS/Office/locale facts. Capture hash: `sha256:6616a1302f787f77f1acf022315a92f428f425279ef46d5752666c8ff3e1edf1`. |
| full Err | Not applicable: no VBA compile or execution occurred; no Err state was created or consumed. |
| side effects | The only permitted persistent write was initial publication of the capture JSON and report; an identical rerun is read-only and a differing rerun is rejected. No Office, Excel, registry or fixture mutation API was used. No capture-owned temp path was requested. |
| lifecycle/order | Assert no Excel process; read registry/PE/locale/toolchain/manifests; re-read registry; assert no Excel process; seal capture; publish evidence. |
| transport | Read-only Win32 registry, PE metadata, locale APIs and synchronous `rustc`/`cargo`/`rustup` version queries. No COM, VBE, UIA or native fixture execution. |
| balance | Excel PID set was empty before and after. Selected registry observation hash was `sha256:9268beb75ff51cde7cfb00cb75985c41112b5f32ace24d79d69b38ef18130503` before and after. Each of the three owned version-observation children used bounded asynchronous drains, was awaited through exit and disposed; timeout cleanup is limited to its recorded owned PID. No broader system-process or temp-directory snapshot is claimed. |

## Host fingerprint preimage

The exact deterministic preimage below recomputes the SHA-256 suffix of `dev-host-fingerprint-v1@sha256:47f9cfdb1e43709bbdebd461a02021a029c23369a1361706e9d372d6fa081bfa`.

<!-- oxvba-dev-host-fingerprint-preimage-v1-begin -->
```json
{
  "schema_id": "oxvba-windows-x64-dev-host-fingerprint-v1",
  "schema_version": 1,
  "environment_id": "win-x64-dev-oracle-2026-07",
  "profile": "windows-x64",
  "target_arch": "x64",
  "os": {
    "build": "10.0.26200.8655",
    "display_version": "25H2",
    "edition_id": "Professional"
  },
  "office": {
    "product": "Microsoft Excel",
    "version": "16.0",
    "build": "16.0.20131.20112",
    "channel": "Current Channel",
    "channel_identity": "492350f6-3a01-4f97-b9c0-c7c6ddf67d60",
    "bitness": "64",
    "excel_pe_machine": "AMD64",
    "client_culture": "en-us",
    "product_release_ids": "O365BusinessRetail,VisioPro2019Retail,O365ProPlusRetail,VisioProRetail"
  },
  "locale": {
    "current_culture": "en-ZA",
    "current_ui_culture": "en-US",
    "system_locale": "en-US",
    "ansi_codepage": 1252,
    "oem_codepage": 437,
    "console_input_codepage": 65001,
    "console_output_codepage": 65001
  },
  "toolchain": {
    "rustc_verbose": "rustc 1.94.1 (e408947bf 2026-03-25)\nbinary: rustc\ncommit-hash: e408947bfd200af42db322daf0fadfe7e26d3bd1\ncommit-date: 2026-03-25\nhost: x86_64-pc-windows-msvc\nrelease: 1.94.1\nLLVM version: 21.1.8",
    "cargo_version": "cargo 1.94.1 (29ea6fb6a 2026-03-24)",
    "rustup_active_toolchain": "stable-x86_64-pc-windows-msvc (default)"
  },
  "reset_policy": "noncertifying-host;record-owned-processes;manual-recovery",
  "owned_process_policy": "record-and-clean-owned-PIDs-only;never-kill-unowned",
  "uia_modal_policy": "Excel-VBE-UIA-modal-intercept;capture-dialog-token-line;dismiss-owned-dialogs-only"
}
```
<!-- oxvba-dev-host-fingerprint-preimage-v1-end -->

This evidence verifies only `WAC-TARGET-DEV-ENV` as characterized development/oracle infrastructure. Capability and release-certification credit remain `none`. The later WIN-0 reconciliation bead owns publication into the controlled environment root and fixture-matrix handoff.
