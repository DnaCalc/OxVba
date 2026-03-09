# Workset: Host Bridge Object-Value and Event-Ingress Contract Lock

Date: 2026-03-09  
Status: planned  
Scope: lock the authoritative contract for host bridge object values, collection/default-member dispatch at the bridge boundary, host-to-engine event ingress, and host error mapping posture for the embedded hosting program.

## 1. Decision summary

Decision lock:
1. The host bridge keeps a single `Variant`-based value boundary.
2. Object references crossing the host bridge are represented as object-capable `Variant` values that carry `HostObjectToken` identity.
3. Host-to-engine event delivery is explicit and engine-owned:
   - `Engine::dispatch_host_event(subscription: SubscriptionId, args: &[Variant])`
4. The host bridge itself remains host-facing:
   - resolve objects,
   - invoke methods,
   - get/set properties,
   - subscribe/unsubscribe events,
   - release objects.
5. Default-member and collection semantics are not encoded as bridge special cases.
   - The bridge exposes ordinary property/method operations.
   - Host Project + runtime semantics decide when VBA syntax implies property access, method invocation, or default-member use.

Recommendation adopted:
1. keep one pragmatic `Variant` boundary rather than inventing a second typed bridge-value system now,
2. require an explicit event-ingress call instead of hidden callback conventions.

## 2. Why this lock is needed

Without this lock, the host/tooling proposal remains underspecified in exactly the places that will destabilize P5a:
1. how object-valued properties and method returns cross the bridge,
2. how the host pushes events into the engine,
3. how collection/default-member access is expressed at the bridge boundary,
4. how host errors relate to runtime/VBA error routing.

The review was correct that these are architectural contract questions, not implementation details.

## 3. Contract shape

### 3.1 Value boundary

The bridge uses:
1. `Variant` for all inbound and outbound values,
2. object-capable `Variant` payloads for host object references,
3. typed tokens for object identity and subscriptions behind that boundary.

Normative implication:
1. object-valued property gets and method returns produce a `Variant` carrying object identity, not a parallel out-of-band object channel.
2. consumers must not rely on stringly or COM-specific conventions to distinguish object vs scalar values.

## 3.2 Host bridge responsibilities

Required bridge responsibilities:
1. `resolve_root_object(name) -> HostObjectToken`
2. `invoke_method(object, method, args: &[Variant]) -> Variant`
3. `get_property(object, property) -> Variant`
4. `set_property(object, property, value: Variant) -> ()`
5. `subscribe_event(object, event_name, handler) -> SubscriptionId`
6. `unsubscribe_event(subscription) -> ()`
7. `release_object(object) -> ()`

Interpretation:
1. the bridge is responsible for ordinary object model navigation and mutation,
2. the bridge is not responsible for VBA semantic interpretation of default-member or `WithEvents` rules,
3. the engine/runtime remains the semantic authority.

## 3.3 Event ingress

The event ingress path is explicit:
1. host raises or observes a host-side event,
2. host maps that event to a previously issued `SubscriptionId`,
3. host calls `Engine::dispatch_host_event(subscription, args)`,
4. engine/runtime resolves the handler set and executes VBA semantics.

This means:
1. event ingress is not hidden inside `subscribe_event`,
2. event dispatch is not a side effect of unrelated host bridge calls,
3. host bridge event subscription and engine event dispatch are separate but complementary contracts.

## 3.4 Collection and default-member behavior

The bridge does not invent special APIs for collection/default-member behavior.

Rules:
1. named property access uses `get_property` / `set_property`,
2. method calls use `invoke_method`,
3. collection/default-member access is expressed through ordinary resolved member operations at the semantic layer,
4. if a collection uses `Item` as its default member, the semantic/runtime layer decides that `Controls(\"x\")` maps to invoking the appropriate member on the bridged object model.

Practical consequence:
1. the bridge stays small and transportable,
2. default-member semantics remain consistent across COM and non-COM hosts.

## 3.5 Error posture

This lock does not fully define host-to-VBA error-number mapping, but it establishes direction:
1. host bridge methods return structured `HostError`,
2. engine/runtime is responsible for mapping host failures into the deterministic VBA/runtime error surface,
3. bridge methods should not embed VBA semantics directly.

Full error-number/source mapping remains a later closure item, but the ownership boundary is now explicit.

## 4. Design consequences

### 4.1 Why not a richer bridge-value enum now

Rejected for now:
1. a second typed boundary enum parallel to `Variant`.

Reason:
1. it would duplicate the runtime value model too early,
2. it would create extra marshaling work before the host bridge is even active,
3. the project already centers Automation/Variant semantics in multiple adjacent domains.

### 4.2 Why explicit event ingress matters

Accepted:
1. event dispatch must be an explicit engine-facing operation.

Reason:
1. it removes hidden conventions,
2. it aligns non-COM host events with the event-runtime work already underway,
3. it gives the pathfinder and future hosts a clear control point for dispatch.

## 5. Interface guidance

Target proposal shape:

```rust
pub trait OxvbaHostBridge {
    fn load_project(&self, id: &str) -> Result<ProjectManifest, HostError>;
    fn load_artifact(&self, id: &str) -> Result<Vec<u8>, HostError>;
    fn resolve_root_object(&self, name: &str) -> Result<HostObjectToken, HostError>;
    fn subscribe_event(
        &self,
        object: HostObjectToken,
        event_name: &str,
        handler: EventHandlerBinding,
    ) -> Result<SubscriptionId, HostError>;
    fn unsubscribe_event(&self, subscription: SubscriptionId) -> Result<(), HostError>;
    fn release_object(&self, object: HostObjectToken) -> Result<(), HostError>;
    fn invoke_method(
        &self,
        object: HostObjectToken,
        method: &str,
        args: &[Variant],
    ) -> Result<Variant, HostError>;
    fn get_property(&self, object: HostObjectToken, property: &str) -> Result<Variant, HostError>;
    fn set_property(
        &self,
        object: HostObjectToken,
        property: &str,
        value: Variant,
    ) -> Result<(), HostError>;
    fn emit_diagnostic(&self, diagnostic: EngineDiagnostic);
}

impl Engine {
    pub fn dispatch_host_event(
        &mut self,
        subscription: SubscriptionId,
        args: &[Variant],
    ) -> Result<(), HostError>;
}
```

## 6. Relation to other work

This lock resolves `F-02` in `docs/REVIEW_20260309_FOLLOWUP.md`.

It also supports:
1. host/pathfinder planning under `P5a`,
2. the existing event-runtime split workset,
3. the COM bridge repurpose decision, because COM should map into the same semantic bridge model rather than define a separate ownership model for event semantics.

## 7. Ladder and program relation

Primary future program relation:
1. P5a host bridge trait + typed tokens + test harness
2. P6 event model co-development

Supportive current relation:
1. clarifies how non-COM host-event ingress should remain semantic-owner authoritative,
2. keeps COM as a transport adapter rather than semantic owner.

## 8. Immediate next actions

1. Update the host/tooling proposal to state this contract explicitly.
2. Mark `F-02` resolved in review triage.
3. When host-bridge implementation starts, use this workset as the contract source of truth.
