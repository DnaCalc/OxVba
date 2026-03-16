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












