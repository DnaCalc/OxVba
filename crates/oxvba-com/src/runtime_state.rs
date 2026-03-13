use crate::{
    ComCallbackPayload, ComCallbackToken, ComMemberToken, ComObjectDescriptor, ComObjectToken,
    ComObjectTransportKind, ComSubscriptionToken, ComValue, TypeLibMemberInvokeKind,
};
use oxvba_runtime::ObjectHandle;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone)]
pub struct ComBinding {
    pub prog_id_name: String,
    pub native_dispatch: usize,
    pub member_dispids: BTreeMap<ComMemberToken, i32>,
    pub member_specs: BTreeMap<ComMemberToken, ComMemberSpec>,
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
            member_dispids: BTreeMap::new(),
            member_specs: BTreeMap::new(),
            default_member_token: None,
            direct_dispatch_specs: BTreeMap::new(),
            event_specs: BTreeMap::new(),
            event_trigger_specs: BTreeMap::new(),
        }
    }

    pub fn descriptor(
        &self,
        object: ObjectHandle,
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
            known_member_tokens: self.member_specs.keys().copied().collect(),
            known_event_tokens: self.event_specs.keys().copied().collect(),
            default_member_token: self.default_member_token,
            default_member_name: self
                .default_member_token
                .and_then(|token| self.member_specs.get(&token))
                .map(|spec| spec.name.clone()),
            typelib_cache_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComEventSubscription<TTransport> {
    pub object: ComObjectToken,
    pub event: ComMemberToken,
    pub transport: TTransport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComEventCallback {
    pub subscription: ComSubscriptionToken,
    pub object: ComObjectToken,
    pub event: ComMemberToken,
    pub args: Vec<ComValue>,
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
                args: args.to_vec(),
            },
        );
        self.pending_callbacks.push_back(callback);
        true
    }

    pub fn queue_callbacks_for_source_event(
        &mut self,
        object: ComObjectToken,
        event: ComMemberToken,
        args: &[ComValue],
        is_projection_transport: impl Fn(&TTransport) -> bool,
    ) -> usize {
        let targets: Vec<ComSubscriptionToken> = self
            .subscriptions
            .iter()
            .filter_map(|(subscription, entry)| {
                if entry.object == object
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
            object: ObjectHandle::new(payload.object.raw()),
            event: payload.event,
            args: payload.args,
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
                if entry.object == object {
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
                if payload.object == object {
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
    use super::{ComBinding, ComEventSubscription, ComRuntimeState};
    use crate::{ComMemberToken, ComObjectToken, ComValue};

    #[test]
    fn runtime_state_queues_projection_callbacks() {
        let mut state = ComRuntimeState::<bool>::default();
        let object = state.allocate_handle();
        let subscription = state.allocate_subscription();
        state.subscriptions.insert(
            subscription,
            ComEventSubscription {
                object,
                event: ComMemberToken::new(11),
                transport: true,
            },
        );

        let queued = state.queue_callbacks_for_source_event(
            ComObjectToken::new(object.raw()),
            ComMemberToken::new(11),
            &[ComValue::I32(7)],
            |transport| *transport,
        );
        assert_eq!(queued, 1);

        let payload = state
            .take_polled_callback()
            .expect("queued callback should be available");
        assert_eq!(payload.subscription.raw(), subscription.raw());
        assert_eq!(payload.object.raw(), object.raw());
        assert_eq!(payload.args, vec![ComValue::I32(7)]);
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
                object,
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
