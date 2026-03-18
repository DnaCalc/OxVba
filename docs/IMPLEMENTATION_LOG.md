## 2026-03-18 - Start IP-05A metadata-authority checklist

- Started execution from [WORKSET_2026-03-18_IP-05A_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-05A_EXECUTION_CHECKLIST.md) so the COM reference-facade phase now runs against an explicit metadata-authority contract instead of open-ended early-bind frontier notes.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), changed the supported external early-bound member-call rewrite path to resolve member tokens from `oxvba-com` synthetic typelib metadata via `known_typelib_identity_for_prog_id_name(...)`, `build_typelib_metadata(...)`, and `member_token_and_spec_from_typelib_metadata_name(...)` instead of the compiler-local hardcoded external member-token switch.
- In [typelib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib.rs), [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), [typelib_cache.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_cache.rs), and [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs), added synthetic activation metadata so the current supported imported types can expose deterministic `CreateObject` selector ownership through the same reference facade.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), changed the supported external `As New` rewrite path to resolve its deterministic `CreateObject` selector from `oxvba-com` synthetic typelib metadata instead of the compiler-local hardcoded selector switch.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs) and [com_early_project_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_early_project_end_to_end.rs), added deterministic metadata-backed external arity validation so wrong-arity early-bound imported-member calls now fail at compile time on `BIND-E-TYPELIB-INVOKE-ARITY-UNSUPPORTED` instead of deferring the mismatch to runtime dispatch.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), added direct compiler proof that the metadata-backed lookup returns the expected controlled token for `OxVba.TestDispatch.Count` while still rejecting unsupported external members deterministically.

## 2026-03-18 - Add DISP_E_PARAMNOTFOUND host fault classification evidence

- Continued the active `IP-03A` late-bound COM fault-surface sweep instead of widening transport semantics.
- Added a controlled dispatch fixture lane that returns `DISP_E_PARAMNOTFOUND` without synthetic `arg_err`, and wired host VM/JIT end-to-end evidence through runtime-string `DispatchInvoke(...)`.
- The adapter fault prefix now classifies this HRESULT as `com-dispatch-param-not-found` instead of dropping it into the generic native-failure bucket.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) and [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) so the bounded direct-error register reflects the newly covered fault lane honestly.
- `IP-03` remains `in-progress`: broader non-`IDispatch` interface transport, broader multi-dimensional SAFEARRAY parity, fuller external `VARIANT` parity, and richer external automation `VarResult` / `ExcepInfo` / argument-fault coverage are still open.

## 2026-03-18 - Classify bounded E_NOINTERFACE non-IDispatch faults

- Continued the same `IP-03A` late-bound COM fault-surface sweep instead of widening non-`IDispatch` transport.
- Promoted bounded `IUnknown::QueryInterface(IDispatch)` rejection from the generic `com-dispatch-fault-unspecified` bucket into a stable `com-dispatch-no-interface` classification.
- Reused the existing plain `VT_UNKNOWN`, typed `VT_UNKNOWN` SAFEARRAY, and `VT_VARIANT` SAFEARRAY lanes that intentionally return non-`IDispatch` payloads, so the slice remains classification-only rather than a carrier expansion.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) and [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) so the bounded non-`IDispatch` rejection register is explicit.
- `IP-03` remains `in-progress`: broader non-`IDispatch` interface transport, broader multi-dimensional SAFEARRAY parity, fuller external `VARIANT` parity, and richer external automation `VarResult` / `ExcepInfo` / argument-fault coverage are still open.

## 2026-03-18 - Classify bounded internal dispatch conversion faults

- Continued the `IP-03A` host fault-surface sweep on the remaining deterministic dispatch faults that were still rendered as `com-dispatch-fault-unspecified`.
- Added stable internal labels for the two current no-HRESULT conversion families:
  - `com-dispatch-carrier-overflow` for out-of-lane integer result conversions that exceed the current `i32` carrier,
  - `com-dispatch-unsupported-byref-return` for intentionally unsupported `VT_BYREF` return payloads.
- Reused the existing controlled wide-integer and `VT_BYREF` fixtures, so the slice stays classification-only and does not widen the supported automation carrier.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) and [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) so the bounded internal-failure surface is explicit instead of hidden behind the unspecified bucket.
- `IP-03` remains `in-progress`: broader non-`IDispatch` interface transport, broader multi-dimensional SAFEARRAY parity, fuller external `VARIANT` parity, and richer external automation `VarResult` / `ExcepInfo` / argument-fault coverage are still open.

## 2026-03-18 - Lock bounded dispatch fault labels with direct unit evidence

- Added direct Windows HAL unit coverage for `ComInvokeFailure::classification_label()` so the new detail-based `carrier-overflow` and `unsupported-byref-return` labels are not protected only by end-to-end host tests.
- Corrected the stale blocker wording that still described bounded non-`IDispatch` `VT_UNKNOWN` rejection as a generic native-failure path even though the host surface now classifies it as `E_NOINTERFACE`.

## 2026-03-18 - Close IP-02 native/property/default-member scope

- Completed the checklist audit in [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) instead of leaving `IP-02` open behind generic “broader/wider” wording.
- Explicitly classified the remaining native getter syntax surface as either:
  - executable in the supported native scope, or
  - intentionally unsupported with deterministic diagnostics.
- Explicitly closed the assignment-intent table for:
  - plain scalar sources,
  - plain `Object` sources,
  - object-producing call results,
  - declared-`Variant` sources with runtime payload validation,
  - scalar/object native property/default-member getter results.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) and [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) so `IP-02` is no longer described as an active semantic gap.
- `IP-02` is now closed for the scoped native/property/default-member `DG-03` target. Remaining late-bound COM default-member parity continues under `IP-03`, and broader oracle/formal program work continues under `IP-10` / `IP-11`.

## 2026-03-18 - Add runtime-validated plain Variant-source assignment intent evidence

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the explicit `Set`/`Let` source-target table instead of leaving declared `Variant` sources as a runtime blind spot.
- Added a bytecode-level runtime assignment validator so plain declared-`Variant` source variables now have direct bounded assignment-intent coverage across the current `Variant` / `Object` / scalar target lanes for both scalar-payload and object-payload shapes:
  - `Set` now requires an object payload even when the source is only declared `Variant`,
  - implicit assignment to typed `Object` targets now distinguishes runtime object payloads (`Set required ...`) from runtime scalar payloads (`cannot assign Long ...`),
  - explicit `Let` / implicit assignment to scalar targets now reject runtime object payloads instead of silently storing them.
- Added compiler proof that the lowered bytecode now preserves this validation on both explicit-`Set` `Variant` targets and implicit typed-`Object` targets, and removed an optimizer rewrite that had been unsafely collapsing post-typecheck `Variant`-source assignments into semantically different constant-source assignments.
- Added VM/JIT host evidence for both payload families:
  - scalar-payload `Variant` sources now execute only on the supported `Let` / implicit scalar-or-`Variant` lanes and fail deterministically on explicit `Set` or typed-`Object` mismatch lanes,
  - object-payload `Variant` sources now execute on the supported explicit-`Set` / `Variant` lanes and fail deterministically on omitted-`Set` typed-`Object` and scalar-target mismatch lanes.
- `IP-02` remains `in-progress`: broader `Set`/`Let` source-target parity beyond the now-proved plain scalar/object/declared-`Variant` source variable subsets, broader non-authoritative default-member closure beyond the currently bounded subsets, and wider Office-style call-vs-value parity are still open.

## 2026-03-18 - Add scalar-typed getter Let and implicit assignment evidence

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the scalar-typed getter source-target table after locking the explicit-`Set` rejection column.
- Added compiler and VM/JIT host evidence proving that scalar-typed native property/default-member getter results now have direct bounded coverage for:
  - explicit `Let` success into typed `Variant` and scalar targets,
  - implicit assignment success into typed `Variant` and scalar targets,
  - explicit `Let` rejection on typed `Object` targets,
  - implicit assignment rejection on typed `Object` targets,
  - across named, zero-arg parenthesized, indexed, authoritative default-member, and bounded single-visible-candidate non-authoritative default-member syntax.
- The implicit `Object`-target rejection contract on this surface is the current `cannot assign Long to Object variable ...` assignment-from-call diagnostic, which is now locked as direct evidence instead of inferred from plain scalar-source lanes.
- `IP-02` remains `in-progress`: broader `Set`/`Let` source-target parity, broader non-authoritative default-member closure beyond the current bounded subsets, and wider Office-style call-vs-value parity are still open.

## 2026-03-18 - Add scalar-typed getter explicit Set rejection evidence

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the explicit source-target matrix and corrected the test model before widening semantics: the new lanes now use true scalar-typed `Property Get` declarations instead of omitted-`As` `Variant` returns.
- Added compiler and VM/JIT host evidence proving that scalar-typed native property/default-member getter results reject explicit `Set` across typed `Variant`, `Object`, and scalar targets for:
  - named property syntax,
  - zero-arg parenthesized syntax,
  - indexed syntax,
  - authoritative default-member syntax,
  - bounded single-visible-candidate non-authoritative default-member syntax.
- `IP-02` remains `in-progress`: broader `Set`/`Let` source-target parity, broader non-authoritative default-member closure beyond the current bounded subsets, and wider Office-style call-vs-value parity are still open.

## 2026-03-18 - Expand no-paren getter rejection target-matrix evidence

- Continued the active `IP-02A` checklist run on the call-vs-value sweep without widening semantics.
- Added compiler and VM/JIT host evidence proving that no-parentheses getter RHS read-assignment remains on the current compile-time `unsupported statement` surface across typed `Variant`, `Object`, and scalar targets for explicit `Set`, explicit `Let`, and implicit assignment on named property, authoritative default-member, and bounded single-candidate non-authoritative default-member receivers.
- `IP-02` remains in-progress: broader `Set`/`Let` source-target parity, broader non-authoritative default-member closure beyond the current bounded subsets, and wider Office-style call-vs-value parity are still open.

## 2026-03-18 - Added plain scalar-source assignment intent evidence

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the explicit source-target matrix instead of leaving the plain scalar lanes partially implied by generic arithmetic coverage and the earlier object-source/object-result slices.
- Added compiler and host evidence proving that plain scalar sources now have direct bounded assignment-intent coverage for:
  - explicit `Let` into `Variant` targets,
  - explicit `Let` into scalar targets,
  - implicit assignment into `Variant` targets,
  - implicit assignment into scalar targets,
  - explicit `Set` rejection on typed scalar targets in addition to the already-proved `Variant` and `Object` target rejection lanes.
- `IP-02` remains `in-progress`: broader `Set`/`Let` source-target parity, broader non-authoritative default-member closure beyond the currently bounded subsets, and wider Office-style call-vs-value parity are still open.

## 2026-03-18 - Added plain object-source assignment intent evidence

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the explicit source-target matrix instead of leaving plain object-variable sources implied by the earlier `CreateObject(...)` and getter-result lanes.
- Added compiler and host evidence proving that a plain `Object`-typed source variable now has direct bounded assignment-intent coverage for:
  - `Set` into `Variant` targets,
  - `Set` into `Object` targets,
  - explicit `Let` into `Variant` targets,
  - implicit assignment into `Variant` targets.
- Added matching compiler and host evidence proving deterministic rejection for the neighboring mismatch lanes:
  - explicit `Let` into `Object` targets,
  - implicit assignment into `Object` targets,
  - explicit `Set`, explicit `Let`, and implicit assignment into scalar targets.
- `IP-02` remains `in-progress`: broader `Set`/`Let` source-target parity, broader non-authoritative default-member closure beyond the currently bounded subsets, and wider Office-style call-vs-value parity are still open.

## 2026-03-18 - Added no-parentheses getter RHS rejection evidence

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the call-vs-value matrix instead of widening unsupported RHS syntax into execution semantics.
- Added compiler and host phased evidence proving that no-parentheses getter calls remain unsupported in read-assignment RHS contexts and fail deterministically on the current compile-time surface for:
  - named property receivers,
  - authoritative default-member receivers,
  - bounded single-visible-candidate non-authoritative default-member receivers,
  - under both explicit `Let` and implicit assignment.
- The current rejection contract is the existing compile-time `unsupported statement` surface after the partial receiver rewrite; this is now locked as direct evidence instead of leaving the family implied by adjacent statement-context and `Call`-statement support.
- `IP-02` remains `in-progress`: broader `Set`/`Let` source-target parity, broader non-authoritative default-member closure beyond the currently bounded subsets, and wider Office-style call-vs-value parity are still open.

## 2026-03-18 - Added non-authoritative Variant-target read-assignment diagnostic evidence

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) without widening semantics and added the missing typed-`Variant` diagnostic neighbors on the non-authoritative object-valued default-member source-resolution path.
- Added compiler and host phased evidence proving that ambiguous and `no viable candidate` non-authoritative default-member resolution diagnostics now directly cover:
  - explicit `Let valueOut = widget`,
  - implicit `valueOut = widget`,
  - the same typed-`Variant` target lanes across zero-arg parenthesized and indexed syntax.
- `IP-02` remains `in-progress`: broader `Set`/`Let` source-target parity, broader non-authoritative default-member closure beyond the currently bounded subsets, and wider Office-style call-vs-value parity are still open.

## 2026-03-18 - Added bounded non-authoritative Object-target rejection evidence for object-valued default members

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the remaining object-getter source-target matrix instead of treating the bounded single-visible-candidate non-authoritative `Object`-target column as implied by the already-landed `Variant`-target and scalar-target neighbors.
- Added compiler and host phased evidence proving object-returning bounded single-visible-candidate non-authoritative native default-member getter read-assignment also fails deterministically for typed `Object` targets when the assignment intent is not explicit `Set`, across:
  - bare default-member syntax: `Let childOut = widget` and `childOut = widget`,
  - zero-arg parenthesized default-member syntax: `Let childOut = widget()` and `childOut = widget()`,
  - indexed default-member syntax: `Let childOut = widget(x)` and `childOut = widget(x)`.
- The current rejection contract is now locked at both compiler and host levels for the bounded non-authoritative object-getter subset:
  - explicit `Let` fails with `Let cannot assign to Object variable childOut`,
  - implicit assignment fails with `Set required for Object variable childOut`.
- `IP-02` remains `in-progress`: this closes the bounded single-visible-candidate non-authoritative `Object`-target rejection neighbors for object-valued native default members, but broader `Set` vs `Let` intent parity, broader fallback/recovery parity, and wider Office-style call-vs-value closure remain open.
## 2026-03-18 - Added authoritative Object-target rejection evidence for object-valued native getters

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the remaining object-getter source-target matrix instead of assuming the `Object`-target rejection column was already implied by the earlier `CreateObject(...)` and explicit-`Set` coverage.
- Added compiler and host phased evidence proving object-returning authoritative native property/default-member getter read-assignment now also fails deterministically for typed `Object` targets when the assignment intent is not explicit `Set`, across:
  - named member syntax: `Let childOut = widget.Value` and `childOut = widget.Value`,
  - named zero-arg parenthesized member syntax: `Let childOut = widget.Value()` and `childOut = widget.Value()`,
  - indexed member syntax: `Let childOut = widget.Value(x)` and `childOut = widget.Value(x)`,
  - authoritative default-member syntax: `Let childOut = widget`, `childOut = widget`, `Let childOut = widget()`, `childOut = widget()`, `Let childOut = widget(x)`, and `childOut = widget(x)`.
- The current rejection contract is now locked at both compiler and host levels:
  - explicit `Let` fails with `Let cannot assign to Object variable childOut`,
  - implicit assignment fails with `Set required for Object variable childOut`.
- `IP-02` remains `in-progress`: this closes the authoritative `Object`-target rejection column for object-valued native getters, but the bounded non-authoritative `Object`-target neighbors, broader `Set` vs `Let` intent parity, broader fallback/recovery parity, and wider Office-style call-vs-value closure remain open.

## 2026-03-18 - Expanded scalar-target rejection evidence for object-valued native getters

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the remaining object-getter source-target matrix instead of moving on with the parenthesized/indexed scalar-target neighbors still implied by adjacent lanes.
- Added compiler and host phased evidence proving object-returning native property/default-member getter read-assignment still rejects typed scalar targets with the current deterministic compile-time diagnostic across:
  - named zero-arg parenthesized member syntax: `Let n = widget.Value()` and `n = widget.Value()`,
  - indexed member syntax: `Let n = widget.Value(x)` and `n = widget.Value(x)`,
  - authoritative zero-arg parenthesized and indexed default-member syntax: `Let n = widget()`, `n = widget()`, `Let n = widget(x)`, and `n = widget(x)`,
  - bounded single-visible-candidate non-authoritative zero-arg parenthesized and indexed default-member syntax: `Let n = widget()`, `n = widget()`, `Let n = widget(x)`, and `n = widget(x)`.
- This confirms the current PMR/default-member typechecking contract is stable across the widened syntax matrix: object-valued getter results still fail with `cannot assign Object to Long variable n` instead of silently narrowing or escaping rewrite.
- `IP-02` remains `in-progress`: this closes the remaining parenthesized/indexed scalar-target object-getter rejection neighbors on the current native/default-member surface, but broader `Set` vs `Let` intent parity, broader fallback/recovery parity, and wider Office-style call-vs-value closure remain open.

## 2026-03-18 - Added scalar-target rejection evidence for object-valued native getters

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the `Set`/`Let` source-target matrix instead of widening the PMR object-getter surface again.
- Added compiler and host phased evidence proving object-returning native property/default-member getter read-assignment still rejects scalar targets with the current deterministic compile-time diagnostic across:
  - named member syntax: `Let n = widget.Value` and `n = widget.Value`,
  - authoritative bare default-member syntax: `Let n = widget` and `n = widget`,
  - bounded single-visible-candidate non-authoritative bare default-member syntax: `Let n = widget` and `n = widget`.
- This locks the current typechecking contract where object-valued PMR/default-member getter results do not silently narrow into typed scalar targets and instead fail with `cannot assign Object to Long variable n`.
- `IP-02` remains `in-progress`: this closes another bounded object-getter rejection neighbor, but broader `Set` vs `Let` intent parity, broader fallback/recovery parity, and wider Office-style call-vs-value closure remain open.

## 2026-03-18 - Expanded non-authoritative object-default-member Variant-target assignment evidence

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the remaining `Set`/`Let` object-getter matrix instead of stopping after the authoritative syntax surface.
- Added compiler and host phased evidence proving the bounded single-visible-candidate non-authoritative native default-member fallback now also preserves the `Variant`-target success lanes for explicit `Set`, explicit `Let`, and implicit assignment across:
  - bare default-member syntax: `widget`,
  - zero-arg parenthesized syntax: `widget()`,
  - indexed syntax: `widget(x)`.
- This locks the current fallback contract where the dynamic member resolver already has exactly one viable object-returning default-member candidate and confirms that the same legacy `RuntimeValue::I32(...)` handle shape is preserved on the phased host snapshot surface.
- `IP-02` remains `in-progress`: this closes the bounded non-authoritative object-default-member `Variant` neighbors, but broader `Set` vs `Let` intent parity, broader fallback/recovery parity, and wider Office-style call-vs-value closure remain open.

## 2026-03-18 - Expanded authoritative object-getter Variant-target assignment evidence

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the `Set`/`Let` source-target matrix instead of leaving the remaining `Variant` object-getter lanes implied by adjacent `Object`-target coverage.
- Added compiler and host phased evidence proving authoritative object-returning native property/default-member getters now preserve the bounded `Variant`-target success lanes for explicit `Set`, explicit `Let`, and implicit assignment across:
  - named zero-arg parenthesized member syntax: `widget.Value()`,
  - indexed member syntax: `widget.Value(x)`,
  - authoritative default-member syntax: `widget`, `widget()`, and `widget(x)`.
- The current phased host snapshot contract remains stable across these lanes: the receiver/object result still surfaces through the same legacy `RuntimeValue::I32(...)` handle shape already used by the neighboring object-target PMR evidence.
- `IP-02` remains `in-progress`: this closes the authoritative object-getter `Variant` syntax matrix, but non-authoritative `Variant` object-getter neighbors, broader `Set` vs `Let` parity, and wider Office-style call-vs-value closure remain open.

## 2026-03-18 - Added named object-property-get Variant-target assignment evidence

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) on the `Set`/`Let` source-target matrix instead of adding more syntax-only examples.
- Added compiler and host phased evidence proving named object-returning native `Property Get` results now preserve the bounded `Variant`-target success lanes for:
  - explicit `Set valueOut = widget.Value`,
  - explicit `Let valueOut = widget.Value`,
  - implicit `valueOut = widget.Value`.
- On the current phased host snapshot surface these lanes project the same stable object-handle shape already used by the existing object-target PMR evidence, so the new tests lock the actual contract instead of assuming a different snapshot API.
- `IP-02` remains `in-progress`: this closes the named non-indexed object-property-get `Variant` neighbor, but indexed/default-member object-getter lanes and broader `Set` vs `Let` parity remain open.

## 2026-03-18 - Expanded no-parentheses default-member evidence on the bounded native fallback surface

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) and closed the next call-vs-value neighbor instead of widening semantics blindly.
- Added compiler and host phased evidence proving the existing native no-parentheses-argument default-member getter rewrite now also covers the bounded non-authoritative single-visible-candidate lane:
  - `widget x`
- Added deterministic diagnostic evidence for the same no-parentheses-argument default-member getter shape when native non-authoritative fallback is not resolvable:
  - ambiguous candidate set -> `PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS`
  - no viable candidate -> `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING`
- `IP-02` remains `in-progress`: this closes the bounded no-parentheses default-member getter neighbor, but broader call-vs-value enumeration and broader `Set` vs `Let` parity remain open.

## 2026-03-18 - Expanded missing and ambiguous non-authoritative default-member evidence

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) and widened direct proof for the already-landed non-authoritative diagnostic surface instead of introducing new semantics.
- Added compiler and host phased evidence proving bounded native non-authoritative default-member diagnostics now also hold for:
  - indexed `Let` assignment in both missing and ambiguous form: `widget(x) = 9`,
  - ambiguous indexed getter and indexed `Property Set`,
  - ambiguous statement-context and explicit `Call` getter forms in scalar, indexed, and zero-arg parenthesized shape,
  - missing statement-context and explicit `Call` zero-arg parenthesized getter forms: `widget()` and `Call widget()`.
- `IP-02` remains `in-progress`: this closes more bounded diagnostic neighbors on the active checklist, but broader call-vs-value syntax closure and broader `Set` vs `Let` parity remain open.

## 2026-03-18 - Added indexed property-set evidence for missing non-authoritative default-member diagnostics

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) and extended direct proof for the already-landed `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING` path onto the indexed default-member `Property Set` neighbor.
- Added compiler and host phased evidence proving `Set widget(1) = x` now fails deterministically at compile time when a native internal-class receiver has no authoritative default-member metadata and no visible `Property Set` candidate of the requested kind exists.
- `IP-02` remains `in-progress`: this closes the bounded indexed property-set missing-candidate neighbor, but broader ambiguity neighbors, broader `Set` vs `Let` intent parity, and wider Office-style call-vs-value parity still remain open.

## 2026-03-18 - Expanded missing non-authoritative default-member evidence across getter contexts

- Continued execution from [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) and widened proof coverage for the already-landed `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING` path instead of introducing new semantics.
- Added compiler and host phased evidence proving bounded native non-authoritative default-member `no viable candidate` getter diagnostics now hold for:
  - indexed read-assignment: `valueOut = widget(x)`,
  - explicit `Call` scalar and indexed getter forms: `Call widget`, `Call widget(x)`,
  - statement-context scalar and indexed getter forms: `widget`, `widget(x)`.
- `IP-02` remains `in-progress`: this extends direct proof coverage for the missing-candidate getter subset, but indexed property-set missing-candidate neighbors, broader ambiguity neighbors, broader `Set` vs `Let` intent parity, and wider Office-style call-vs-value parity still remain open.

## 2026-03-18 - Added bounded missing non-authoritative default-member diagnostics

- Started execution from the new [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md) so `IP-02` progress is now driven by an explicit lane matrix and exit gate rather than ad hoc frontier notes.
- Continued the active `IP-02A` semantic-closure slice and converted bounded native non-authoritative default-member `no viable candidate` cases from silent "no rewrite" escape into deterministic PMR compile-time failure.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), added `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING` and now raise it when a native internal-class receiver has no authoritative default-member metadata and no visible candidate of the requested kind exists.
- Added compiler and host phased evidence for the initial bounded missing-candidate lanes:
  - `valueOut = widget` now fails at compile time when `Widget` exposes no visible `Property Get` candidate without authoritative default-member metadata.
  - `widget = 9` now fails at compile time when `Widget` exposes no visible `Property Let` candidate without authoritative default-member metadata.
  - `Set widget = x` now fails at compile time when `Widget` exposes no visible `Property Set` candidate without authoritative default-member metadata.
- `IP-02` remains `in-progress`: this only closes the first scalar getter/let/property-set `no viable candidate` diagnostics in the bounded non-authoritative native subset; indexed/call/statement neighbors, broader `Set` vs `Let` parity, broader non-authoritative/default-member resolution, and wider Office-style call-vs-value parity still remain open.

## 2026-03-17 - Bounded ambiguous non-authoritative default-member fallback with a PMR diagnostic

- Continued the active `IP-02A` semantic-closure slice and converted ambiguous native non-authoritative default-member fallback from a silent "no rewrite" escape hatch into a deterministic PMR compile-time failure.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), added `PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS` and now raise it when a native internal-class receiver has no authoritative default-member metadata and more than one visible candidate of the requested kind is present.
- Added compiler and host phased evidence for the bounded ambiguous lanes:
  - `valueOut = widget` now fails at compile time when `Widget` exposes multiple visible `Property Get` candidates without authoritative default-member metadata.
  - `widget = 9` now fails at compile time when `Widget` exposes multiple visible `Property Let` candidates without authoritative default-member metadata.
  - `Set widget = x` now fails at compile time when `Widget` exposes multiple visible `Property Set` candidates without authoritative default-member metadata.
- Added compiler and host evidence for the bounded single-candidate non-authoritative native default-member `Property Set` lanes:
  - `Set widget = x`
  - `Set widget(1) = x`
- `IP-02` remains `in-progress`: this only extends the bounded non-authoritative native subset through getter/let/set ambiguity diagnostics plus single-candidate `Property Set`; broader `Set` vs `Let` parity, broader non-authoritative/default-member resolution, and wider Office-style call-vs-value parity still remain open.

## 2026-03-16 - Added parenthesized explicit Let getter evidence

- Continued the active `IP-02A` read-assignment closure slice and locked the zero-arg parenthesized explicit-`Let` neighbors instead of leaving them implied by the earlier bare/indexed `Let` and parenthesized statement/`Call` getter coverage.
- Added compiler and host evidence for:
  - `Let valueOut = widget.Value()`
  - `Let valueOut = widget()` when authoritative default-member metadata exists
  - `Let valueOut = widget()` under the existing bounded single-visible-candidate non-authoritative fallback
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` parity, ambiguous or broader non-authoritative default-member resolution, and wider Office-style call-vs-value parity still remain open.

## 2026-03-16 - Added parenthesized explicit Set object-getter evidence

- Continued the active `IP-02A` assignment-intent closure slice and locked the zero-arg parenthesized explicit-`Set` read-assignment neighbors instead of leaving them implied by the earlier non-parenthesized and statement-context getter coverage.
- Added compiler and host evidence for:
  - `Set childOut = widget.Value()`
  - `Set childOut = widget()` when authoritative default-member metadata exists
  - `Set childOut = widget()` under the existing bounded single-visible-candidate non-authoritative fallback
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` parity, ambiguous or broader non-authoritative default-member resolution, and wider Office-style call-vs-value parity still remain open.

## 2026-03-16 - Tightened bounded implicit Object assignment to require Set

- Continued the active `IP-02A` assignment-intent closure slice and removed the earlier bounded implicit `Object`-target object-call success lane once it was clear that the compiler and project rewriter were still accepting or emitting invalid omitted-`Set` syntax.
- In [typecheck.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\typecheck.rs), tightened assignment-intent validation so implicit assignment now rejects known object-producing values for `Object` targets with a stable `Set required for Object variable ...` diagnostic while keeping the bounded implicit `Variant` object-result lane intact.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), changed lowered external `As New` initialization to emit `Set obj = CreateObject(...)` so rewrite-based early-bound lowering no longer manufactures invalid implicit object assignment.
- Updated compiler and host evidence for:
  - `obj = CreateObject(4)` rejecting on a typed `Object` target with a stable `Set required ...` diagnostic
  - early-bound external member-call fixtures using explicit `Set obj = CreateObject(4)` instead of the invalid omitted-`Set` form
  - lowered external `As New` rewrite emitting `Set obj = CreateObject(4)`
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` parity, ambiguous or broader non-authoritative default-member resolution, and wider Office-style call-vs-value parity still remain open.

## 2026-03-16 - Extended IP-02 bounded non-authoritative default-member Set read evidence

- Continued the active `IP-02A` default-member closure slice and proved that the existing single-visible-candidate non-authoritative fallback also carries explicit `Set` read-assignment for object-returning native default members instead of only scalar `Get`/`Let` behavior.
- Added compiler and host evidence for:
  - `Set childOut = widget` with a single visible object-returning `Property Get`
  - `Set childOut = widget(x)` with a single visible indexed object-returning `Property Get`
- `IP-02` remains `in-progress`: ambiguous or broader non-authoritative default-member resolution and wider Office-style call-vs-value parity still remain open, along with broader `Set`/`Let` parity outside the bounded lanes now covered.

## 2026-03-16 - Extended IP-02 explicit Set default-member object property-get evidence

- Continued the active `IP-02A` assignment-intent closure slice and fixed the default-member read-assignment rewrite so explicit `Set` now follows the same PMR-backed object getter path as the existing implicit and explicit `Let` read-assignment forms.
- Added compiler and host evidence for:
  - `Set childOut = widget` when authoritative `VB_UserMemId = 0` metadata marks an object-returning native internal-class default member
  - `Set childOut = widget(x)` for the indexed authoritative default-member neighbor on the same object-returning path
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` intent parity beyond the now-covered bounded named/indexed/default-member object lanes, plus broader non-authoritative default-member resolution and Office-style call-vs-value parity, still remain open.

## 2026-03-16 - Extended IP-02 explicit Set indexed native object property-get evidence

- Continued the active `IP-02A` assignment-intent closure slice and extended the new object-returning native PMR `Set` read-assignment evidence to the indexed property-get neighbor.
- Added compiler and host evidence for:
  - `Set childOut = widget.Value(x)` when `Value` is a native internal-class indexed `Property Get` returning an object
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` intent parity beyond the now-covered bounded named/indexed property-get object lanes, plus broader non-authoritative default-member resolution and Office-style call-vs-value parity, still remain open.

## 2026-03-16 - Extended IP-02 explicit Set native object property-get evidence

- Continued the active `IP-02A` assignment-intent closure slice and added a direct native PMR `Set` read-assignment lane for object-returning internal-class properties instead of leaving `Set` parity at plain assignment and property-set-only behavior.
- Added compiler and host evidence for:
  - `Set childOut = widget.Value` when `Value` is a native internal-class `Property Get` returning an object
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` intent parity beyond the now-covered bounded named-property lane, plus broader non-authoritative default-member resolution and Office-style call-vs-value parity, still remain open.

## 2026-03-16 - Extended IP-02 implicit object-call success evidence

- Continued the active `IP-02A` assignment-intent closure slice and locked the direct implicit object-call success neighbors instead of leaving them implied by broader integration behavior.
- Added compiler and host evidence for:
  - `v = CreateObject(4)` succeeding for a `Variant` target
  - `obj = CreateObject(4)` succeeding for an `Object` target
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` / implicit assignment parity beyond the now-covered bounded typed lanes is still open.

## 2026-03-16 - Extended IP-02 typed Variant Set scalar rejection evidence

- Continued the active `IP-02A` assignment-intent closure slice and locked the direct typed `Variant` scalar-source `Set` rejection lane instead of relying on the earlier untyped `Set x = 7` evidence alone.
- Added compiler and host evidence for:
  - `Set v = 7` rejecting a scalar source on a `Variant` target
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` / implicit assignment parity beyond the now-covered bounded typed lanes is still open.

## 2026-03-16 - Extended IP-02 implicit object/scalar assignment evidence

- Continued the active `IP-02A` assignment-intent closure slice and bounded two implicit assignment neighbors instead of leaving them to drift behind the explicit `Set`/`Let` coverage.
- Added compiler and host evidence for:
  - `n = CreateObject(4)` rejecting an object-producing call result on a scalar `Long` target
-  - `obj = 7` rejecting a scalar source on an `Object` target
- This remains intentionally narrow: implicit object-target `CreateObject(...)` assignment still exists as a compiler-managed route for the current early-bound/object-creation subset.
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` / implicit assignment parity beyond the now-covered bounded scalar/object implicit lanes is still open.

## 2026-03-16 - Extended IP-02 typed Let rejection evidence

- Continued the active `IP-02A` assignment-intent closure slice and proved two additional typed `Let` rejection lanes instead of leaving `Let` parity at the `Object`-target object-call rejection case only.
- Added compiler and host evidence for:
  - `Let obj = 7` rejecting a scalar source on an `Object` target
  - `Let n = CreateObject(4)` rejecting an object-producing call result on a scalar `Long` target
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` parity beyond the now-covered bounded success and rejection lanes is still open.

## 2026-03-16 - Extended IP-02 typed Let object-call assignment evidence

- Continued the active `IP-02A` assignment-intent closure slice and proved that the bounded `Let` object-call lane also succeeds for `Variant` targets instead of leaving `Let` coverage at the `Object`-target rejection lane only.
- Added compiler and host evidence for:
  - `Let v = CreateObject(4)`
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` parity beyond the now-covered bounded `Object`/`Variant` object-call lanes and typed rejection lanes is still open.

## 2026-03-16 - Extended IP-02 typed Set rejection evidence

- Continued the active `IP-02A` assignment-intent closure slice and proved two additional typed `Set` rejection lanes instead of relying on only the earlier generic `Variant`-target scalar rejection.
- Added compiler and host evidence for:
  - `Set obj = 7` rejecting a scalar source on an `Object` target
  - `Set n = CreateObject(4)` rejecting an object-producing call result on a scalar `Long` target
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` parity beyond the now-covered bounded success and rejection lanes is still open.

## 2026-03-16 - Extended IP-02 typed Set object-call assignment evidence

- Continued the active `IP-02A` assignment-intent closure slice and proved that the direct `Object` target lane is also accepted for object-producing call results instead of only the previously documented `Variant` target success lane.
- Added compiler and host evidence for:
  - `Set obj = CreateObject(4)`
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` parity beyond the now-covered `Object`/`Variant` object-call success lanes and bounded `Let` rejection lane is still open.

## 2026-03-16 - Extended IP-02 parenthesized statement-context getter evidence

- Continued the active `IP-02A` call-vs-value closure slice and proved that natural zero-arg parenthesized statement-context getter syntax also reaches the existing PMR-backed getter route for native internal project-class property/default-member getters.
- Added compiler and host evidence for:
  - `widget.Value()`
  - `widget()` when authoritative default-member metadata exists
  - `widget()` under the existing bounded single-visible-candidate non-authoritative fallback
- This remains bounded: wider Office-style parenthesized call-vs-value recovery and ambiguous non-authoritative cases are still open.

## 2026-03-16 - Extended IP-02 zero-arg parenthesized Call getter evidence

- Continued the active `IP-02A` call-vs-value closure slice and proved that explicit zero-arg parenthesized `Call` form also reaches the existing PMR-backed getter route for native internal project-class property/default-member getters.
- Added compiler and host evidence for:
  - `Call widget.Value()`
  - `Call widget()` when authoritative default-member metadata exists
  - `Call widget()` under the existing bounded single-visible-candidate non-authoritative fallback
- This remains bounded: wider Office-style parenthesized call-vs-value recovery and ambiguous non-authoritative cases are still open.

## 2026-03-16 - Extended bounded non-authoritative default-member fallback into statement-context form

- Continued the active `IP-02A` semantic-closure slice and proved that the existing single-visible-candidate non-authoritative native default-member fallback also reaches statement-context getter execution instead of stopping at assignment and explicit `Call` evidence.
- Added compiler and host evidence for:
  - bare `widget` in statement context when a native class exposes exactly one visible `Property Get`
  - bare `widget(x)` in statement context when a native class exposes exactly one visible indexed `Property Get`
- This remains bounded, not general non-authoritative closure: ambiguous multi-candidate cases and broader Office-style call-vs-value recovery are still open.

## 2026-03-16 - Extended bounded non-authoritative default-member fallback into explicit Call form

- Continued the active `IP-02A` semantic-closure slice and proved that the existing single-visible-candidate non-authoritative native default-member fallback also reaches explicit `Call` getter contexts instead of stopping at assignment/read-assignment evidence.
- Added compiler and host evidence for:
  - `Call widget` when a native class exposes exactly one visible `Property Get`
  - `Call widget(x)` when a native class exposes exactly one visible indexed `Property Get`
- This remains a bounded subset, not broad non-authoritative closure: ambiguous multi-candidate cases and wider Office-style call-vs-value recovery are still open.

## 2026-03-16 - Extended IP-02 explicit Call indexed native property/default-member evidence

- Continued the active `IP-02A` semantic-closure slice and proved that explicit `Call ...(...)` form for indexed native internal project-class `Property Get` and indexed default-member `Property Get` executes on the same PMR/dynamic-object route as the already-covered statement-context and no-parentheses forms.
- Added compiler and host evidence for:
  - `Call widget.Value(x)`
  - `Call widget(x)` when authoritative `VB_UserMemId = 0` metadata exists
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), added direct lowered-source tests locking the explicit `Call` rewrite boundary for both indexed lanes.
- In [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs), added end-to-end VM/JIT execution lanes proving the indexed argument is passed and updated through the shared getter path in both explicit `Call` forms.
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` parity, ambiguous/broader non-authoritative default-member resolution, and wider Office-style call-vs-value context parity are still open.

## 2026-03-16 - Added bounded non-authoritative native default-member fallback evidence

- Continued the active `IP-02A` semantic-closure slice and added the first bounded non-authoritative native default-member fallback instead of leaving all non-metadata-backed internal-class default-member syntax unresolved.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), extended `resolve_internal_class_default_member_target_of_kinds(...)` so native project-class resolution now falls back only when there is exactly one visible candidate of the requested kind, preserving bounded ambiguity handling.
- Added compiler and host evidence for:
  - `valueOut = widget` when a native class exposes exactly one visible `Property Get`
  - `widget = 9` plus surrounding reads when a native class exposes exactly one visible `Property Get` and exactly one visible `Property Let`
- Added compiler and host evidence for the indexed form of the same bounded fallback:
  - `valueOut = widget(x)` when a native class exposes exactly one visible indexed `Property Get`
  - `widget(x) = 9` plus surrounding reads when a native class exposes exactly one visible indexed `Property Get` and exactly one visible indexed `Property Let`
- This is still a bounded subset, not full closure: ambiguous non-authoritative cases, broader Office-style default-member recovery, and COM-side non-authoritative recovery remain open.

## 2026-03-16 - Extended IP-02 explicit Let native property/default-member read evidence

- Continued the active `IP-02A` semantic-closure slice and proved that explicit `Let` also survives the native PMR-backed property/default-member read-assignment route instead of only the property/default-member write-assignment route.
- Added host rewrite/execution evidence for:
  - `Let valueOut = widget.Value`
  - `Let valueOut = widget`
  - `Let valueOut = widget.Value(x)`
  - `Let valueOut = widget(x)`
- In [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs), added bounded project-execution lanes plus lowered-source assertions proving the compiler now preserves the `Let` prefix while rewriting the native PMR getter route.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), added direct compiler rewrite tests for the same four lanes so the explicit-`Let` guarantee is locked at the PMR rewrite boundary instead of only through host integration evidence.
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` parity, non-authoritative default-member resolution, and wider Office-style call-vs-value context parity are still open.

## 2026-03-16 - Extended IP-02 explicit Let native property/default-member evidence

- Continued the active `IP-02A` semantic-closure slice and proved that explicit `Let` on native internal project-class property/default-member assignment routes through the same PMR/dynamic-object execution path as the already-covered implicit property `Let` subset.
- Added host end-to-end evidence for:
  - `Let widget.Value = 9`
  - `Let widget.Value(2) = 9`
  - `Let widget = 9` when authoritative `VB_UserMemId = 0` metadata exists
  - `Let widget(2) = 9` when authoritative `VB_UserMemId = 0` metadata exists
- In [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs), added the bounded VM/JIT project-execution lanes above and verified that explicit `Let` preserves the same pre/post property-get observations as the implicit syntax.
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` parity, non-authoritative default-member resolution, and wider Office-style call-vs-value context parity are still open.

## 2026-03-16 - Extended IP-02 bounded typed Set/Let call-assignment evidence

- Continued the active `IP-02A` semantic-closure slice and proved two additional end-to-end typed/object assignment-intent lanes instead of leaving the earlier `Set`/`Let` work at plain variable assignment coverage only.
- Added compiler and host evidence that:
  - `Set` accepts an object-producing call result for a `Variant` target: `Set v = CreateObject(4)`.
  - `Let` still rejects an object-producing call result for an `Object` target with a stable bounded diagnostic: `Let obj = CreateObject(4)`.
- In [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs), added Windows host/JIT-surface tests proving that the `Variant` target preserves the object-handle result shape while the `Let` misuse fails before any runtime drift.
- In [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), added compile-time evidence for the same two lanes so assignment-intent validation remains aligned across compiler and host surfaces.
- `IP-02` remains `in-progress`: broader typed/object `Set` vs `Let` parity, non-authoritative default-member resolution, and wider Office-style call-vs-value context parity are still open.

## 2026-03-16 - Added bounded runtime-string value/default-member-name COM dispatch evidence in `IP-03A`

- Continued the bounded dynamic-name late-bound COM work instead of overclaiming full dynamic-name/property/default-member parity.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), [resolve.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\resolve.rs), [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), and [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs), added controlled fixture members plus metadata/token coverage for `ReturnValueMemberName` and `ReturnDefaultMemberName` (`85` / `86`) so the current fixture can produce both a zero-argument property-get selector and the authoritative default-member name from COM at runtime.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), added VM/JIT host evidence proving:
  - `DispatchInvoke(obj, valueName)` executes zero-argument property-get `Value` when `valueName` is produced dynamically at runtime,
  - `DispatchInvoke(obj, defaultName, value := 19)` executes the authoritative default member `EchoVariant` when `defaultName` is produced dynamically at runtime.
- `IP-03` remains `in-progress`: this slice only proves bounded runtime-string recovery for a zero-arg property-get selector and the authoritative default-member name inside the metadata-backed fixture subset; non-metadata-backed dynamic-name recovery, broader default-member recovery, event queue integration, and broader Office automation parity remain open.

## 2026-03-16 - Added bounded runtime-string indexed property-put COM dispatch evidence in `IP-03A`

- Continued the bounded dynamic-name late-bound COM work instead of overclaiming full dynamic-name/property/default-member parity.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), [resolve.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\resolve.rs), [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), and [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs), added controlled fixture members plus metadata/token coverage for `ReturnSetIndexedValueMemberName` and `ReturnSetIndexedValueRefMemberName` (`83` / `84`) so runtime string selectors for indexed property-put and property-putref members can be produced from COM within the supported subset.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), added VM/JIT host evidence proving:
  - `DispatchInvoke(obj, setIndexedName, 7, 11)` executes `SetIndexedValue` when `setIndexedName` is produced dynamically at runtime,
  - `DispatchInvoke(obj, "Value")` observes the indexed property-put mutation on the same bound object,
  - `DispatchInvoke(obj, setIndexedRefName, 8, 13)` executes `SetIndexedValueRef` when `setIndexedRefName` is produced dynamically at runtime,
  - `DispatchInvoke(obj, "Value")` observes the deterministic indexed property-putref mutation on the same bound object.
- `IP-03` remains `in-progress`: this slice only proves bounded runtime-string indexed property put/property putref execution when authoritative metadata exists; non-metadata-backed dynamic-name recovery, broader default-member recovery, event queue integration, and broader Office automation parity remain open.

## 2026-03-16 - Added bounded runtime-string property-put COM dispatch evidence in `IP-03A`

- Continued the bounded dynamic-name late-bound COM work instead of overclaiming full dynamic-name/property/default-member parity.
- In [windows_bridge.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_bridge.rs), routed runtime string selectors with authoritative typelib metadata back onto the existing token-based bound-dispatch path so exact invoke-kind execution reuses the same bound object identity and stateful property semantics as the compile-time known-member lane.
- In [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs) and [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs), added name-based synthetic typelib lookup so runtime string selectors can recover authoritative controlled member tokens by name inside the current subset.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), [resolve.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\resolve.rs), [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), and [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs), added controlled fixture members plus metadata/token coverage for `ReturnSetValueMemberName` and `ReturnSetValueRefMemberName` (`81` / `82`) so runtime string selectors for property-put and property-putref members can be produced from COM within the supported subset.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), added VM/JIT host evidence proving:
  - `DispatchInvoke(obj, setName, 12)` executes `SetValue` when `setName` is produced dynamically at runtime,
  - `DispatchInvoke(obj, "Value")` observes the state mutation from that runtime-string property-put call,
  - `DispatchInvoke(obj, setRefName, 12)` executes `SetValueRef` when `setRefName` is produced dynamically at runtime,
  - `DispatchInvoke(obj, "Value")` observes the deterministic property-putref mutation on the same bound object.
- `IP-03` remains `in-progress`: this slice only proves bounded runtime-string property put/property putref execution when authoritative metadata exists; non-metadata-backed dynamic-name recovery, broader default-member recovery, event queue integration, and broader Office automation parity remain open.

## 2026-03-16 - Added bounded runtime-string named-argument COM dispatch evidence in `IP-03A`

- Continued the bounded dynamic-name late-bound COM work instead of overclaiming full dynamic-name parity.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), [resolve.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\resolve.rs), [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), and [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs), added controlled fixture members plus metadata/token coverage for `ReturnSumPairMemberName` and `ReturnLookupPairMemberName` (`79` / `80`) so runtime string selectors for named-argument members can be produced from COM within the supported subset.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), added VM/JIT host evidence proving:
  - `DispatchInvoke(obj, methodName, lhs:=12, rhs:=34)` executes `SumPair` when `methodName` is produced dynamically at runtime,
  - `DispatchInvoke(obj, propertyName, lhs:=5, rhs:=9)` executes indexed `LookupPair` when `propertyName` is produced dynamically at runtime.
- `IP-03` remains `in-progress`: this slice only proves bounded named-argument execution on the runtime-string method/property-get subset; dynamic-name property put/putref intent, default-member recovery, event queue integration, and broader Office automation parity remain open.

## 2026-03-16 - Added bounded runtime-string known-member COM dispatch fallback evidence in `IP-03A`

- Continued the bounded dynamic-name late-bound COM work instead of overclaiming full dynamic-name/property/default-member parity.
- In [windows_bridge.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_bridge.rs), widened the runtime string member selector path so native Windows dispatch now retries the opposite invoke flag on `DISP_E_BADPARAMCOUNT`, allowing the current subset to execute both zero-argument methods and indexed property-get members without assuming the initial heuristic is authoritative.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), [resolve.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\resolve.rs), [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), and [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs), added controlled fixture members plus metadata/token coverage for `ReturnPingMemberName` and `ReturnLookupMemberName` (`77` / `78`) so runtime string selectors can be produced from COM at execution time inside the supported subset.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), added VM/JIT host evidence proving:
  - `DispatchInvoke(obj, methodName)` executes `Ping` when `methodName` is produced dynamically at runtime,
  - `DispatchInvoke(obj, propertyName, 42)` executes indexed `Lookup` when `propertyName` is produced dynamically at runtime.
- `IP-03` remains `in-progress`: this is still only a bounded `DISP_E_BADPARAMCOUNT` invoke-flag fallback on the runtime-string subset; dynamic-name property put/putref intent, default-member recovery, event queue integration, and broader Office automation parity remain open.

## 2026-03-16 - Added runtime-string `DISP_E_UNKNOWNNAME` host evidence in `IP-03A`

- Continued the bounded late-bound COM invoke-fidelity work instead of overclaiming broader dynamic-name or Office automation parity.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), added the controlled `ReturnMissingMemberName` fixture member so host execution can obtain the missing member selector at runtime instead of relying on a compile-time string literal.
- In [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), [resolve.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\resolve.rs), and [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), extended the controlled fixture metadata and compiler token coverage for member token `76`.
- In [windows_bridge.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_bridge.rs), [traits.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\traits.rs), [dynamic_bridge.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\dynamic_bridge.rs), and [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs), threaded a bounded dynamic-name dispatch path through the native Windows COM bridge so runtime string member selectors can hit `IDispatch::GetIDsOfNames` and preserve the stable `com-dispatch-unknown-name;hresult=0x80020006;` adapter-fault surface.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), added VM/JIT host evidence proving `DispatchInvoke(obj, missingName)` now surfaces the stable unknown-name classification together with the raw `GetIDsOfNames` detail when `missingName` is produced dynamically at runtime.
- `IP-03` remains `in-progress`: this slice only closes the earlier adapter-only `DISP_E_UNKNOWNNAME` evidence gap for runtime string member selectors; broader dynamic-name property intent, default-member recovery, event queue integration, non-`IDispatch` interface transport, and multi-dimensional SAFEARRAY parity remain open.

## 2026-03-15 - Bounded nested VT_UNKNOWN-in-VT_ARRAY|VT_VARIANT COM result diagnostics
- Added a controlled `ReturnPlainUnknownVariantArray` fixture member together with compiler token coverage, HAL metadata expectations, and host VM/JIT evidence for one-dimensional `VT_ARRAY | VT_VARIANT` results that contain nested plain `VT_UNKNOWN` elements.
- Locked the current adapter contract so nested non-`IDispatch` elements inside variant SAFEARRAY results now fail with the explicit bounded `IUnknown::QueryInterface(IDispatch)` diagnostic instead of depending on undocumented nested-object drift.

## 2026-03-15 - Bounded rank-2 VT_ARRAY|VT_VARIANT COM result diagnostics
- Added a controlled `ReturnVariantMatrix` fixture member together with compiler token coverage, HAL metadata expectations, and host VM/JIT evidence for rank-2 `VT_ARRAY | VT_VARIANT` results.
- Locked the current adapter contract so multi-dimensional variant SAFEARRAY results now fail with the explicit unsupported-rank bounded diagnostic instead of depending on an undocumented shape path.

## 2026-03-15 - Bounded VT_I8/VT_UI8 COM overflow transport on the current i32 lane
- Added controlled `ReturnWideHyper`, `ReturnWideHyperArray`, `ReturnWideUnsignedHyper`, and `ReturnWideUnsignedHyperArray` fixture members together with compiler token coverage, HAL metadata expectations, and host VM/JIT evidence.
- Extended the current bounded-overflow evidence so scalar `VT_I8` / `VT_UI8` values and one-dimensional typed `VT_ARRAY | VT_I8` / `VT_ARRAY | VT_UI8` elements that exceed the current `i32` carrier lane now fail with deterministic diagnostics instead of silently narrowing.

## 2026-03-15 - Bounded unsupported VT_BYREF COM result diagnostics
- Extended the bounded unsupported `VT_BYREF` coverage to include typed `VT_BYREF | VT_ARRAY | VT_I4` result payloads with a controlled `ReturnByRefLongArray` fixture member, compiler/metadata token mapping, COM bridge unit coverage, and host VM/JIT evidence.
- Added an explicit Windows `VARIANT` bridge guard for `VT_BYREF` result payloads, a controlled `ReturnByRefLong` fixture member, compiler/metadata token mapping, COM bridge unit coverage, and host VM/JIT evidence so unsupported byref result shapes now fail with a deterministic bounded diagnostic instead of an undocumented adapter path.

## 2026-03-15 - Preserved outward Single and Date COM vartype fidelity on the tagged f64 carrier
- Added `Single` / `Double` / `Date` subtype tracking on the shared semantic `f64` carrier and threaded it through the owned runtime `Variant` bridge, Windows COM `VARIANT` bridge, typed SAFEARRAY translation, and host VM/JIT parity coverage so outward COM arguments now preserve `VT_R4` and `VT_DATE` instead of re-emitting both lanes as `VT_R8`.

## 2026-03-15 - Added outbound float, currency, and decimal COM classifier evidence
- Added direct host VM/JIT evidence that outbound `Double`, `Single`, `Date`, `Currency`, and `Decimal` values classify as `VT_R8`, `VT_R8`, `VT_R8`, `VT_CY`, and `VT_DECIMAL` respectively on the current shared COM carrier, documenting the still-open outward `Single`/`Date` vartype-fidelity gap.

## 2026-03-15 - Added outbound scalar COM VARIANT classifier evidence
- Added direct host VM/JIT evidence that outbound `True`, `BSTR`, `Empty`, `Null`, and `CVErr(...)` arguments marshal to the expected COM `VT_BOOL`, `VT_BSTR`, `VT_EMPTY`, `VT_NULL`, and `VT_ERROR` tags on the controlled classifier lane.

## 2026-03-15 - Added named scalar VT_EMPTY, VT_NULL, and VT_ERROR COM result evidence
- Added controlled fixture members, compiler token coverage, HAL metadata expectations, and host VM/JIT evidence for named scalar VT_EMPTY, VT_NULL, and VT_ERROR result lanes.

## 2026-03-15 - Added named scalar VT_BOOL and VT_BSTR COM result evidence
- Added controlled fixture members, compiler token coverage, HAL metadata expectations, and host VM/JIT evidence for named scalar VT_BOOL and VT_BSTR result lanes.

## 2026-03-15 - Bounded VT_UI4/VT_UINT COM overflow transport on the current i32 lane
- Fixed the Windows COM bridge so scalar VT_UI4 / VT_UINT values and one-dimensional typed VT_ARRAY | VT_UI4 / VT_ARRAY | VT_UINT elements use checked narrowing instead of lossy s i32 casts.
- Added controlled fixture members, compiler token coverage, HAL metadata expectations, and host VM/JIT evidence for deterministic overflow diagnostics on both scalar and typed-array lanes.

## 2026-03-16 - Locked additional COM invoke HRESULT classification evidence in `IP-03A`

- Continued the bounded late-bound COM invoke-fidelity work instead of overclaiming broader Office automation error parity.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), extended the shared HRESULT label mapping so `DISP_E_UNKNOWNNAME` and `DISP_E_BADPARAMCOUNT` no longer collapse into the generic native-failure bucket.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), added VM/JIT host evidence proving deterministic adapter-fault and raw-detail surfaces for:
  - `DISP_E_MEMBERNOTFOUND` on bogus numeric DISPID invocation,
  - `DISP_E_BADPARAMCOUNT` on wrong-arity invocation of a real member.
- In [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs), added adapter-boundary evidence proving raw `IDispatch::GetIDsOfNames` failures now classify stably as `DISP_E_UNKNOWNNAME` instead of falling through to the generic native-failure bucket.
- `IP-03` remains `in-progress`: broader external `ArgErr` / `ExcepInfo` / `VarResult`, non-`IDispatch` interface transport, and multi-dimensional SAFEARRAY parity are still open.

## 2026-03-15 - Added controlled `VT_DECIMAL` COM result transport evidence in `IP-03A`

- Continued the active late-bound COM value-transport pass by introducing an exact `Decimal96` semantic carrier instead of degrading automation `Decimal` payloads into the float or legacy integer lanes.
- In [decimal.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-runtime\src\decimal.rs), [runtime_value.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-runtime\src\runtime_value.rs), and [variant.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-runtime\src\variant.rs), added the exact decimal carrier plus owned runtime `Variant` bridging for `Decimal`.
- In [model.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\model.rs) and [windows_variant.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_variant.rs), extended the semantic `ComValue` bridge and Windows `VARIANT` / typed SAFEARRAY translation so scalar `VT_DECIMAL` and one-dimensional typed `VT_ARRAY | VT_DECIMAL` results now roundtrip on the exact decimal carrier.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), added the controlled `ReturnDecimal` and `ReturnDecimalArray` fixture members.
- In [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), and [resolve.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\resolve.rs), extended controlled metadata and compiler member-token coverage for `57` and `58`.
- In [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs), [main.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-cli\src\main.rs), and [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), updated host-facing formatting, HAL metadata expectations, and VM/JIT parity coverage so scalar `VT_DECIMAL` and typed `VT_ARRAY | VT_DECIMAL` results stay deterministic on the exact decimal carrier.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) and [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) so the supported COM value-transport subset now explicitly includes automation `Decimal` while the broader parity program remains open.
## 2026-03-15 - Added controlled `VT_CY` / `Currency` COM result transport evidence in `IP-03A`

- Continued the active late-bound COM value-transport pass by introducing an exact scaled-`i64` semantic currency carrier instead of widening automation `Currency` payloads into the existing float lane.
- In [runtime_value.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-runtime\src\runtime_value.rs), added `CurrencyValue` and threaded `RuntimeValue::Currency(...)` through the semantic runtime carrier with exact scaled-value formatting and legacy-slot rejection.
- In [variant.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-runtime\src\variant.rs) and [model.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\model.rs), extended the owned runtime `Variant` and semantic `ComValue` bridges so `Currency` values now survive across the shared runtime/COM boundary without degrading to the float or legacy integer lanes.
- In [windows_variant.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_variant.rs), added scalar and one-dimensional typed SAFEARRAY `VT_CY` translation in both directions together with focused Windows bridge tests.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), added the controlled `ReturnCurrency` and `ReturnCurrencyArray` fixture members.
- In [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), and [resolve.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\resolve.rs), extended controlled metadata and compiler member-token coverage for `55` and `56`.
- In [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs), [main.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-cli\src\main.rs), and [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), updated host-facing formatting, HAL metadata expectations, and VM/JIT parity coverage so scalar `VT_CY` and typed `VT_ARRAY | VT_CY` results stay deterministic on the exact currency carrier.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) and [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) so the supported COM value-transport subset now explicitly includes automation `Currency` alongside the existing float/date subset while the broader parity program remains open.
## 2026-03-15 - Added controlled `VT_DATE` COM result transport evidence in `IP-03A`

- Continued the active late-bound COM value-transport pass by routing automation `Date` payloads across the existing semantic `f64` carrier instead of introducing a separate runtime date lane before the broader value-model work closes.
- In [variant.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-runtime\src\variant.rs), added `VarType::Date` plus an owned runtime `Variant` bridge from `Date` into `RuntimeValue::F64(...)` with focused coverage.
- In [windows_variant.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_variant.rs), added scalar and one-dimensional typed SAFEARRAY `VT_DATE` result translation together with focused Windows bridge tests.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), added the controlled `ReturnDate` and `ReturnDateArray` fixture members.
- In [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), and [resolve.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\resolve.rs), extended controlled metadata and compiler member-token coverage for `53` and `54`.
- In [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs) and [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), updated HAL metadata expectations and VM/JIT parity coverage so scalar `VT_DATE` and typed `VT_ARRAY | VT_DATE` results stay deterministic on the semantic `f64` carrier.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) and [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) so the supported COM value-transport subset now explicitly includes automation `Date` alongside the current float subset while the broader parity program remains open.
## 2026-03-15 - Added controlled `VT_R4` / `Single` COM result transport evidence in `IP-03A`

- Continued the active late-bound COM value-transport pass by widening COM `Single` payloads into the existing semantic `f64` carrier instead of inventing a second runtime float lane.
- In [variant.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-runtime\src\variant.rs), extended the owned runtime `Variant` bridge so `VarType::Single` now roundtrips into `RuntimeValue::F64(...)` and added focused bridge coverage.
- In [windows_variant.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_variant.rs), added scalar and one-dimensional typed SAFEARRAY `VT_R4` result translation together with focused Windows bridge tests.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), added the controlled `ReturnSingle` and `ReturnSingleArray` fixture members.
- In [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), and [resolve.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\resolve.rs), extended controlled metadata and compiler member-token coverage for `51` and `52`.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), added VM/JIT parity coverage so scalar `VT_R4` and typed `VT_ARRAY | VT_R4` results widen deterministically into the semantic `f64` carrier.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) and [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md) so the supported COM value-transport subset now explicitly includes `Single` alongside `Double` while the broader parity program remains open.
## 2026-03-15 - Added first-class `VT_R8` / `Double` COM result transport evidence in `IP-03A`

- Continued the active late-bound COM value-transport work on the next honest scalar category after the checked integer lanes by introducing a first-class bit-stable `f64` carrier instead of narrowing `Double` into the legacy integer lane.
- In [runtime_value.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-runtime\src\runtime_value.rs), added `F64Value` and threaded `RuntimeValue::F64(...)` through the semantic runtime carrier with exact-bit equality.
- In [variant.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-runtime\src\variant.rs) and [model.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\model.rs), extended the owned runtime `Variant` and semantic `ComValue` bridges so `Double` values can survive across the shared runtime/COM boundary without degrading to the legacy `i32` lane.
- In [windows_variant.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_variant.rs), added scalar and one-dimensional typed SAFEARRAY `VT_R8` translation in both directions together with focused Windows bridge tests.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), added the controlled `ReturnDouble` and `ReturnDoubleArray` fixture members.
- In [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), and [resolve.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\resolve.rs), added compiler token coverage for `49` and `50`.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), extended VM/JIT parity coverage so scalar `VT_R8` and typed `VT_ARRAY | VT_R8` results roundtrip into the new semantic `f64` carrier.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) to record that `Double` is now part of the supported one-dimensional COM value-transport subset while the broader `VARIANT`/SAFEARRAY/object parity program remains open.
- 2026-03-15: extended controlled COM scalar result evidence to `VT_I8`, `VT_UI8`, `VT_ARRAY | VT_I8`, and `VT_ARRAY | VT_UI8` via new ReturnHyper / ReturnUnsignedHyper fixture members, with compiler member-token coverage (45-48), checked narrowing into the current `RuntimeValue::I32` carrier lane, and host VM/JIT parity assertions for both scalar and typed SAFEARRAY results.
- 2026-03-15: extended controlled COM scalar result evidence to VT_INT, VT_UINT, VT_ARRAY | VT_INT, and VT_ARRAY | VT_UINT via new platform-int fixture members, with compiler member-token coverage (41-44) and host VM/JIT parity on the current RuntimeValue::I32 carrier lane.
- 2026-03-15: extended controlled COM scalar result evidence to VT_I1 and VT_ARRAY | VT_I1 via ReturnSignedByte / ReturnSignedByteArray, with compiler member-token coverage (39/40) and host VM/JIT parity on the current RuntimeValue::I32 carrier lane.
- 2026-03-15: extended controlled COM scalar result evidence to VT_UI1 and VT_ARRAY | VT_UI1 via ReturnByte / ReturnByteArray, with compiler member-token coverage (37/38) and host VM/JIT parity on the current RuntimeValue::I32 carrier lane.
- 2026-03-15: extended controlled COM scalar result evidence to VT_I4 and VT_UI4 via ReturnLong / ReturnUnsignedLong, with compiler member-token coverage (35/36) and host VM/JIT parity on the current RuntimeValue::I32 carrier lane.
- 2026-03-15: extended controlled COM typed SAFEARRAY result evidence to VT_ARRAY | VT_I4 and VT_ARRAY | VT_UI4 via new ReturnLongArray / ReturnUnsignedLongArray fixture members, compiler member-token coverage (33/34), and host VM/JIT parity assertions on the current RuntimeValue::I32 carrier lane.
## 2026-03-14 - Late-bound COM object-valued SAFEARRAY variant subset

- Extended the Windows result bridge in `oxvba-com` so one-dimensional `VT_ARRAY | VT_VARIANT` payloads can recursively rebind nested `VT_DISPATCH` and `VT_UNKNOWN` elements that expose `IDispatch` into runtime-owned `ObjectHandle` values.
- Added controlled `OxVba.TestDispatch` classifier and return-array members proving nested object-valued SAFEARRAY elements now survive both argument and result paths.
- Added compiler token-table coverage, deterministic typelib metadata entries, and VM/JIT end-to-end host coverage for the new controlled members.
- Remaining gap: broader non-`IDispatch` interface arrays and multi-dimensional SAFEARRAY parity remain open.
- Verification:
  - `cargo fmt --all`
  - `cargo test -p oxvba-compiler -p oxvba-host -p oxvba-vm -p oxvba-com -p oxvba-hal --quiet`
  - `cargo clippy -p oxvba-com -p oxvba-compiler -p oxvba-host -p oxvba-vm -p oxvba-hal --all-targets -- -D warnings`
## 2026-03-14 - ParamArray lowering now preserves semantic arrays

- Replaced the compiler''s `ParamArray` count-tag lowering with semantic array-literal construction, so trailing `ParamArray` packs now enter runtime and COM lanes as `RuntimeValue::ArrayIntent(SafeArray::from_values(...))`.
- Added compiler evidence for `IntrinsicArrayLiteral` emission and VM/JIT/host evidence proving `ParamArray` forwarding into `DispatchInvoke(..., "ClassifyVariantArg", items)` now reaches the controlled COM fixture as `VT_ARRAY | VT_VARIANT`.
- Remaining gap: broader multi-dimensional and object-valued SAFEARRAY parity is still open.
- Verification:
  - `cargo fmt --all`
  - `cargo test -p oxvba-compiler -p oxvba-host -p oxvba-vm -p oxvba-com -p oxvba-hal --quiet`
  - `cargo clippy -p oxvba-com -p oxvba-compiler -p oxvba-host -p oxvba-vm -p oxvba-hal --all-targets -- -D warnings`
## 2026-03-14 - Natural named late-bound default-member dispatch

- Removed the stale compiler-side rejection for named arguments on natural late-bound default-member syntax and now lower that form onto the same `DispatchInvoke(..., 0, name := ...)` metadata-backed path already used by explicit dispatch.
- Added compiler and VM/JIT/host evidence proving `value = obj(value := 19)` now executes for `OxVba.TestDispatch` when authoritative default-member identity is available on the COM binding.
- Remaining gap: non-metadata-backed bindings still cannot recover authoritative default-member identity for this syntax, so full Office-style default-member parity remains open.
- Verification:
  - `cargo fmt --all`
  - `cargo test -p oxvba-compiler -p oxvba-host -p oxvba-vm -p oxvba-com -p oxvba-hal --quiet`
  - `cargo clippy -p oxvba-com -p oxvba-compiler -p oxvba-host -p oxvba-vm -p oxvba-hal --all-targets -- -D warnings`
## 2026-03-14 - Late-bound COM semantic array argument marshalling

- Replaced the compiler's legacy `Array(...)` length-tag lowering with a real array-literal instruction that materializes `RuntimeValue::ArrayIntent(SafeArray::from_values(...))` at runtime.
- Added end-to-end VM/JIT/host evidence proving `DispatchInvoke(obj, "ClassifyVariantArg", Array(1, 2, 3))` now reaches the controlled COM fixture as `VT_ARRAY | VT_VARIANT`.
- This closes the specific outbound VBA `Array(...)` argument gap on the current one-dimensional semantic array subset; broader object-valued and multi-dimensional SAFEARRAY parity remains open.
- Verification:
  - `cargo fmt --all`
  - `cargo test -p oxvba-compiler -p oxvba-vm -p oxvba-host -p oxvba-com -p oxvba-hal --quiet`
  - `cargo clippy -p oxvba-com -p oxvba-compiler -p oxvba-vm -p oxvba-host -p oxvba-hal --all-targets -- -D warnings`

## 2026-03-14 - Late-bound COM object argument evidence

- Added a controlled raw-variant classifier member to `OxVba.TestDispatch` and end-to-end host coverage proving outbound object-valued COM arguments marshal as `VT_DISPATCH`.
- Verification:
  - `cargo fmt --all`
  - `cargo test -p oxvba-compiler -p oxvba-com -p oxvba-host -p oxvba-hal --quiet`
  - `cargo clippy -p oxvba-com -p oxvba-compiler -p oxvba-host -p oxvba-hal --all-targets -- -D warnings`
## 2026-03-14 - Late-bound COM object result rebinding evidence

- Added controlled `OxVba.TestDispatch` members returning `VT_DISPATCH` and `VT_UNKNOWN` results that expose `IDispatch`.
- Added compiler token-table coverage plus VM/JIT end-to-end host coverage showing those results rebind into invokable runtime object handles.
- Verification:
  - `cargo fmt --all`
  - `cargo test -p oxvba-compiler -p oxvba-com -p oxvba-host -p oxvba-hal --quiet`
  - `cargo clippy -p oxvba-com -p oxvba-compiler -p oxvba-host -p oxvba-hal --all-targets -- -D warnings`
## 2026-03-14 - Late-bound COM typed SAFEARRAY result subset

- Extended the Windows `VARIANT` bridge in `oxvba-com` so one-dimensional typed SAFEARRAY results with `VT_I2`, `VT_BOOL`, and `VT_BSTR` elements now map into `RuntimeValue::ArrayIntent`.
- Added controlled `OxVba.TestDispatch` methods returning typed SAFEARRAY results and wired the deterministic compiler token tables/catalog so those fixture members lower cleanly.
- Added unit coverage in `crates/oxvba-com/src/windows_variant.rs`, a compiler token-mapping regression test, and VM/JIT end-to-end host coverage in `crates/oxvba-host/tests/com_client_end_to_end.rs`.
- Verification:
  - `cargo fmt --all`
  - `cargo test -p oxvba-compiler -p oxvba-com -p oxvba-host --quiet`
  - `cargo clippy -p oxvba-com -p oxvba-compiler -p oxvba-host --all-targets -- -D warnings`
## 2026-03-14 - Added dependency-ordered COM/property/hosting execution workset

- Added [WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md) as the large-program execution map after `IP-04` closure.
- It orders the remaining feature areas by dependency:
  - `IP-03`
  - `IP-02`
  - `IP-05`
  - `IP-06`
  - `IP-08`
  - with `IP-07` called out where it is a blocking ingress/event dependency.
- It records:
  - the dependency spine,
  - phase-by-phase execution order,
  - work package split,
  - planning notes and acceptance expectations,
  - and the rule that downstream work must not reopen the already-closed `IP-04` boundary work.
## 2026-03-14 - Added dependency-ordered COM/property/hosting execution workset

- Added [WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-14_COM_PARITY_PROPERTY_SERVER_HOSTING_EXECUTION_SEQUENCE.md) as the large-program execution map after `IP-04` closure.
- It orders the remaining feature areas by dependency:
  - `IP-03`
  - `IP-02`
  - `IP-05`
  - `IP-06`
  - `IP-08`
  - with `IP-07` called out where it is a blocking ingress/event dependency.
- It records:
  - the dependency spine,
  - phase-by-phase execution order,
  - work package split,
  - planning notes and acceptance expectations,
  - and the rule that downstream work must not reopen the already-closed `IP-04` boundary work.
## 2026-03-14 - Closed IP-04 oxvba-com / HAL extraction

- Completed the final `IP-04` closure slice in:
  - [windows_bridge.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_bridge.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
  - [WORKSET_2026-03-14_IP04_OXVBA_COM_HAL_EXTRACTION_CLOSURE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-14_IP04_OXVBA_COM_HAL_EXTRACTION_CLOSURE.md)
  - [ARCHITECTURE.md](C:\Work\DnaCalc\OxVba\docs\ARCHITECTURE.md)
- `oxvba-com` now exposes `WindowsComBridge` as the live Windows COM client facade.
- `standard.rs` now delegates create-object activation, invoke execution, object description/release, event subscription/callback interrogation, and typelib services through that facade.
- Native subscription transport teardown for object release now also executes inside `oxvba-com`, removing the last substantive COM lifecycle seam from HAL.
- `BLK-COM-BOUNDARY-001` is resolved and `IP-04` is now closed.
- Verification:
  - `cargo fmt --all`
  - `cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings`
  - `cargo test -p oxvba-com -p oxvba-hal -p oxvba-host --quiet`
  - `./scripts/check-governance.ps1`
  - `./scripts/meta-check.ps1 -Fast -NoArtifacts`
## 2026-03-14 - Moved bound COM dispatch service entry into oxvba-com

- Continued the `IP-04` extraction slice in:
  - [windows_invoke.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_invoke.rs)
  - [windows_runtime_state.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_runtime_state.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- `oxvba-com` now owns a higher-level bound-dispatch execution service that combines:
  - binding lookup from shared COM state,
  - cached DISPID reuse/update,
  - member-spec/direct-DISPID/unbound dispatch routing over shared-state invoke helpers,
  - projection callback queueing for the bound native lane.
- `standard.rs::dispatch_invoke_runtime_value_v2(...)` now delegates the native bound COM object path through that `oxvba-com` service instead of coordinating the bound dispatch pipeline locally.
- Net effect:
  - HAL no longer owns the bound COM dispatch service entry,
  - the remaining `IP-04` wall is now the residual legacy projection invoke lane, teardown-side native transport release, dead transitional helper cleanup, and the final public COM/HAL seam collapse and ownership audit.
- Verification:
  - `cargo fmt --all`
  - `cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings`
  - `cargo test -p oxvba-com -p oxvba-hal --quiet`
## 2026-03-14 - Moved COM create-object binding insertion into oxvba-com

- Continued the `IP-04` extraction slice in:
  - [windows_client.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_client.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- `oxvba-com` now owns a higher-level activation service that combines:
  - runtime dispatch activation,
  - binding assembly,
  - caller-supplied binding configuration,
  - shared-state binding insertion.
- `standard.rs::create_object(...)` now delegates native COM object creation through that `oxvba-com` service instead of assembling and inserting the binding locally.
- Net effect:
  - HAL no longer owns create-object binding insertion,
  - the remaining `IP-04` wall is now the bound dispatch service entry, projection callback queueing, the residual legacy projection invoke lane, and the final public COM/HAL seam collapse.
- Verification:
  - `cargo fmt --all`
  - `cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings`
  - `cargo test -p oxvba-com -p oxvba-hal --quiet`
## 2026-03-14 - Moved COM event transport orchestration into oxvba-com

- Continued the `IP-04` extraction slice in:
  - [windows_runtime_state.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_runtime_state.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- `oxvba-com` now owns shared-state helpers for:
  - callback-pump token advancement,
  - event subscription allocation/transport resolution/insertion,
  - event unsubscription transport release and callback pruning.
- `standard.rs` now delegates COM event subscription/unsubscription orchestration and callback-pump progression to `oxvba-com` shared services instead of owning that subscription-state workflow locally.
- Net effect:
  - HAL no longer owns COM event transport orchestration,
  - the remaining `IP-04` wall is now activation/create-object entry, dispatch service entry, projection callback queueing, and the final public COM/HAL seam collapse.
- Verification:
  - `cargo fmt --all`
  - `cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings`
  - `cargo test -p oxvba-com -p oxvba-hal --quiet`
## 2026-03-14 - Moved COM runtime invoke closure wiring into oxvba-com

- Continued the `IP-04` extraction slice in:
  - [windows_invoke.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_invoke.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- `oxvba-com` now owns the shared-state runtime invoke wrappers for:
  - member-spec runtime-value invoke,
  - direct-DISPID runtime-value invoke,
  - generic runtime-value `IDispatch::Invoke` execution with shared object-result rebinding.
- `standard.rs` now delegates those runtime invoke/result closure paths to `oxvba-com` instead of wiring object resolution, `IDispatch` requery, `AddRef`, and invoke-result rebinding locally.
- Net effect:
  - HAL no longer owns the direct runtime invoke closure wiring around controlled Windows COM dispatch,
  - the remaining `IP-04` wall is now activation/event transport authority plus the final public COM/HAL boundary contraction and ownership audit.
- Verification:
  - `cargo fmt --all`
  - `cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings`
  - `cargo test -p oxvba-com -p oxvba-hal --quiet`
## 2026-03-14 - Moved shared COM object/result state wrappers into oxvba-com

- Continued the `IP-04` extraction slice in:
  - [windows_runtime_state.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_runtime_state.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- `oxvba-com` now owns shared-state helpers for:
  - inserted binding registration,
  - bound-dispatch resolution from shared host state,
  - invoke-result object rebinding into shared host state,
  - object-release teardown from shared host state.
- `standard.rs` now delegates those state-locking lifecycle paths into `oxvba-com` instead of managing the COM state lock and mutation flow itself.
- Net effect:
  - HAL no longer owns the COM state-locking wrappers around object/result lifecycle,
  - the remaining `IP-04` wall is narrowed to the last direct Windows `IDispatch` invoke/result lifecycle seam and the final public boundary contraction.
- Verification:
  - `cargo clippy -p oxvba-com -p oxvba-hal -p oxvba-vm -p oxvba-host --all-targets -- -D warnings`
  - `cargo test -p oxvba-com -p oxvba-hal -p oxvba-vm -p oxvba-host --quiet`
  - `./scripts/check-governance.ps1`
  - `./scripts/meta-check.ps1 -Fast -NoArtifacts`
## 2026-03-14 - Moved activation-time COM binding creation into oxvba-com

- Continued the `IP-04` extraction slice in:
  - [windows_client.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_client.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- `oxvba-com` now owns activation-time COM binding creation through `activate_runtime_binding(...)`, combining:
  - runtime dispatch activation,
  - deterministic typelib-backed binding assembly,
  - the existing registered-test-dispatch selection policy.
- `standard.rs::create_object(...)` now delegates binding creation to `oxvba-com` and only retains:
  - apartment readiness,
  - optional registered-event override application,
  - insertion of the resulting binding into shared host state.
- Net effect:
  - HAL no longer owns activation-time COM binding assembly,
  - the remaining `IP-04` wall is narrowed to the shared-state object/result rebinding wrappers and the final invoke-result lifecycle authority.
- Verification:
  - `cargo clippy -p oxvba-com -p oxvba-hal -p oxvba-vm -p oxvba-host --all-targets -- -D warnings`
  - `cargo test -p oxvba-com -p oxvba-hal -p oxvba-vm -p oxvba-host --quiet`
  - `./scripts/check-governance.ps1`
  - `./scripts/meta-check.ps1 -Fast -NoArtifacts`
## 2026-03-14 - Moved bound runtime invoke orchestration into oxvba-com

- Continued the IP-04 extraction slice in:
  - [windows_invoke.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_invoke.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
- oxvba-com now owns the high-level bound runtime invoke orchestration helper that chooses between:
  - legacy vtable fast-path invocation,
  - named default-member resolution,
  - metadata-backed member-spec dispatch,
  - direct-DISPID dispatch,
  - bound-dispatch fallback.
- standard.rs now delegates that routing choice to oxvba-com and only supplies the remaining Windows-native execution closures.
- Net effect:
  - HAL no longer owns the high-level bound runtime invoke policy,
  - the remaining IP-04 wall is narrowed further to raw Windows activation plus final invoke-result/object-lifecycle ownership.
- Verification:
  - cargo test -p oxvba-com -p oxvba-hal -p oxvba-vm -p oxvba-host --quiet
  - cargo clippy -p oxvba-com -p oxvba-hal -p oxvba-vm -p oxvba-host --all-targets -- -D warnings
  - ./scripts/check-governance.ps1
  - ./scripts/meta-check.ps1 -Fast -NoArtifacts
## 2026-03-14 - Contracted the event-side ComHal boundary to typed COM handles

- Completed the first coordinated public `ComHal` migration slice across:
  - [traits.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\traits.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [null.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\null.rs)
  - [wasm.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\wasm.rs)
  - [interpreter.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-vm\src\interpreter.rs)
  - [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs)
- `ComHal::{subscribe_event,unsubscribe_event,event_callback_subscription,event_callback_arity,event_callback_arg,release_event_callback}` now use typed COM/object handles on the public seam.
- VM COM intrinsics and host COM event helpers now decode/encode typed tokens at the edge instead of routing event-side identity through `RuntimeValue` wrappers.
- Null/wasm adapters now compile against the contracted trait while preserving explicit legacy-only test helpers outside the public trait.
- Net effect:
  - the event-side public COM contract is no longer transitional,
  - the remaining `IP-04` wall is narrowed to the live Windows invoke-result lifecycle seam and the last HAL-to-`oxvba-com` execution delegation work.
- Verification:
  - `cargo test -p oxvba-hal -p oxvba-vm -p oxvba-host --quiet`
  - `cargo clippy -p oxvba-hal -p oxvba-vm -p oxvba-host --all-targets -- -D warnings`
  - `./scripts/check-governance.ps1`
  - `./scripts/meta-check.ps1 -Fast -NoArtifacts`
## 2026-03-14 - Reached the public ComHal contraction wall

- After the resolved-member DISPID cache extraction, the next remaining COM/HAL work was tested as a typed-token `ComHal` contraction slice.
- That attempt showed the remaining boundary is no longer a local helper move:
  - it touches the public `ComHal` trait,
  - VM COM host intrinsics,
  - host COM event helper surfaces,
  - null/wasm adapter stubs,
  - and the final result-lifecycle glue still routed through HAL.
- I reverted the partial uncommitted contract-edit attempt rather than leaving the repo in a half-migrated state.
- Current conclusion:
  - the next COM/HAL step must be executed as one coordinated public contract migration program,
  - not as another incremental helper extraction.
## 2026-03-14 - Moved resolved-member DISPID cache lookup/update into oxvba-com

- Continued the COM extraction/contraction slice in:
  - [windows_runtime_state.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_runtime_state.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
- `oxvba-com` now owns the reusable helper that:
  - resolves member metadata fallback for a bound COM object,
  - performs raw `GetIDsOfNames` member lookup when needed,
  - updates the bound-object DISPID cache in shared COM runtime state.
- `oxvba-hal::standard` now delegates that cache/lookup behavior and keeps only binding fallback selection and error mapping around it.
- Net effect:
  - resolved-member cache authority is no longer HAL-owned,
  - the remaining COM extraction wall is now centered on final invoke-result lifecycle glue and public contract contraction.
- Verification:
  - cargo fmt --all
  - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings
  - cargo test -p oxvba-com -p oxvba-hal --quiet
## 2026-03-14 - Moved member-spec/direct-DISPID runtime invoke helpers into oxvba-com

- Continued the COM extraction/contraction slice in:
  - [windows_invoke.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_invoke.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
- `oxvba-com` now owns the reusable runtime-value invoke helpers for:
  - member-metadata-backed COM dispatch,
  - direct-DISPID COM dispatch,
  - property-get / method / property-put / property-putref routing inside those helpers.
- `oxvba-hal::standard` now keeps lookup/cache/state/error-mapping responsibilities around those calls instead of owning the invoke execution strategy itself.
- Net effect:
  - the remaining live HAL-owned COM seam is narrower again,
  - the extraction wall is now centered on resolved-member DISPID/cache authority, final invoke-result lifecycle glue, and public contract contraction.
- Verification:
  - cargo fmt --all
  - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings
  - cargo test -p oxvba-com -p oxvba-hal --quiet
## 2026-03-14 - Moved callback payload polling and metadata access into oxvba-com

- Continued the COM extraction/contraction slice in:
  - [windows_runtime_state.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_runtime_state.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
- `oxvba-com` now owns:
  - polling the queued callback payload,
  - resolving callback -> subscription identity,
  - reporting callback arity and argument payloads,
  - callback release bookkeeping.
- `oxvba-hal::standard` now keeps only policy/error mapping around those callback queries.
- Net effect:
  - callback interrogation is no longer a HAL-owned COM state concern,
  - the remaining extraction wall is narrower again and centered on the final invoke/result-lifecycle/contract seam.
- Verification:
  - cargo fmt --all
  - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings
  - cargo test -p oxvba-com -p oxvba-hal --quiet
## 2026-03-14 - Moved COM runtime-value invoke execution helper into oxvba-com

- Extended `crates/oxvba-com/src/windows_invoke.rs` with a higher-level runtime-value `IDispatch::Invoke` helper that:
  - executes the Windows invoke call,
  - classifies the semantic result,
  - delegates dispatch-backed result rebinding through caller-provided closures.
- Rebound `oxvba-hal::standard` so its `native_dispatch_invoke_runtime_value_args(...)` helper is now a thin delegation wrapper over that shared `oxvba-com` surface.
- This removes the generic raw execute-and-classify path from HAL-owned COM authority.
- The remaining extraction wall is now narrower again:
  - resolved-member DISPID lookup/cache update and final object rebinding/lifecycle glue still live in HAL,
  - public HAL COM contract contraction/rebinding still pending.
## 2026-03-14 - Moved COM invoke-policy planning into oxvba-com

- Added `crates/oxvba-com/src/invoke_policy.rs` as the shared policy surface for:
  - named-argument ordering validation,
  - metadata-backed argument canonicalization,
  - bound default-member/direct-DISPID/member-spec routing,
  - unbound fallback property-get planning.
- Rebound `oxvba-hal::standard` so the Windows native dispatch path now asks `oxvba-com` to plan the route before executing raw `IDispatch` calls.
- This removes the high-level default-member/direct-DISPID/member-spec routing rules from HAL-owned COM authority.
- The remaining extraction wall is now narrower:
  - live `IDispatch` execution / DISPID resolution / result rebinding still live in HAL,
  - public HAL COM contract contraction/rebinding still pending.
# Implementation Log

## 2026-03-14 - Added explicit IP-04 closure workset

- Added:
  - [WORKSET_2026-03-14_IP04_OXVBA_COM_HAL_EXTRACTION_CLOSURE.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-14_IP04_OXVBA_COM_HAL_EXTRACTION_CLOSURE.md)
- Purpose:
  - turn the approved 1-24 COM/HAL extraction plan into the authoritative end-to-end `IP-04` closure workset,
  - make explicit what is and is not required to close `IP-04`,
  - define the final verification and ownership-audit gates needed before `IP-04` can be described as complete.
- Cross-linked the new closure workset from:
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)


## 2026-03-13 - Moved COM event transport-choice resolution into oxvba-com

- Continued the COM extraction/contraction slice in:
  - [windows_runtime_state.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_runtime_state.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
- `oxvba-com` now owns the binding/spec-to-transport decision for COM event subscriptions, including the projection-vs-native connection-point choice.
- `oxvba-hal::standard` now keeps only apartment/policy/error-mapping responsibilities around subscription transport setup.
- Net effect:
  - the remaining COM extraction wall is now centered on invoke-policy/default-member/direct-DISPID sequencing and final HAL contract contraction,
  - event transport-choice authority is no longer primarily HAL-owned.
- Verification:
  - cargo fmt --all
  - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings
  - cargo test -p oxvba-com -p oxvba-hal --quiet

## 2026-03-13 - Moved COM binding-table mutation into oxvba-com

- Continued the COM extraction/contraction slice in:
  - [windows_runtime_state.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_runtime_state.rs)
  - [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\lib.rs)
  - [standard.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-hal\src\adapters\standard.rs)
  - [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md)
  - [IN_PROGRESS_FEATURE_WORKLIST.md](C:\Work\DnaCalc\OxVba\docs\IN_PROGRESS_FEATURE_WORKLIST.md)
  - [WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-09_OXVBA_COM_REPURPOSE_AND_HAL_COM_EXTRACTION.md)
- `oxvba-com` now owns:
  - activation-time binding insertion for native COM objects,
  - per-object DISPID cache mutation,
  - the previously extracted bound-dispatch/object-release/subscription teardown bookkeeping.
- `oxvba-hal::standard` now delegates binding-table mutation and retains only activation policy, invoke-policy sequencing, event transport choice, and contract-level error mapping.
- Net effect:
  - the remaining COM extraction wall is no longer basic object/binding bookkeeping,
  - it is now the higher-level invoke-policy/contract authority still centered in HAL.
- Verification:
  - cargo fmt --all
  - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings
  - cargo test -p oxvba-com -p oxvba-hal --quiet

## 2026-03-13 - Moved bound-dispatch and subscription teardown ownership into oxvba-com
## 2026-03-13 - Moved bound-dispatch and subscription teardown ownership into oxvba-com

























## 2026-03-15 - Added deterministic unsupported multidimensional SAFEARRAY diagnostics in `IP-03A`

- Continued the late-bound COM transport substrate with an explicit unsupported-path coverage slice rather than pretending broader SAFEARRAY rank support already exists.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), added the controlled `ReturnSmallIntMatrix` fixture member that returns a rank-2 `VT_ARRAY | VT_I2` payload.
- In [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), added compiler token coverage proving `DispatchInvoke(..., "ReturnSmallIntMatrix")` lowers to member token `30`.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), added end-to-end VM/JIT coverage proving rank-2 SAFEARRAY results now surface a stable runtime adapter fault containing `unsupported SAFEARRAY rank 2` instead of crashing or drifting semantically.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) so the remaining multidimensional SAFEARRAY gap is documented as unsupported-but-bounded rather than unproven.
## 2026-03-15 - Locked direct COM failure-detail parity for the bounded `IP-03A` subset

- Continued the late-bound COM fidelity work without overclaiming broader Office-style automation parity.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), added direct error-path VM/JIT parity coverage proving:
  - `DispatchInvoke(obj, "SumPair", "bad", 42)` preserves a real `DISP_E_TYPEMISMATCH` adapter fault with `arg_err=1`,
  - `DispatchInvoke(obj, "RaiseException")` preserves bounded `EXCEPINFO` source, description, and scode details without synthesizing a fake `arg_err`.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) so the remaining external `Invoke` fidelity gap is documented relative to this now-proven controlled subset rather than as an unqualified absence of host-facing error detail.
## 2026-03-15 - Locked bounded non-`IDispatch` `VT_UNKNOWN` failure diagnostics in `IP-03A`

- Continued the late-bound COM transport work with another explicit unsupported-but-bounded slice instead of pretending broader non-`IDispatch` interface parity already exists.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), added the controlled `ReturnPlainUnknown` fixture member backed by an `IUnknown`-only COM object that intentionally does not expose `IDispatch`.
- In [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), added compiler token coverage proving `DispatchInvoke(..., "ReturnPlainUnknown")` lowers to member token `31`.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), added VM/JIT parity coverage proving `VT_UNKNOWN` results that do not expose `IDispatch` now fail with a stable native adapter diagnostic containing `IUnknown::QueryInterface(IDispatch) failed with HRESULT 0x80004002`.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) so the remaining non-`IDispatch` interface-pointer gap is documented as broader parity debt beyond this now-proven bounded failure lane.
## 2026-03-15 - Locked bounded non-`IDispatch` typed `VT_ARRAY | VT_UNKNOWN` failure diagnostics in `IP-03A`

- Continued the adjacent late-bound COM transport work with the array-form version of the non-`IDispatch` interface failure lane.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), added the controlled `ReturnPlainUnknownArray` fixture member that returns a one-dimensional typed `VT_ARRAY | VT_UNKNOWN` payload whose element object intentionally does not expose `IDispatch`.
- In [lib.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\lib.rs), added compiler token coverage proving `DispatchInvoke(..., "ReturnPlainUnknownArray")` lowers to member token `32`.
- In [com_client_end_to_end.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\tests\com_client_end_to_end.rs), added VM/JIT parity coverage proving typed unknown-array elements that do not expose `IDispatch` fail with the same bounded `IUnknown::QueryInterface(IDispatch)` adapter diagnostic as the single-value lane.
- Updated [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) so the remaining non-`IDispatch` interface-pointer gap is documented beyond both bounded single-value and typed-array failure surfaces.
## 2026-03-14 - Closed the typed `VT_ARRAY | VT_UNKNOWN` result lane in `IP-03A`

- Continued the adjacent `IP-03A` substrate slice for typed interface SAFEARRAY results where each element arrives as `VT_UNKNOWN` but exposes `IDispatch`.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), added the controlled `ReturnSelfTypedUnknownArray` fixture member and implemented one-dimensional `VT_ARRAY | VT_UNKNOWN` result construction using `SafeArrayPutElement(...)` over the live `IUnknown` pointer.
- In [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), extended the synthetic test typelib metadata so the new member is authoritatively classified as a method rather than falling back to a property-get guess.
- In [windows_variant.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_variant.rs), routed typed `VT_ARRAY | VT_UNKNOWN` elements through the existing `VT_UNKNOWN` `VARIANT` query-and-bind path so the array lane reuses the already-proven single-result object rebinding semantics.
- Added focused compiler and host coverage proving `DispatchInvoke(..., "ReturnSelfTypedUnknownArray")` compiles to token `29` and roundtrips a typed unknown SAFEARRAY result into semantic `RuntimeValue::ArrayIntent` with nested `ObjectHandle` elements when the underlying interface exposes `IDispatch`.
## 2026-03-14 - Closed the typed `VT_ARRAY | VT_DISPATCH` result lane in `IP-03A`

- Continued the active umbrella COM/property/hosting execution sequence with the next `IP-03A` late-bound COM transport slice.
- In [windows_test_dispatch.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_test_dispatch.rs), added the controlled `ReturnSelfTypedDispatchArray` fixture member and implemented typed `VT_ARRAY | VT_DISPATCH` result construction using `SafeArrayPutElement(...)` on the live `IDispatch` pointer rather than the earlier invalid local-pointer indirection.
- In [typelib_catalog.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\typelib_catalog.rs), extended the synthetic test typelib metadata so the new member is authoritatively classified as a method, eliminating the transient property-get misinvoke.
- In [windows_variant.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-com\src\windows_variant.rs), routed typed `VT_ARRAY | VT_DISPATCH` elements back through the existing `VARIANT`-based object-result binding path so interface-array ownership now matches the already-proven single-value and nested-variant object lanes.
- Added focused compiler and host coverage proving `DispatchInvoke(..., "ReturnSelfTypedDispatchArray")` compiles to token `28` and roundtrips a one-dimensional typed dispatch SAFEARRAY result into semantic `RuntimeValue::ArrayIntent` with nested `ObjectHandle` elements.
- Removed `BLK-COM-ARRAY-DISPATCH-001` from [CURRENT_BLOCKERS.md](C:\Work\DnaCalc\OxVba\CURRENT_BLOCKERS.md) now that the host lane is green and the typed-dispatch-array result slice is honest.
## 2026-03-14 - Advanced IP-02 native property semantics and bounded typed-dispatch-array COM blocker

- Continued the umbrella COM/property/hosting execution sequence with the first concrete `IP-02A` native property-semantic slice.
- Compiler/project lowering in [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs) now rewrites natural internal-class property syntax onto the same PMR substrate already used by explicit dynamic dispatch:
  - `widget.Value` now lowers through the native `Property Get` route,
  - `widget.Value = 9` now lowers through the native `Property Let` route,
  - existing internal-class member-call rewriting remains in place.
- Added end-to-end host coverage in [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs) proving natural internal-class `Property Get` / `Property Let` execution through the shared dynamic-object path.
- In parallel, attempted the next late-bound COM substrate slice for typed `VT_ARRAY | VT_DISPATCH` results and deliberately backed it out after the host lane continued to fault with `STATUS_ACCESS_VIOLATION`.
- Recorded the unresolved COM array crash as `BLK-COM-ARRAY-DISPATCH-001` rather than leaving an unverified partial slice in the tree.
## 2026-03-14 - Advanced IP-02 native default-member syntax on the PMR path

- Extended the active `IP-02A` slice so native internal project-class bare default-member syntax now lowers and executes end to end through the same PMR/dynamic-object substrate already used by explicit `DispatchInvoke(...)` and natural `widget.Value` property syntax.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), added bounded lowering for:
  - `beforeValue = widget` -> native default-member `Property Get`,
  - `widget = 9` -> native default-member `Property Let`,
  - while explicitly preserving compiler-generated `Dim ... As New` internal-instance initialization lines instead of misrouting them through default-member `Property Let`.
- Added end-to-end host coverage in [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs) proving natural bare default-member `Get` / `Let` execution for native internal project-class objects with authoritative `VB_UserMemId = 0` metadata.
- `IP-02` remains in progress: `Property Set`, `Set` vs `Let`, indexed/default property behavior beyond this proven subset, and Office-style call-vs-value parity are still open.
## 2026-03-14 - Extended IP-02 native PMR property-set semantics

- Continued the active `IP-02A` semantic-closure slice and proved that native internal project-class `Property Set` semantics now execute end to end on the shared PMR/dynamic-object path.
- Added end-to-end host coverage in [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs) for:
  - natural member `Property Set` syntax: `Set widget.Value = x`,
  - natural bare default-member `Property Set` syntax: `Set widget = x` when authoritative `VB_UserMemId = 0` metadata exists.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), extended `rewrite_internal_class_set_assignment(...)` so bare internal-class `Set lhs = rhs` now routes through the authoritative default-member `Property Set` target instead of falling back to a lossy bare assignment.
- `IP-02` remains in progress: indexed property behavior, broader typed/object `Set` vs `Let` intent parity, and Office-style call-vs-value context behavior are still open.
## 2026-03-14 - Extended IP-02 indexed native property/default-member semantics

- Continued the active IP-02A semantic-closure slice and proved that indexed native internal project-class property/default-member Get / Let syntax now executes end to end on the shared PMR/dynamic-object path when authoritative metadata exists.
- Added end-to-end host coverage in [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs) for:
  - natural indexed member syntax: eforeValue = widget.Value(2), widget.Value(2) = 9, fterValue = widget.Value(2),
  - natural indexed default-member syntax: eforeValue = widget(2), widget(2) = 9, fterValue = widget(2) when authoritative VB_UserMemId = 0 metadata exists.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), extended the internal class PMR rewrite helpers so indexed member/default-member assignment and read-assignment forms lower onto the same authoritative property targets instead of falling back to lossy bare assignment semantics.
- IP-02 remains in progress: broader typed/object Set vs Let intent parity, non-authoritative default-member resolution, and Office-style call-vs-value context behavior are still open.
## 2026-03-14 - Extended IP-02 indexed native property/default-member set semantics

- Continued the active IP-02A semantic-closure slice and proved that indexed native internal project-class Property Set and indexed authoritative default-member Property Set syntax now execute end to end on the shared PMR/dynamic-object path.
- Added end-to-end host coverage in [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs) for:
  - natural indexed member Property Set syntax: Set widget.Value(1) = x,
  - natural indexed default-member Property Set syntax: Set widget(1) = x when authoritative VB_UserMemId = 0 metadata exists.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), extended the internal class PMR set-assignment rewrite so indexed member/default-member Set forms lower onto the same authoritative property targets instead of falling through to unsupported statement handling.
- IP-02 remains in progress: broader typed/object Set vs Let intent parity, non-authoritative default-member resolution, and Office-style call-vs-value context behavior are still open.
## 2026-03-14 - Extended IP-02 statement-context indexed property/default-member get semantics

- Continued the active IP-02A semantic-closure slice and proved that statement-context indexed native internal project-class Property Get calls now execute end to end on the shared PMR/dynamic-object path instead of being silently dropped.
- Added end-to-end host coverage in [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs) for:
  - statement-context indexed member syntax: widget.Value(x),
  - statement-context indexed default-member syntax: widget(x) when authoritative VB_UserMemId = 0 metadata exists.
- Both tests use ByRef argument mutation as the observable effect, proving that the getter actually executes in statement context and that the effect is visible in caller state.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), extended the internal-class dynamic member-dispatch rewrite so authoritative default-member indexed invocations participate in the same lowering path as named-member indexed getter calls.
- IP-02 remains in progress: broader typed/object Set vs Let intent parity, non-authoritative default-member resolution, non-indexed statement-context default-member call behavior, and wider Office-style call-vs-value context parity are still open.
## 2026-03-14 - Extended IP-02 statement-context non-indexed property/default-member get semantics

- Continued the active IP-02A semantic-closure slice and proved that statement-context non-indexed native internal project-class Property Get calls now execute end to end on the shared PMR/dynamic-object path instead of remaining expression-only behavior.
- Added end-to-end host coverage in [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs) for:
  - statement-context member syntax: widget.Value,
  - statement-context default-member syntax: widget when authoritative VB_UserMemId = 0 metadata exists.
- Both tests use a side-effect-free observation property to prove that the statement-form getter executed and mutated hidden class state before the later observable read.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), extended property-read rewriting with a dedicated authoritative default-member statement-form getter lane so bare internal-class object statements lower onto the same PMR property-get target as expression/default-member reads.
- IP-02 remains in progress: broader typed/object Set vs Let intent parity, non-authoritative default-member resolution, and wider Office-style call-vs-value context parity are still open.
## 2026-03-14 - Extended IP-02 Call-form non-indexed property/default-member get semantics

- Continued the active IP-02A call-vs-value closure slice and proved that explicit `Call`-form non-indexed native internal project-class Property Get calls now execute end to end on the shared PMR/dynamic-object path.
- Added end-to-end host coverage in [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs) for:
  - `Call widget.Value`,
  - `Call widget` when authoritative VB_UserMemId = 0 metadata exists.
- Both tests prove the getter executed by mutating hidden class state and then reading the observable state through a separate property.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), extended `rewrite_internal_class_call_statement_without_parens(...)` so dotless authoritative default-member call statements route through the same PMR property-get lowering as dotted member calls.
- IP-02 remains in progress: broader typed/object Set vs Let intent parity, non-authoritative default-member resolution, and wider Office-style call-vs-value context parity are still open.
## 2026-03-14 - Extended IP-02 no-parentheses argument property/default-member get semantics

- Continued the active IP-02A call-vs-value closure slice and proved that no-parentheses argument native internal project-class Property Get calls now execute end to end on the shared PMR/dynamic-object path.
- Added end-to-end host coverage in [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs) for:
  - `widget.Value x`,
  - `widget x` when authoritative VB_UserMemId = 0 metadata exists.
- Both tests use ByRef argument mutation as the observable effect, proving that the getter executed through the statement-form/no-parentheses call path rather than falling back to expression-only rewriting.
- In [project.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\project.rs), added a dedicated internal-class no-parentheses statement-invoke rewrite and taught the property-expression rewrite to leave that exact statement shape untouched until the targeted invoke rewrite can lower it correctly.
- IP-02 remains in progress: broader typed/object Set vs Let intent parity, non-authoritative default-member resolution, and wider Office-style call-vs-value context parity are still open.
## 2026-03-14 - Extended IP-02 bounded Set/Let assignment intent semantics

- Continued the active IP-02A semantic-closure slice and preserved explicit Set / Let keywords through resolver and typecheck for plain assignment and assignment-from-call statements instead of erasing that intent before semantic validation.
- In [resolve.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\resolve.rs), added AssignmentIntent and threaded it through BoundStmt::Assign / BoundStmt::AssignFromCall so the compiler can distinguish implicit assignment from explicit Let / Set intent.
- In [typecheck.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-compiler\src\typecheck.rs), added bounded intent validation so:
  - Let rejects object-only targets,
  - Set rejects non-object/non-Variant targets,
  - Set rejects scalar sources while still allowing the current object-carrying Variant source lane used by CreateObject(...).
- Added end-to-end host coverage in [engine.rs](C:\Work\DnaCalc\OxVba\crates\oxvba-host\src\engine.rs) proving:
  - Let x = 5 plus Set obj = CreateObject(4) executes successfully,
  - Set x = 7 fails deterministically with the expected type error.
- IP-02 remains in progress: broader typed/object Set vs Let parity, non-authoritative default-member resolution, and wider Office-style call-vs-value context parity are still open.

## 2026-03-18 - Add non-authoritative object-target read-assignment diagnostic evidence

- Continued the active `IP-02A` checklist run without widening semantics and added direct bounded evidence for non-authoritative object-valued default-member source-resolution failures on typed `Object` targets.
- Added compiler and VM/JIT host evidence proving that ambiguous and `no viable candidate` non-authoritative default-member resolution diagnostics now directly cover:
  - explicit `Let childOut = widget`,
  - implicit `childOut = widget`,
  - the same typed-`Object` target lanes across zero-arg parenthesized and indexed syntax.
- `IP-02` remains `in-progress`: broader `Set`/`Let` source-target parity, broader non-authoritative default-member closure beyond the currently bounded subsets, and wider Office-style call-vs-value parity are still open.

## 2026-03-18 - Add explicit Set scalar-target getter rejection evidence

- Continued the active `IP-02A` checklist run without widening semantics and added the missing direct bounded `Set`-intent rejection evidence for object-valued native getter/default-member reads targeting a scalar variable.
- Added compiler and VM/JIT host evidence proving that `Set requires Object or Variant target, got Long variable n` now directly covers:
  - named object-returning native property getters,
  - authoritative object-returning native default-member getters,
  - bounded single-visible-candidate non-authoritative object-returning native default-member getters,
  - across bare, zero-arg parenthesized, and indexed syntax where applicable.
- `IP-02` remains `in-progress`: broader `Set`/`Let` source-target parity, broader non-authoritative default-member closure beyond the currently bounded subsets, and wider Office-style call-vs-value parity are still open.

## 2026-03-18 - Remove explicit Set default-member diagnostic escape hatch

- Continued the active `IP-02A` checklist run and removed a real silent-escape path instead of only adding more proof around existing semantics.
- The native default-member read-assignment rewrite no longer swallows `PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS` or `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING` when the RHS is reached through explicit `Set`.
- Added compiler and VM/JIT host evidence proving that those deterministic diagnostics now directly cover explicit `Set` read-assignment to both typed `Object` and typed `Variant` targets across bare, zero-arg parenthesized, and indexed non-authoritative default-member syntax.
- `IP-02` remains `in-progress`: broader `Set`/`Let` source-target parity, broader non-authoritative default-member closure beyond the currently bounded subsets, and wider Office-style call-vs-value parity are still open.

## 2026-03-18 - Add explicit Set scalar-target source-resolution precedence evidence

- Continued the active `IP-02A` checklist run without widening semantics and locked the remaining scalar-target precedence neighbors on the new explicit-`Set` source-resolution surface.
- Added compiler and VM/JIT host evidence proving that explicit `Set n = widget`, `Set n = widget()`, and `Set n = widget(x)` now prefer `PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS` / `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING` over downstream target-mismatch diagnostics when the non-authoritative default-member source itself cannot be resolved.
- `IP-02` remains `in-progress`: broader `Set`/`Let` source-target parity, broader non-authoritative default-member closure beyond the currently bounded subsets, and wider Office-style call-vs-value parity are still open.

## 2026-03-18 - Add scalar-target default-member source-resolution evidence for Let and implicit assignment

- Continued the active `IP-02A` checklist run without widening semantics and filled the remaining scalar-target precedence gap for the non-authoritative object-valued read-assignment surface outside explicit `Set`.
- Added compiler and VM/JIT host evidence proving that `Let n = widget`, `n = widget`, and their zero-arg parenthesized/indexed counterparts now prefer `PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS` / `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING` over downstream scalar target-mismatch diagnostics when the non-authoritative default-member source itself cannot be resolved.
- `IP-02` remains `in-progress`: broader `Set`/`Let` source-target parity, broader non-authoritative default-member closure beyond the currently bounded subsets, and wider Office-style call-vs-value parity are still open.













