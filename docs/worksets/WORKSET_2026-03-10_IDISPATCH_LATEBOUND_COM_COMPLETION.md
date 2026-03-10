# Workset: Complete `IDispatch` / Late-Bound COM Client Support

Date: 2026-03-10  
Status: planned  
Primary ladder mapping: `v506..v526`, `v536..v539`  
Secondary ladder mapping: `v553..v556`, `v559..v566`  
Program anchor: `docs/worksets/PROFILE_LADDER_2026-03-08_MACH1000_V467_V620_VBA71_WINDOWS_OFFICE_COMPLIANCE.md`

## 1. Objective

Complete the Windows late-bound COM client surface so OxVba supports the real `IDispatch` calling model that VBA in Office relies on, rather than the current narrowed deterministic subset.

This workset is specifically about client-side late binding:
1. `CreateObject` / activation,
2. `GetIDsOfNames` / member-name resolution,
3. `IDispatch::Invoke` argument/result/error semantics,
4. `VARIANT` and `DISPPARAMS` marshalling,
5. Office-relevant late-bound behavior across supported invoke shapes.

This workset is not the general COM extraction plan and not the COM server/export plan. Those remain related but separate.

## 2. Current state

Implemented now:
1. request/vector-based late-bound invoke transport exists,
2. multi-argument method/property `get` / `put` / `putref` exists in the current integer-token lane,
3. controlled and registered fixtures exist,
4. native callback/event lanes are substantially healthier,
5. deterministic member-name resolution and cache behavior exist for the supported subset.

Still incomplete for true VBA/Office parity:
1. exact named-argument packing and `rgdispidNamedArgs` semantics,
2. optional-argument omission semantics across broad `Invoke` shapes,
3. broad `VARIANT` coercion and byref/value legality,
4. object/interface-pointer argument and result handling,
5. SAFEARRAY argument/result parity beyond the current narrow transport shape,
6. richer `VarResult` / `ExcepInfo` / `ArgErr` translation behavior,
7. broad Office automation compatibility outside the current fixture set.

Conclusion:
1. OxVba now has a meaningful late-bound COM subset.
2. OxVba does not yet have the full `IDispatch` / late-bound COM scope that VBA in Office supports.

## 3. Target outcome

For in-scope Windows client COM lanes, OxVba should support:
1. member resolution by name with deterministic cache behavior,
2. method calls with positional and supported named arguments,
3. property `get`, `put`, and `putref`,
4. omission/default handling for supported optional parameters,
5. scalar, object, and array argument/result marshalling for the supported Automation matrix,
6. deterministic but faithful translation of native invoke failure channels into OxVba diagnostics and `Err` state.

## 4. Scope boundaries

### In scope

1. Windows late-bound COM client behavior.
2. `IDispatch::Invoke` packing/unpacking semantics.
3. `DISPPARAMS` construction and named-arg handling.
4. `VARIANT`/`SAFEARRAY` marshalling needed for practical Office/VBA automation parity.
5. controlled and external registered automation test lanes.
6. documentation and evidence that explicitly state what is now supported.

### Out of scope

1. COM server/export completion.
2. non-Windows COM support.
3. DCOM/remoting.
4. full early-bound/type-library parity beyond what late-bound completion needs.
5. every obscure Office-host quirk before the main client surface is closed.

## 5. Problem decomposition

### IDC-01 Invoke shape parity

Need:
1. positional argument ordering parity,
2. named-arg packing via `rgdispidNamedArgs`,
3. property-put/property-putref rules that match Office/VBA expectations,
4. explicit omission/default handling.

Current gap:
1. current invoke path is widened, but broad named/optional parity is still deferred in spec.

### IDC-02 `VARIANT` marshalling parity

Need:
1. scalar numeric/bool/string parity,
2. object-valued arguments/results,
3. byref legality and deterministic rejection where unsupported,
4. array/SAFEARRAY parity for practical Office lanes.

Current gap:
1. the implemented lane is still centered around integer-token transport and narrow helper coverage.

### IDC-03 Error-channel fidelity

Need:
1. deterministic translation of `HRESULT`,
2. meaningful use of `ArgErr`,
3. explicit treatment of `VarResult`,
4. bounded but faithful `ExcepInfo` mapping.

Current gap:
1. error mapping is improved, but still not documented or exercised as a parity-closed broad invoke surface.

### IDC-04 Real automation coverage

Need:
1. controlled fixture coverage for exact invoke semantics,
2. registered external automation coverage against practical servers,
3. at least one Office-relevant late-bound evidence lane beyond the synthetic fixture-only path.

Current gap:
1. current lanes prove the subset, not the whole Office-style dispatch surface.

## 6. Execution phases

### Phase A. Contract lock

Deliverables:
1. update the late-bound COM bridge/spec docs so the remaining unsupported areas are explicit,
2. define the exact supported invoke matrix to implement now,
3. define explicit non-goals for the first completion pass.

Primary outputs:
1. updated `COM_CLIENT_LATEBOUND_BRIDGE_V1.md`
2. updated `COM_CLIENT_SERVER_SCOPE_V1.md`
3. updated `COM_CLIENT_SERVER_CONFORMANCE_V1.md`

### Phase B. `DISPPARAMS` and named-arg support

Deliverables:
1. request model extension for named args and omission metadata,
2. native `DISPPARAMS` builder that can populate named args correctly,
3. deterministic compiler/runtime lowering for supported named/optional forms.

Acceptance:
1. method/property invoke shapes with named args no longer collapse into positional-only subset semantics.

### Phase C. `VARIANT` / object / array marshalling expansion

Deliverables:
1. supported Automation type matrix for late-bound client calls,
2. object-valued argument and result handling,
3. SAFEARRAY argument/result handling for supported element types,
4. explicit byref legality handling and stable failure mapping where unsupported.

Acceptance:
1. supported argument/result categories cross the invoke boundary without integer-token-only distortion.

### Phase D. Error-channel and `Err` fidelity

Deliverables:
1. stable mapping rules for `HRESULT`, `ArgErr`, `VarResult`, and bounded `ExcepInfo`,
2. host/runtime assertions for `On Error Resume Next` behavior in invoke failures,
3. conformance evidence for the main failure classes.

Acceptance:
1. failure behavior is deterministic and closer to real VBA late-bound expectations.

### Phase E. Real-lane evidence closure

Deliverables:
1. controlled fixture matrix for:
   - positional multi-arg calls,
   - named args,
   - property `put`,
   - property `putref`,
   - object results,
   - array arguments/results
2. registered-lane evidence for the same categories where feasible,
3. at least one Office-relevant automation lane if environment and repeatability make that defensible.

Acceptance:
1. the supported late-bound client surface is proven by runnable evidence, not just design notes.

## 7. Concrete acceptance matrix

The completion pass should cover at minimum:

1. Positional method invoke:
   - `Foo(a, b, c)`
2. Property get:
   - `obj.Value`
3. Property put:
   - `obj.Value = 42`
4. Property putref:
   - `Set obj.Child = other`
5. Named-argument invoke:
   - supported subset only, but real `rgdispidNamedArgs` packing
6. Optional/default omission:
   - supported subset only, but omission semantics explicit and tested
7. Object result:
   - late-bound call returning another automation object
8. Array result:
   - supported SAFEARRAY subset
9. Error path:
   - missing member
   - bad arg count
   - bad arg type / bad arg index
   - native server exception path

## 8. Design constraints

1. Do not re-expand HAL into the long-term COM home.
2. New late-bound invoke/marshalling logic should prefer `oxvba-com` ownership wherever practical.
3. Keep deterministic unsupported behavior on non-Windows profiles.
4. Do not claim “full VBA parity” until the acceptance matrix and evidence actually support that claim.

## 9. Verification

Core verification:

```powershell
cargo test -p oxvba-com -p oxvba-hal -p oxvba-vm -p oxvba-host --quiet
cargo test -p oxvba-host --test com_client_end_to_end -- --test-threads=1 --nocapture
cargo test -p oxvba-host --test com_client_registered_lane -- --ignored --test-threads=1 --nocapture
./scripts/check-governance.ps1
./scripts/meta-check.ps1 -Fast -NoArtifacts
```

Additional target verification:

```powershell
pwsh -ExecutionPolicy Bypass -File scripts/run-com-registered-events.ps1 -ProgId OxVba.TestEventServer -EnableTrace
pwsh -ExecutionPolicy Bypass -File scripts/run-com-registered.ps1
```

## 10. Exit criteria

This workset is complete when:
1. the supported late-bound client invoke matrix is explicitly documented,
2. named/optional argument support for the chosen subset is real, not deferred text,
3. object/array/value marshalling covers the supported Automation matrix,
4. property `get` / `put` / `putref` are evidence-backed,
5. failure-channel mapping is documented and tested,
6. controlled and registered lanes demonstrate the supported surface,
7. repo docs can honestly say OxVba supports the defined Windows late-bound `IDispatch` client scope without hand-waving.

## 11. Related documents

- `docs/worksets/WORKSET_2026-03-09_COM_INTEROP_CONTINUATION_MULTIARG_LATEBOUND_AND_EVENT_PROJECTION.md`
- `docs/worksets/WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md`
- `docs/spec/COM_CLIENT_LATEBOUND_BRIDGE_V1.md`
- `docs/spec/COM_CLIENT_SERVER_SCOPE_V1.md`
- `docs/spec/COM_CLIENT_SERVER_CONFORMANCE_V1.md`
- `docs/evidence/hal/HAL_IMPLEMENTATION_DEFINED.md`
