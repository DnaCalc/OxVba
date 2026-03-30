# Web/Wasm Hosted Realization Review

Date: 2026-03-30  
Status: active review output  
Owning workset: `WORKSET_2026-03-30_WEB_WASM_HOSTED_REALIZATION_REVIEW_AND_SHOWCASE_PLANNING.md`

## 1. Review Question

What is the right near-term hosted realization of OxVba for web/wasm environments, what can be honestly shown now, and what execution work should follow from that answer?

## 2. Current-State Truth

### 2.1 What is real today

The repo has real wasm substrate in the HAL and host-policy layers.

Evidence and active truth:
1. [HAL_WASM_RUNTIME_CLASSES_V1.md](/C:/Work/DnaCalc/OxVba/docs/spec/HAL_WASM_RUNTIME_CLASSES_V1.md)
   - defines `wasi-local` and `browser-sandbox`
   - makes the capability boundary explicit
2. [HAL_OPERATING_ENVELOPE_V1.md](/C:/Work/DnaCalc/OxVba/docs/spec/HAL_OPERATING_ENVELOPE_V1.md)
   - records wasm as a deterministic sandbox profile
   - keeps filesystem/process/COM/dynlink unsupported in the current baseline
3. [HAL_RUNTIME_PROFILE_MATRIX_V1.md](/C:/Work/DnaCalc/OxVba/docs/spec/HAL_RUNTIME_PROFILE_MATRIX_V1.md)
   - records the runtime-profile split `wasm-wasi-local` and `wasm-browser-sandbox`
4. [hal_wasm32 evidence](/C:/Work/DnaCalc/OxVba/docs/evidence/hal_wasm32/HAL_CONFORMANCE_1772458937.md)
   - captures passing conformance evidence for both wasm runtime classes across runtime, compile-time, and interactive-dev lanes
5. [PH-0009](/C:/Work/DnaCalc/OxVba/docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv)
   - records the host-sensitive policy surface as implemented-subset and locally evidenced

The repo also has a clear preferred host-shell direction for a future user-facing application:
1. [DNAVBCALC_HOST_SHELL_BASELINE_PREPARATION_2026-03-09.md](/C:/Work/DnaCalc/OxVba/docs/DNAVBCALC_HOST_SHELL_BASELINE_PREPARATION_2026-03-09.md)
   - desktop Tauri shell
   - Rust backend
   - web UI frontend
   - debug/immediate-style first interaction surface

### 2.2 What is not real today

The repo does not currently prove a browser-native OxVba wasm product lane.

The design exists in [HOSTING_PROJECT_TOOLING_PROPOSAL.md](/C:/Work/DnaCalc/OxVba/docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md), especially UC-E section 3.5, but that section is still proposal truth rather than runnable/evidenced product truth.

What is missing from active evidence:
1. no demonstrated `oxvba.wasm` build artifact in the repo
2. no proved JS/WebAssembly import/export ABI for OxVba host callbacks
3. no browser-hosted end-to-end execution artifact
4. no browser-hosted project-loading evidence
5. no browser-shell UX evidence

### 2.3 Review conclusion on current truth

The honest repo truth is:
1. wasm profile/runtime-class and policy substrate are real
2. desktop embedded-host direction is real at the planning level and is the preferred host-shell path
3. browser-native OxVba wasm hosting remains proposed, not yet demonstrated

## 3. Recommended Near-Term Realization

The right near-term realization is:
1. desktop-first embedded host shell,
2. with a Rust backend and web UI frontend,
3. while treating wasm/browser-native hosting as a later realization that should be designed against the same explicit host-bridge contract.

This means the near-term product shape is not:
1. pure browser-sandbox OxVba as the first public demo,
2. or a speculative `oxvba.wasm` showcase with no proved bridge/runtime packaging path.

Reasoning:
1. the desktop-first shell aligns with the explicit DnaVbCalc host-shell baseline
2. it uses host-controlled embedded execution that the repo already understands better than browser-native packaging
3. it still preserves the long-term path to browser/wasm hosting, because the host-bridge contract remains the architectural seam
4. it avoids overstating the current repo as having a proved browser-native OxVba realization

## 4. Honest Showcase Slice Today

The honest showcaseable slice today is narrower than a browser-native product demo.

It can honestly show:
1. deterministic OxVba execution under wasm profile/runtime-class policy selection
2. capability-denial behavior for unsupported wasm domains
3. runtime-class distinction between `wasi-local` and `browser-sandbox`
4. host-policy/bootstrap surface that selects those lanes
5. the planned desktop-shell direction as a reviewed recommendation, not as a completed demo

It cannot honestly show today:
1. browser-native loading of an OxVba wasm binary
2. JS-hosted callback bridge behavior
3. browser-side project execution
4. a finished Tauri/web shell application

So the current showcase boundary is:
1. repo-backed wasm policy/profile demonstration
2. plus review-backed host-shell recommendation
3. not a web-hosted product demonstration

## 5. Required Validation Lanes For The Recommended Realization

For the recommended desktop-first host-shell realization with future wasm continuity, the required validation lanes are:

### 5.1 Existing lanes to retain

1. HAL/runtime profile conformance for `wasi-local` and `browser-sandbox`
2. host-policy/bootstrap override validation
3. compiler, interpreter, and JIT execution validation under the selected profile/policy lanes where applicable

### 5.2 New lanes to add in the next execution workset

1. embedded host-bridge contract validation
   - property get/set
   - method invocation
   - diagnostic routing
   - host-event ingress
2. shell-command validation
   - open/load
   - run
   - reset
   - eval/immediate-like command flow
3. project-hosting validation inside the embedded shell lane
4. packaged host baseline validation
   - host boots
   - project loads
   - engine executes under host control
5. explicit non-claim guardrails for browser-native wasm until that lane has real artifact and bridge evidence

## 6. Immediate Gaps

The immediate gaps are:
1. no explicit canonical host-bridge spec row for embedded web/desktop shell realization
2. no runnable embedded host-shell baseline in this repo
3. no browser-native wasm artifact/build/export lane
4. no validation matrix rows yet dedicated to browser-hosted/web-shell realization claims

## 7. Recommended Next Workset

The next execution workset should be:
1. desktop-first host-shell and host-bridge foundation
2. with wasm/browser-native work kept as a future follow-on, not the first execution promise

That next workset is defined in [WORKSET_2026-03-30_WEB_WASM_DESKTOP_FIRST_HOST_SHELL_AND_BRIDGE_FOUNDATION.md](/C:/Work/DnaCalc/OxVba/docs/worksets/WORKSET_2026-03-30_WEB_WASM_DESKTOP_FIRST_HOST_SHELL_AND_BRIDGE_FOUNDATION.md).

## 8. Bottom Line

The right answer is not “ship OxVba in the browser now.”

The right answer is:
1. keep the wasm profile/runtime-class substrate,
2. adopt a desktop-first embedded host shell as the first real realization,
3. showcase only the deterministic wasm policy/profile slice today,
4. and use the next execution workset to prove the host bridge and shell baseline before claiming a broader web-hosted story.
