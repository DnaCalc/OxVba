use crate::{
    ComCallbackPayload, ComCallbackToken, ComCallbackValue, ComMemberToken, ComObjectDescriptor,
    ComObjectToken, ComObjectTransportKind, ComSubscriptionToken, ComValue,
    TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind, TypeLibMetadataBlob,
    TypeLibParamType, runtime_class_descriptor_from_typelib_metadata,
};
use oxvba_runtime::{
    ObjectRef, RuntimeClassDescriptor, RuntimeDispatchPlan, RuntimeDispatchPlanCache,
    RuntimeInterfaceId, RuntimeMemberInvokeKind, Variant,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone)]
pub struct ComBinding {
    pub prog_id_name: String,
    pub native_dispatch: usize,
    pub native_unknown: usize,
    pub runtime_object: Option<i32>,
    pub runtime_class_descriptor: Option<&'static RuntimeClassDescriptor>,
    pub runtime_dispatch_plan_cache: RuntimeDispatchPlanCache,
    pub member_dispids: BTreeMap<ComMemberToken, i32>,
    /// Keyed by `(memid, invoke_kind)` so a read/write property's get / let
    /// (`PropertyPut`) / set (`PropertyPutRef`) FUNCDESCs — which SHARE a single
    /// memid (Scripting `Item`, `Key`, `CompareMode`) — each retain their own
    /// spec (own vtable slot + ABI param shape). A memid-only key would collapse
    /// them, which is why paired members previously declined the vtable fast path
    /// and fell back to IDispatch. Look up via [`ComBinding::lookup_member_spec`],
    /// never `.get(&token)` directly.
    pub member_specs: BTreeMap<(ComMemberToken, TypeLibMemberInvokeKind), ComMemberSpec>,
    pub default_member_token: Option<ComMemberToken>,
    pub direct_dispatch_specs: BTreeMap<ComMemberToken, ComDirectDispatchSpec>,
    pub event_specs: BTreeMap<ComMemberToken, ComEventSpec>,
    pub event_trigger_specs: BTreeMap<ComMemberToken, ComEventTriggerSpec>,
}

impl ComBinding {
    pub fn new(prog_id_name: String, native_dispatch: usize) -> Self {
        Self {
            prog_id_name,
            native_dispatch,
            native_unknown: 0,
            runtime_object: None,
            runtime_class_descriptor: None,
            runtime_dispatch_plan_cache: RuntimeDispatchPlanCache::new(),
            member_dispids: BTreeMap::new(),
            member_specs: BTreeMap::new(),
            default_member_token: None,
            direct_dispatch_specs: BTreeMap::new(),
            event_specs: BTreeMap::new(),
            event_trigger_specs: BTreeMap::new(),
        }
    }

    /// Resolve the member spec for `token` under the access `intent`. A read
    /// (`Method`/`PropertyGet`) request matches either read-side spec; a let
    /// (`PropertyPut`) request prefers the put spec then the putref; a set
    /// (`PropertyPutRef`) request prefers the putref then the put. The returned
    /// spec carries the invoke-kind's OWN vtable slot and ABI param shape, so a
    /// read/write property dispatches the correct slot for the requested kind
    /// instead of declining to IDispatch. Falls back to any spec recorded for the
    /// memid (single-kind members) when no preferred kind is present.
    pub fn lookup_member_spec(
        &self,
        token: ComMemberToken,
        intent: TypeLibMemberInvokeKind,
    ) -> Option<&ComMemberSpec> {
        use TypeLibMemberInvokeKind::{Method, PropertyGet, PropertyPut, PropertyPutRef};
        let order: &[TypeLibMemberInvokeKind] = match intent {
            Method | PropertyGet => &[PropertyGet, Method],
            PropertyPut => &[PropertyPut, PropertyPutRef],
            PropertyPutRef => &[PropertyPutRef, PropertyPut],
        };
        for kind in order {
            if let Some(spec) = self.member_specs.get(&(token, *kind)) {
                return Some(spec);
            }
        }
        // Defensive fallback: any spec recorded for this memid (a single-kind
        // member whose only invoke_kind is not in the preference list above).
        self.member_specs
            .iter()
            .find(|((spec_token, _), _)| *spec_token == token)
            .map(|(_, spec)| spec)
    }

    pub fn resolve_runtime_dispatch_plan(
        &mut self,
        member_name: &str,
        invoke_kind: TypeLibMemberInvokeKind,
        arity: usize,
    ) -> Option<RuntimeDispatchPlan> {
        let descriptor = self.runtime_class_descriptor?;
        let interface = descriptor
            .interfaces
            .iter()
            .find(|interface| interface.id == RuntimeInterfaceId::IDispatch)?;
        self.runtime_dispatch_plan_cache.resolve_member(
            interface,
            member_name,
            runtime_invoke_kind_from_typelib_member(invoke_kind),
            arity,
        )
    }

    pub fn resolve_runtime_default_dispatch_plan(
        &mut self,
        invoke_kind: TypeLibMemberInvokeKind,
        arity: usize,
    ) -> Option<RuntimeDispatchPlan> {
        let descriptor = self.runtime_class_descriptor?;
        let interface = descriptor
            .interfaces
            .iter()
            .find(|interface| interface.id == RuntimeInterfaceId::IDispatch)?;
        self.runtime_dispatch_plan_cache.resolve_default_member(
            interface,
            runtime_invoke_kind_from_typelib_member(invoke_kind),
            arity,
        )
    }

    pub fn descriptor(
        &self,
        object: ObjectRef,
        typelib_cache_key: Option<String>,
    ) -> ComObjectDescriptor {
        ComObjectDescriptor {
            object,
            prog_id_name: self.prog_id_name.clone(),
            transport: if self.native_dispatch != 0 {
                ComObjectTransportKind::NativeDispatch
            } else {
                ComObjectTransportKind::Projection
            },
            supports_events: !self.event_specs.is_empty(),
            // Composite-keyed map: project to the distinct memids (a paired
            // get/put member contributes its token once), preserving the prior
            // "one token per known member" contract for descriptor consumers.
            known_member_tokens: self
                .member_specs
                .keys()
                .map(|(token, _)| *token)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            known_event_tokens: self.event_specs.keys().copied().collect(),
            default_member_token: self.default_member_token,
            default_member_name: self
                .default_member_token
                .and_then(|token| {
                    self.lookup_member_spec(token, TypeLibMemberInvokeKind::PropertyGet)
                })
                .map(|spec| spec.name.clone()),
            typelib_cache_key,
        }
    }
}

pub fn binding_from_typelib_metadata(
    prog_id_name: String,
    native_dispatch: usize,
    metadata: Option<&TypeLibMetadataBlob>,
) -> ComBinding {
    let mut binding = ComBinding::new(prog_id_name, native_dispatch);
    binding.runtime_class_descriptor = metadata.map(runtime_class_descriptor_from_typelib_metadata);
    binding.member_specs = metadata
        .map(member_specs_from_typelib_metadata)
        .unwrap_or_default();
    binding.default_member_token = metadata.and_then(default_member_token_from_typelib_metadata);
    binding.event_specs = metadata
        .map(event_specs_from_typelib_metadata)
        .unwrap_or_default();
    binding.event_trigger_specs = metadata
        .map(event_trigger_specs_from_typelib_metadata)
        .unwrap_or_default();
    binding
}

fn normalize_ci_token(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

fn runtime_invoke_kind_from_typelib_member(
    invoke_kind: TypeLibMemberInvokeKind,
) -> RuntimeMemberInvokeKind {
    match invoke_kind {
        TypeLibMemberInvokeKind::Method => RuntimeMemberInvokeKind::Method,
        TypeLibMemberInvokeKind::PropertyGet => RuntimeMemberInvokeKind::PropertyGet,
        TypeLibMemberInvokeKind::PropertyPut => RuntimeMemberInvokeKind::PropertyLet,
        TypeLibMemberInvokeKind::PropertyPutRef => RuntimeMemberInvokeKind::PropertySet,
    }
}

fn event_trigger_specs_from_typelib_metadata(
    blob: &TypeLibMetadataBlob,
) -> BTreeMap<ComMemberToken, ComEventTriggerSpec> {
    let events_by_name: BTreeMap<String, &TypeLibEventMetadata> = blob
        .events
        .iter()
        .map(|event| (normalize_ci_token(&event.name), event))
        .collect();
    let mut out = BTreeMap::new();
    for member in &blob.members {
        let normalized_member_name = normalize_ci_token(&member.name);
        let event_name = normalized_member_name
            .strip_prefix("fire")
            .or_else(|| normalized_member_name.strip_prefix("raise"))
            .unwrap_or(normalized_member_name.as_str());
        let Some(event) = events_by_name.get(event_name) else {
            continue;
        };
        out.insert(
            member.token.into(),
            ComEventTriggerSpec {
                event_token: event.token.into(),
                callback_arity: usize::from(event.callback_arity),
                second_arg_is_incremented: normalized_member_name.ends_with("pair"),
            },
        );
    }
    out
}

fn event_specs_from_typelib_metadata(
    blob: &TypeLibMetadataBlob,
) -> BTreeMap<ComMemberToken, ComEventSpec> {
    blob.events
        .iter()
        .map(|event| {
            (
                event.token.into(),
                ComEventSpec {
                    callback_arity: usize::from(event.callback_arity),
                    path: match event.dispatch_path {
                        TypeLibEventDispatchPath::Dispatch => ComEventPath::Dispatch,
                        TypeLibEventDispatchPath::SourceInterface => ComEventPath::SourceInterface,
                    },
                    connection_point_iid: event.connection_point_iid.clone(),
                    dispatch_member_id: event.dispatch_member_id,
                },
            )
        })
        .collect()
}

fn member_specs_from_typelib_metadata(
    blob: &TypeLibMetadataBlob,
) -> BTreeMap<(ComMemberToken, TypeLibMemberInvokeKind), ComMemberSpec> {
    blob.members
        .iter()
        .map(|member| {
            (
                (member.token.into(), member.invoke_kind),
                ComMemberSpec {
                    name: member.name.clone(),
                    requires_argument: member.requires_argument,
                    invoke_kind: member.invoke_kind,
                    parameter_names: member.parameter_names.clone(),
                    is_default_member: member.is_default_member,
                    vtable_slot: member.vtable_slot,
                    parameter_types: member.parameter_types.clone(),
                    // Bug-4b: carry the per-parameter object-arg interface IIDs so the
                    // vtable marshaller can QI each object arg to its declared interface.
                    parameter_iids: member.parameter_iids.clone(),
                    // D3: carry the per-parameter omitted-optional synthesis rules
                    // so the vtable gate can admit a call that drops trailing
                    // optionals (with a default or a VARIANT) and the dispatch site
                    // can synthesize them in declared order.
                    parameter_optional_defaults: member.parameter_optional_defaults.clone(),
                    return_type: member.return_type,
                    // The FUNCDESC callconv is not part of the projected
                    // metadata blob; honor the explicit `callconv_is_stdcall`
                    // bit the live loader stamps onto `TypeLibMemberMetadata`.
                    callconv_is_stdcall: member.callconv_is_stdcall,
                    // S5a: carry the dual interface IID so the vtable dispatch
                    // site can QueryInterface for it before any slot call.
                    interface_iid: member.interface_iid,
                    // Carry the FDUAL/source-typekind/AV-bound triad the gate
                    // keys on (previously dropped, which is how a pure
                    // dispinterface member slipped past with a slot).
                    is_dual: member.is_dual,
                    source_typekind: member.source_typekind,
                    vtable_slot_bound: member.vtable_slot_bound,
                },
            )
        })
        .collect()
}

fn default_member_token_from_typelib_metadata(
    blob: &TypeLibMetadataBlob,
) -> Option<ComMemberToken> {
    blob.members
        .iter()
        .find(|member| member.is_default_member)
        .map(|member| member.token.into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComEventSubscription<TTransport> {
    pub object: i32,
    pub event: ComMemberToken,
    pub transport: TTransport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComEventCallbackValue {
    value: Variant,
}

impl ComEventCallbackValue {
    pub fn from_com_value(value: &ComValue) -> Self {
        Self {
            value: value
                .to_variant()
                .expect("COM event callback values must be exact VARIANT-compatible"),
        }
    }

    pub fn to_com_value(&self) -> ComValue {
        ComValue::from_variant(&self.value)
            .expect("queued COM event callback VARIANT must project to ComValue")
    }

    pub fn variant(&self) -> &Variant {
        &self.value
    }
}

// COM event callbacks can enter from native connection point threads. The
// retained VARIANT owns or addrefs its BSTR, SAFEARRAY, and IUnknown payloads
// (BStr/SafeArray clones are deep copies; refcounts are atomic), so the
// cross-thread *memory* story is sound.
//
// INVARIANT this impl does NOT cover (W1-com-007): an `ObjectRef`-bearing
// Variant dropped on a foreign thread releases through `compat_release`,
// which parks zero-refcount project objects into that thread's thread_local
// TERMINATIONS queue — a queue only the VM thread drains. Since W1-com-008
// the native event sinks hold the client state only WEAKLY, so the state
// (and its queued callbacks) can no longer be dropped on a COM event thread
// via sink release; the residual exposure is a host that hands its last
// strong `Arc<Mutex<WindowsComClientState>>` to another thread itself. A
// process-global termination queue would close that for good.
// SAFETY: the wrapped VARIANT owns or atomically addrefs every BSTR, SAFEARRAY,
// and IUnknown payload it carries (deep clones, atomic refcounts — see the block
// comment above), so moving the value to another thread transfers sole ownership
// of memory that is valid there; the W1-com-007 foreign-thread-drop residual is
// documented above and mitigated since W1-com-008 by sinks holding the client
// state weakly.
unsafe impl Send for ComEventCallbackValue {}
// SAFETY: same ownership argument as the Send impl above; shared references only
// expose read access to the owned VARIANT (`variant()`/`to_com_value` never
// mutate), so concurrent shared reads cannot race.
unsafe impl Sync for ComEventCallbackValue {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComEventCallback {
    pub subscription: ComSubscriptionToken,
    pub object: i32,
    pub event: ComMemberToken,
    pub args: Vec<ComEventCallbackValue>,
    /// `(arg_index, git_cookie)` for object-typed event arguments delivered on a
    /// cross-apartment thread (an out-of-process source's agile sink fires on an
    /// MTA RPC worker, but the VM consumes events on its STA thread). Each object
    /// arg is registered in the process Global Interface Table on the delivery
    /// thread, leaving a `Nothing` placeholder in `args[arg_index]`; the Windows
    /// poll path revives it into a thread-correct binding before dispatch. Empty
    /// for in-process/projection callbacks (their args are already valid).
    pub pending_marshals: Vec<(usize, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComEventPath {
    Dispatch,
    SourceInterface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComEventSpec {
    pub callback_arity: usize,
    pub path: ComEventPath,
    pub connection_point_iid: Option<String>,
    pub dispatch_member_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComMemberSpec {
    pub name: String,
    pub requires_argument: bool,
    pub invoke_kind: TypeLibMemberInvokeKind,
    pub parameter_names: Vec<String>,
    pub is_default_member: bool,
    /// x64 vtable **slot index** (NOT a byte offset). The live typelib loader
    /// divides `FUNCDESC::oVft` by 8 before storing it (see
    /// `extract_members_from_typeinfo`), so this is the index into
    /// `(*(*this))[slot]` that the S2 `vtable_invoke` marshaller uses directly.
    /// `None` for dispinterface-only members with no vtable slot.
    pub vtable_slot: Option<u16>,
    /// Per-parameter VARTYPEs carried from the FUNCDESC, left-to-right (the
    /// `[out,retval]` parameter is surfaced as `return_type`, not here).
    pub parameter_types: Vec<TypeLibParamType>,
    /// Per-parameter interface IID (parallel to `parameter_types`), `Some` only for an
    /// OBJECT-typed `[in]` parameter declared as a specific interface (`IFoo*`). The
    /// vtable marshaller `QueryInterface`s the supplied object for this exact IID before
    /// passing it, so the callee receives the interface vtable it expects (passing a raw
    /// `IDispatch` where `IFoo*` is declared would call the wrong vtable → host AV). An
    /// empty vec, or a `None` entry, means "no recovered IID" and the vtable gate declines
    /// that object arg to the IDispatch path. (Bug-4b: bare-interface `[in]` object arg.)
    pub parameter_iids: Vec<Option<crate::ComInterfaceIid>>,
    /// Per-parameter (parallel to `parameter_types`) synthesis rule for an OMITTED
    /// trailing positional argument on the vtable slot path (D3). When the guest
    /// supplies fewer positionals than `parameter_types.len()`, the vtable gate
    /// admits the call only if every missing trailing entry here is synthesizable
    /// (a `HasDefault` value or an `OptionalVariant`), and the dispatch site
    /// appends the synthesized args in declared order before the `[out,retval]`.
    /// Empty (fixture/catalog metadata) means "exact arity only", as before D3.
    pub parameter_optional_defaults: Vec<crate::OptionalParamDefault>,
    /// The member's logical return type (typically the `[out,retval]` T* of a
    /// dual member, whose ABI return is the HRESULT). `None` for `void`/HRESULT.
    pub return_type: Option<TypeLibParamType>,
    /// True when the member's FUNCDESC declares `CC_STDCALL` (callconv == 4),
    /// the only calling convention the x64 vtable marshaller may call.
    pub callconv_is_stdcall: bool,
    /// IID of the member's defining **dual interface**. For a live-recovered
    /// FDUAL member this is the PARTNER `TKIND_INTERFACE`'s GUID; for a
    /// registered-typelib interface it is the containing `ITypeInfo`'s GUID. The
    /// vtable dispatch site `QueryInterface`s the live object for this exact IID
    /// and calls the slot on the returned interface pointer instead of the raw
    /// `IDispatch`. `None` when no live interface identity is known (the vtable
    /// path then declines and falls back to `IDispatch`).
    pub interface_iid: Option<crate::ComInterfaceIid>,
    /// True when the member's defining type carried `TYPEFLAG_FDUAL`. The vtable
    /// gate requires `is_dual && source_typekind == Interface`.
    pub is_dual: bool,
    /// The `TKIND` the vtable metadata was sourced from. A slot is only callable
    /// when this is `Interface` (a real vtable); a pure `dispinterface` member
    /// (`Dispatch`) has no slot. The gate rejects anything but `Interface`.
    pub source_typekind: Option<crate::SourceTypeKind>,
    /// AV-safety bound: `cbSizeVft / size_of::<*const c_void>()` of the source
    /// INTERFACE typeinfo (= `cbSizeVft/8` on x64). The gate requires
    /// `slot < vtable_slot_bound` so a slot can never over-run the live vtable.
    /// `None` when the bound is unknown (the gate then declines).
    pub vtable_slot_bound: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComDirectDispatchSpec {
    pub requires_argument: bool,
    pub invoke_kind: TypeLibMemberInvokeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComEventTriggerSpec {
    pub event_token: ComMemberToken,
    pub callback_arity: usize,
    pub second_arg_is_incremented: bool,
}

#[derive(Debug)]
pub struct ComRuntimeState<TTransport> {
    pub next_handle: i32,
    pub next_subscription: i32,
    pub next_callback: i32,
    pub bindings: BTreeMap<ComObjectToken, ComBinding>,
    pub subscriptions: BTreeMap<ComSubscriptionToken, ComEventSubscription<TTransport>>,
    pub callbacks: BTreeMap<ComCallbackToken, ComEventCallback>,
    pub pending_callbacks: VecDeque<ComCallbackToken>,
    pub last_pumped_callback: Option<ComCallbackToken>,
}

impl<TTransport> Default for ComRuntimeState<TTransport> {
    fn default() -> Self {
        Self {
            next_handle: 0,
            next_subscription: 0,
            next_callback: 0,
            bindings: BTreeMap::new(),
            subscriptions: BTreeMap::new(),
            callbacks: BTreeMap::new(),
            pending_callbacks: VecDeque::new(),
            last_pumped_callback: None,
        }
    }
}

impl<TTransport: Clone> ComRuntimeState<TTransport> {
    pub fn allocate_handle(&mut self) -> ComObjectToken {
        self.next_handle = self.next_handle.saturating_add(1).max(1);
        ComObjectToken::new(20_000i32.saturating_add(self.next_handle))
    }

    pub fn allocate_subscription(&mut self) -> ComSubscriptionToken {
        self.next_subscription = self.next_subscription.saturating_add(1).max(1);
        ComSubscriptionToken::new(40_000i32.saturating_add(self.next_subscription))
    }

    pub fn allocate_callback(&mut self) -> ComCallbackToken {
        self.next_callback = self.next_callback.saturating_add(1).max(1);
        ComCallbackToken::new(60_000i32.saturating_add(self.next_callback))
    }

    pub fn queue_callback_for_subscription(
        &mut self,
        subscription: ComSubscriptionToken,
        args: &[ComValue],
    ) -> bool {
        self.queue_callback_for_subscription_with_marshals(subscription, args, &[])
    }

    /// As [`Self::queue_callback_for_subscription`], but records the
    /// `(arg_index, git_cookie)` cross-apartment marshals for object-typed event
    /// arguments (see [`ComEventCallback::pending_marshals`]). The native
    /// out-of-process event sink registers each object arg in the Global Interface
    /// Table on its delivery thread and queues a `Nothing` placeholder; the Windows
    /// poll path revives the real object on the VM thread before dispatch.
    pub fn queue_callback_for_subscription_with_marshals(
        &mut self,
        subscription: ComSubscriptionToken,
        args: &[ComValue],
        marshals: &[(usize, u32)],
    ) -> bool {
        let Some(entry) = self.subscriptions.get(&subscription).cloned() else {
            return false;
        };
        let callback = self.allocate_callback();
        self.callbacks.insert(
            callback,
            ComEventCallback {
                subscription,
                object: entry.object,
                event: entry.event,
                args: args
                    .iter()
                    .map(ComEventCallbackValue::from_com_value)
                    .collect(),
                pending_marshals: marshals.to_vec(),
            },
        );
        self.pending_callbacks.push_back(callback);
        true
    }

    pub fn queue_callbacks_for_source_event(
        &mut self,
        object: &ObjectRef,
        event: ComMemberToken,
        args: &[ComValue],
        is_projection_transport: impl Fn(&TTransport) -> bool,
    ) -> usize {
        let targets: Vec<ComSubscriptionToken> = self
            .subscriptions
            .iter()
            .filter_map(|(subscription, entry)| {
                if entry.object == object.raw()
                    && entry.event == event
                    && is_projection_transport(&entry.transport)
                {
                    Some(*subscription)
                } else {
                    None
                }
            })
            .collect();
        for subscription in &targets {
            let _ = self.queue_callback_for_subscription(*subscription, args);
        }
        targets.len()
    }

    pub fn mark_next_callback_pumped(&mut self) -> Option<ComCallbackToken> {
        if let Some(callback) = self.last_pumped_callback {
            return Some(callback);
        }
        let callback = self.pending_callbacks.pop_front()?;
        self.last_pumped_callback = Some(callback);
        Some(callback)
    }

    pub fn take_polled_callback(&mut self) -> Option<ComCallbackPayload> {
        let callback = self
            .last_pumped_callback
            .take()
            .or_else(|| self.pending_callbacks.pop_front())?;
        let payload = self.callbacks.remove(&callback)?;
        Some(ComCallbackPayload {
            callback,
            subscription: payload.subscription,
            object: ObjectRef::from_compat_identity(payload.object),
            event: payload.event,
            args: payload
                .args
                .iter()
                .map(ComEventCallbackValue::to_com_value)
                .map(ComCallbackValue::from_com_value)
                .collect(),
        })
    }

    pub fn release_object_state(
        &mut self,
        object: ComObjectToken,
    ) -> Option<(ComBinding, Vec<TTransport>, BTreeSet<ComCallbackToken>)> {
        let binding = self.bindings.remove(&object)?;
        let subscriptions: Vec<(ComSubscriptionToken, ComEventSubscription<TTransport>)> = self
            .subscriptions
            .iter()
            .filter_map(|(subscription, entry)| {
                if entry.object == object.raw() {
                    Some((*subscription, entry.clone()))
                } else {
                    None
                }
            })
            .collect();
        for (subscription, _) in &subscriptions {
            self.subscriptions.remove(subscription);
        }
        let stale_callbacks: BTreeSet<ComCallbackToken> = self
            .callbacks
            .iter()
            .filter_map(|(callback, payload)| {
                if payload.object == object.raw() {
                    Some(*callback)
                } else {
                    None
                }
            })
            .collect();
        for callback in &stale_callbacks {
            self.callbacks.remove(callback);
        }
        self.pending_callbacks
            .retain(|callback| !stale_callbacks.contains(callback));
        if self
            .last_pumped_callback
            .is_some_and(|callback| stale_callbacks.contains(&callback))
        {
            self.last_pumped_callback = None;
        }
        Some((
            binding,
            subscriptions
                .into_iter()
                .map(|(_, entry)| entry.transport)
                .collect(),
            stale_callbacks,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ComBinding, ComEventSubscription, ComRuntimeState, binding_from_typelib_metadata,
        member_specs_from_typelib_metadata,
    };
    use crate::{
        ComInterfaceIid, ComMemberToken, ComValue, TypeLibMemberInvokeKind, TypeLibMemberMetadata,
        TypeLibMetadataBlob, TypeLibParamType, TypeLibResolvedIdentity,
    };
    use oxvba_runtime::{ObjectRef, RuntimeInterfaceId, RuntimeMemberInvokeKind, VarType};

    fn sample_typelib_identity() -> TypeLibResolvedIdentity {
        TypeLibResolvedIdentity {
            reference_name: "TestLib".to_string(),
            requested_coclass: Some("Widget".to_string()),
            importlib: "TestLib".to_string(),
            libid: Some("{00000000-0000-0000-0000-000000000001}".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "TestLib:1.0".to_string(),
        }
    }

    #[test]
    fn binding_from_typelib_metadata_carries_runtime_descriptor_projection() {
        let metadata = TypeLibMetadataBlob {
            identity: sample_typelib_identity(),
            activation_prog_id: Some("TestLib.Widget".to_string()),
            member_name_to_token: vec![("Lookup".to_string(), 7)],
            members: vec![TypeLibMemberMetadata {
                name: "Lookup".to_string(),
                token: 7,
                vtable_slot: Some(9),
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["key".to_string()],
                parameter_optional: vec![true],
                parameter_optional_defaults: Vec::new(),
                is_default_member: true,
                parameter_types: vec![TypeLibParamType::String],
                parameter_wire_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: Some(TypeLibParamType::Long),
                return_wire_type: None,
                callconv_is_stdcall: true,
                is_dual: true,
                interface_iid: None,
                source_typekind: Some(crate::SourceTypeKind::Interface),
                vtable_slot_bound: Some(64),
            }],
            events: Vec::new(),
            coclass_names: Vec::new(),
        };

        let binding =
            binding_from_typelib_metadata("TestLib.Widget".to_string(), 123, Some(&metadata));
        let descriptor = binding
            .runtime_class_descriptor
            .expect("typelib-backed binding should retain runtime descriptor projection");
        assert_eq!(descriptor.name, "TestLib.Widget");
        let dispatch = descriptor
            .interfaces
            .iter()
            .find(|interface| interface.id == RuntimeInterfaceId::IDispatch)
            .expect("typelib-backed binding should expose dispatch descriptor");
        assert!(dispatch.dual_dispatch);
        assert_eq!(dispatch.members[0].name, "Lookup");
        assert_eq!(dispatch.members[0].dispatch_id, 7);
        assert_eq!(dispatch.members[0].vtable_slot, Some(9));
        assert_eq!(
            dispatch.members[0].invoke_kind,
            RuntimeMemberInvokeKind::Method
        );
        assert_eq!(dispatch.members[0].params[0].name, "key");
        assert!(dispatch.members[0].params[0].optional);

        let mut binding = binding;
        let plan = binding
            .resolve_runtime_dispatch_plan(" lookup ", TypeLibMemberInvokeKind::Method, 1)
            .expect("binding-level runtime descriptor plan should resolve by normalized name");
        assert_eq!(plan.dispatch_id, 7);
        assert_eq!(plan.vtable_slot, Some(9));
        assert_eq!(binding.runtime_dispatch_plan_cache.len(), 1);
        let cached = binding
            .resolve_runtime_dispatch_plan("LOOKUP", TypeLibMemberInvokeKind::Method, 1)
            .expect("binding-level runtime descriptor plan should be cached");
        assert_eq!(plan, cached);
        assert_eq!(binding.runtime_dispatch_plan_cache.len(), 1);
        let default_plan = binding
            .resolve_runtime_default_dispatch_plan(TypeLibMemberInvokeKind::Method, 1)
            .expect("binding-level default descriptor plan should resolve");
        assert_eq!(default_plan.dispatch_id, 7);
        assert_eq!(binding.runtime_dispatch_plan_cache.len(), 2);
    }

    #[test]
    fn member_specs_carry_funcdesc_vtable_signature() {
        // S1: the FUNCDESC vtable signature (slot index + param VARTYPEs +
        // return type + callconv) must survive the projection into
        // `ComMemberSpec`, where today it was dropped. `vtable_slot` carries the
        // x64 slot INDEX (oVft / 8), not the raw byte offset.
        let metadata = TypeLibMetadataBlob {
            identity: sample_typelib_identity(),
            activation_prog_id: Some("TestLib.Widget".to_string()),
            member_name_to_token: vec![("Lookup".to_string(), 7)],
            members: vec![TypeLibMemberMetadata {
                name: "Lookup".to_string(),
                token: 7,
                // Slot index 9 (== oVft 72 / 8); past the 7 IUnknown+IDispatch
                // slots, so a real dual-interface custom slot.
                vtable_slot: Some(9),
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
                parameter_names: vec!["key".to_string()],
                parameter_optional: vec![false],
                parameter_optional_defaults: Vec::new(),
                is_default_member: false,
                parameter_types: vec![TypeLibParamType::String],
                parameter_wire_types: Vec::new(),
                parameter_iids: Vec::new(),
                return_type: Some(TypeLibParamType::Long),
                return_wire_type: None,
                callconv_is_stdcall: true,
                is_dual: true,
                interface_iid: Some(ComInterfaceIid {
                    data1: 0x1234_5678,
                    data2: 0x9abc,
                    data3: 0xdef0,
                    data4: [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80],
                }),
                source_typekind: Some(crate::SourceTypeKind::Interface),
                vtable_slot_bound: Some(64),
            }],
            events: Vec::new(),
            coclass_names: Vec::new(),
        };

        let specs = member_specs_from_typelib_metadata(&metadata);
        let spec = specs
            .get(&(ComMemberToken::new(7), TypeLibMemberInvokeKind::Method))
            .expect("Lookup member spec should be projected from the metadata blob");
        assert_eq!(spec.name, "Lookup");
        assert_eq!(
            spec.vtable_slot,
            Some(9),
            "the divided x64 slot index must reach the member spec"
        );
        assert_eq!(
            spec.parameter_types,
            vec![TypeLibParamType::String],
            "FUNCDESC parameter VARTYPEs must reach the member spec"
        );
        assert_eq!(
            spec.return_type,
            Some(TypeLibParamType::Long),
            "the [out,retval] return type must reach the member spec"
        );
        assert!(
            spec.callconv_is_stdcall,
            "a CC_STDCALL dual member must carry callconv_is_stdcall == true"
        );
        assert_eq!(
            spec.interface_iid,
            Some(ComInterfaceIid {
                data1: 0x1234_5678,
                data2: 0x9abc,
                data3: 0xdef0,
                data4: [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80],
            }),
            "the dual interface IID must reach the member spec for QI dispatch (S5a)"
        );
        // The FDUAL/source-typekind/AV-bound triad must also project (previously
        // dropped); the vtable gate keys on all three.
        assert!(spec.is_dual, "is_dual must reach the member spec");
        assert_eq!(
            spec.source_typekind,
            Some(crate::SourceTypeKind::Interface),
            "the source TKIND must reach the member spec"
        );
        assert_eq!(
            spec.vtable_slot_bound,
            Some(64),
            "the AV-safety slot bound must reach the member spec"
        );
    }

    #[test]
    fn runtime_state_queues_projection_callbacks() {
        let mut state = ComRuntimeState::<bool>::default();
        let object = state.allocate_handle();
        let object_ref = ObjectRef::from_compat_identity(object.raw());
        let subscription = state.allocate_subscription();
        state.subscriptions.insert(
            subscription,
            ComEventSubscription {
                object: object_ref.raw(),
                event: ComMemberToken::new(11),
                transport: true,
            },
        );

        let queued = state.queue_callbacks_for_source_event(
            &object_ref,
            ComMemberToken::new(11),
            &[ComValue::I32(7)],
            |transport| *transport,
        );
        assert_eq!(queued, 1);

        let payload = state
            .take_polled_callback()
            .expect("queued callback should be available");
        assert_eq!(payload.subscription.raw(), subscription.raw());
        assert_eq!(payload.object.raw(), object_ref.raw());
        assert_eq!(payload.args[0].to_com_value(), ComValue::I32(7));
    }

    #[test]
    fn runtime_state_retains_callback_args_as_variants() {
        let mut state = ComRuntimeState::<bool>::default();
        let object = state.allocate_handle();
        let object_ref = ObjectRef::from_compat_identity(object.raw());
        let subscription = state.allocate_subscription();
        state.subscriptions.insert(
            subscription,
            ComEventSubscription {
                object: object_ref.raw(),
                event: ComMemberToken::new(12),
                transport: true,
            },
        );

        assert!(state.queue_callback_for_subscription(subscription, &[ComValue::I32(9)]));
        let callback = *state
            .pending_callbacks
            .front()
            .expect("queued callback token should be retained");
        let payload = state
            .callbacks
            .get(&callback)
            .expect("queued callback payload should be retained");
        assert_eq!(payload.args[0].variant().vtype(), VarType::Long);

        let projected = state
            .take_polled_callback()
            .expect("queued callback should still project through public payload");
        assert_eq!(projected.args[0].to_com_value(), ComValue::I32(9));
    }

    #[test]
    fn runtime_state_release_object_clears_related_tracking() {
        let mut state = ComRuntimeState::<bool>::default();
        let object = state.allocate_handle();
        state
            .bindings
            .insert(object, ComBinding::new("Test.Object".to_string(), 123));
        let subscription = state.allocate_subscription();
        state.subscriptions.insert(
            subscription,
            ComEventSubscription {
                object: object.raw(),
                event: ComMemberToken::new(12),
                transport: true,
            },
        );
        assert!(state.queue_callback_for_subscription(subscription, &[ComValue::I32(9)]));

        let (_, transports, callbacks) = state
            .release_object_state(object)
            .expect("object state should be releasable");
        assert_eq!(transports, vec![true]);
        assert_eq!(callbacks.len(), 1);
        assert!(!state.bindings.contains_key(&object));
        assert!(state.subscriptions.is_empty());
        assert!(state.callbacks.is_empty());
        assert!(state.pending_callbacks.is_empty());
    }
}
