# HAL Interface Draft (Early Stage)

Status: `design-draft`  
Date: 2026-03-01

## 1. Intent

This draft proposes the initial contract model for OxVba Host Abstraction Layer interfaces.

Design constraints:
- Rust-first contract for now.
- Explicit capability and maturity disclosure.
- Deterministic behavior when capability is unavailable.
- Policy-based control for interactive/unsafe host operations.

## 2. Top-Level Contract

Proposed root contract shape:

```rust
pub trait HostServices {
    fn profile(&self) -> HalProfileId;
    fn descriptor(&self) -> HalDescriptor;
    fn policy(&self) -> &HostPolicy;

    fn ui(&self) -> &dyn UiInteractionHal;
    fn events(&self) -> &dyn EventPumpHal;
    fn fs(&self) -> &dyn FileSystemHal;
    fn process(&self) -> &dyn ProcessEnvHal;
    fn com(&self) -> &dyn ComHal;
    fn time_locale(&self) -> &dyn TimeLocaleHal;
    fn dynlink(&self) -> &dyn DynamicLinkHal;
}
```

Interface details are expected to evolve; this shape exists to lock design vocabulary and separation lines.

## 3. Profile and Descriptor Model

```rust
pub enum HalProfileId {
    Windows,
    Linux,
    MacOs,
    Wasm,
    Null,
}

pub struct HalDescriptor {
    pub profile: HalProfileId,
    pub version: String,
    pub capabilities: Vec<CapabilityDescriptor>,
}

pub struct CapabilityDescriptor {
    pub id: CapabilityId,
    pub supported: bool,
    pub maturity: CapabilityMaturity,
    pub notes: Option<String>,
}

pub enum CapabilityMaturity {
    Stub,
    Experimental,
    Provisional,
    Stable,
}
```

Rules:
- `supported = false` implies deterministic unsupported behavior for every API in that capability.
- `supported = true` with `Stub` or `Experimental` must emit explicit diagnostics when exercised.
- Maturity is per capability, not only per profile.

## 4. Host Policy Model

Policy controls permission and virtualization mode:

```rust
pub struct HostPolicy {
    pub allow_interaction: bool,
    pub allow_process_spawn: bool,
    pub allow_filesystem_mutation: bool,
    pub allow_dynamic_link: bool,
    pub allow_com_activation: bool,
    pub deterministic_mode: bool,
    pub ui_virtualization: UiVirtualizationMode,
}

pub enum UiVirtualizationMode {
    Disabled,
    ScriptedResponses,
    FailOnPrompt,
}
```

Policy must be available at runtime and surfaced in test/evidence metadata.

## 5. Domain Contracts (Draft Scope)

### 5.1 UI Interaction HAL

Responsibilities:
- Implement `MsgBox`/`InputBox` pathways.
- Support deterministic virtualization mode.
- Preserve button/result and cancel distinctions.

Deterministic fallback:
- If interaction disallowed: return policy-defined deterministic error path.

### 5.2 Event Pump HAL

Responsibilities:
- Provide `DoEvents` integration.
- Expose whether queue pump actually occurred.
- Support deterministic synthetic event queue in tests.

### 5.3 FileSystem HAL

Responsibilities:
- File open/read/write/seek/close primitives used by runtime library surface.
- Path normalization behavior explicitly documented per profile.

### 5.4 ProcessEnv HAL

Responsibilities:
- Environment variable read/write policy.
- Process launching (`Shell`) capability.
- Current-directory and process metadata integration.

### 5.5 COM HAL

Responsibilities:
- `CreateObject`, dispatch invocation pathways, and object-handle lifecycle integration.
- Windows profile: real COM bridge.
- Non-Windows profiles: supported only where adapter semantics are explicitly defined and tested.

### 5.6 TimeLocale HAL

Responsibilities:
- Time source and locale-sensitive formatting/parsing hooks that may affect runtime behavior.

### 5.7 DynamicLink HAL

Responsibilities:
- Declare/foreign-symbol loading and invocation policy gate.
- Deterministic blocked-path behavior for restricted profiles/modes.

## 6. Unsupported Behavior Contract

Unsupported capability behavior must be:
- deterministic,
- diagnosable (stable error code/message family),
- side-effect free beyond diagnostics.

No silent no-op pathways for unsupported host features.

## 7. Error Taxonomy Hook

HAL errors should map into stable diagnostic families:
- `HostCapabilityUnavailable`
- `HostPolicyDenied`
- `HostAdapterFault`
- `HostAdapterUnsupportedProfile`

Exact code mapping is deferred until taxonomy alignment pass.

## 8. Maturity Promotion Criteria (Draft)

Stub -> Experimental:
- compiles, basic smoke probes pass, deterministic unsupported cases defined.

Experimental -> Provisional:
- domain conformance tests pass on target profile,
- no known crash/unsoundness blockers,
- error taxonomy and policy behavior stable.

Provisional -> Stable:
- sustained conformance pass history,
- differential/reference checks where applicable,
- performance and reliability within declared envelope.

## 9. Deferred/Out-of-Scope for This Draft

- External C ABI stabilization.
- Out-of-process host adapters.
- Security sandbox model beyond policy flags.
- Full formal semantics of each host domain.
