# HAL Profile Matrix Draft (Early Stage)

Status: `design-draft`  
Date: 2026-03-01

Legend:
- Support: `Y` supported, `N` unsupported.
- Maturity: `stub`, `experimental`, `provisional`, `stable`.
- Values are planning targets, not implementation claims.

| Capability | Windows | Linux | macOS | WASM | Null |
|---|---|---|---|---|---|
| `ui_interaction` (`MsgBox`, `InputBox`) | Y / provisional | Y / experimental | Y / stub | Y / experimental | N / stable |
| `event_pump` (`DoEvents`) | Y / provisional | Y / experimental | Y / stub | Y / experimental | N / stable |
| `filesystem_io` | Y / provisional | Y / provisional | Y / experimental | Y / experimental | N / stable |
| `process_env` (`Shell`, env) | Y / provisional | Y / experimental | Y / experimental | N / stable | N / stable |
| `com_activation_dispatch` (`CreateObject`, dispatch) | Y / provisional | N / stable | N / stable | N / stable | N / stable |
| `time_locale` | Y / provisional | Y / provisional | Y / experimental | Y / experimental | Y / stable |
| `dynamic_linking` (`Declare`) | Y / experimental | Y / stub | Y / stub | N / stable | N / stable |
| `diagnostics_telemetry` | Y / stable | Y / stable | Y / stable | Y / stable | Y / stable |

Notes:
- `Null` profile is intentionally strict: deterministic unsupported behavior is considered `stable` when behavior shape is fully specified and tested.
- Non-Windows `com_activation_dispatch` is marked unsupported at baseline to avoid overpromising partial interop before explicit adapter scope is defined.
- Linux and macOS are split to prevent hidden platform divergence under a single "POSIX" label.
