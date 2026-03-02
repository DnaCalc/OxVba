# HAL UI Platform Implementation V2

Status: `implemented-subset`
Scope: `v207..v211`
Date: 2026-03-02

## Delivered Behavior

1. Windows GUI native `MsgBox` lane:
- in host-backed non-deterministic mode, when profile is Windows and runtime class is `windows-gui`, and UI virtualization is `Disabled`, HAL calls native `MessageBoxW`.
- deterministic/policy-denied lanes remain unchanged.

2. Linux stdio interaction lane:
- in host-backed Linux `linux-stdio` runtime class with UI virtualization `Disabled`, HAL emits deterministic console diagnostic lines and returns stable token outputs without blocking.

3. Runtime-class-aware `DoEvents`:
- host-backed Windows GUI lane performs one non-blocking message-queue pump then yields scheduler.
- other host-backed lanes perform scheduler yield.
- deterministic lanes retain token-stable `0` behavior.

4. File/time host-backed behaviors preserved:
- existing host-backed file/time paths continue to operate under policy + profile gating with no regression.

## Stability Rules

- Unsupported capability -> `HAL-E-CAP-UNAVAILABLE`
- Policy denial -> `HAL-E-POLICY-DENIED`
- Host adapter faults -> `HAL-E-ADAPTER-FAULT`

The runtime-class additions did not weaken these error-shape invariants.
