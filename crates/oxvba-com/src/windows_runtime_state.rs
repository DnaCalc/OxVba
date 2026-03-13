#![allow(unsafe_op_in_unsafe_fn)]

use crate::{
    ComBinding, ComCallbackToken, ComEventPath, ComEventSpec, ComMemberToken, ComObjectToken,
    ComRuntimeState, ComSubscriptionToken, ComValue, DispatchEventSinkConfig, RawIDispatch,
    WindowsConnectionPointTransport, release_dispatch, source_interface_event_spec_supported,
    try_advise_dispatch_event_sink, try_advise_single_i32_source_interface_event_sink,
    unadvise_connection_point,
};
use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

const COM_EVENT_DISPATCH_MEMBER_WILDCARD: i32 = i32::MIN + 3_333;

type DispatchEventCallback = Arc<dyn Fn(&[ComValue]) -> bool + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsComSubscriptionTransport {
    Projection,
    NativeConnectionPoint(WindowsConnectionPointTransport),
}

impl WindowsComSubscriptionTransport {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Projection => "projection",
            Self::NativeConnectionPoint(_) => "native-connection-point",
        }
    }

    pub fn is_projection(&self) -> bool {
        matches!(self, Self::Projection)
    }
}

#[derive(Debug, Default)]
pub struct WindowsComClientState {
    inner: ComRuntimeState<WindowsComSubscriptionTransport>,
}

impl Deref for WindowsComClientState {
    type Target = ComRuntimeState<WindowsComSubscriptionTransport>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for WindowsComClientState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Drop for WindowsComClientState {
    fn drop(&mut self) {
        for subscription in self.subscriptions.values() {
            if let WindowsComSubscriptionTransport::NativeConnectionPoint(native) =
                subscription.transport
            {
                unsafe {
                    let _ = unadvise_connection_point(native);
                }
            }
        }
        self.subscriptions.clear();
        self.callbacks.clear();
        self.pending_callbacks.clear();
        for binding in self.bindings.values_mut() {
            if binding.native_dispatch != 0 {
                unsafe {
                    release_dispatch(binding.native_dispatch as *mut RawIDispatch);
                }
                binding.native_dispatch = 0;
            }
        }
        self.bindings.clear();
    }
}

fn callback_sink(
    com_state: Arc<Mutex<WindowsComClientState>>,
    subscription: ComSubscriptionToken,
) -> DispatchEventCallback {
    Arc::new(move |args: &[ComValue]| {
        let mut state = match com_state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.queue_callback_for_subscription(subscription, args)
    })
}

/// # Safety
///
/// `dispatch` must be a valid live `IDispatch` pointer, `spec` and
/// `connection_point_iid` must describe the event interface implemented by that
/// object, and `com_state` must remain a valid shared runtime container for the
/// lifetime of any transport returned by this call.
pub unsafe fn advise_event_subscription(
    dispatch: *mut RawIDispatch,
    com_state: Arc<Mutex<WindowsComClientState>>,
    subscription: ComSubscriptionToken,
    spec: &ComEventSpec,
    expected_arity: usize,
    connection_point_iid: &str,
) -> Result<Option<WindowsConnectionPointTransport>, String> {
    match spec.path {
        ComEventPath::Dispatch => try_advise_dispatch_event_sink(
            dispatch,
            connection_point_iid,
            DispatchEventSinkConfig {
                event_dispatch_member: spec
                    .dispatch_member_id
                    .unwrap_or(COM_EVENT_DISPATCH_MEMBER_WILDCARD),
                expected_arity,
                connection_point_iid: None,
                on_event: callback_sink(com_state, subscription),
            },
        ),
        ComEventPath::SourceInterface => {
            if !source_interface_event_spec_supported(spec) {
                return Err(format!(
                    "COM-E-EVENT-PATH-UNSUPPORTED: source-interface COM event callbacks (COM-EVT-B) are unsupported for connection-point IID `{connection_point_iid}` in current lane"
                ));
            }
            try_advise_single_i32_source_interface_event_sink(
                dispatch,
                connection_point_iid,
                expected_arity,
                callback_sink(com_state, subscription),
            )
        }
    }
}

/// # Safety
///
/// `transport` must have been produced by `advise_event_subscription` and must
/// still represent a live connection-point subscription owned by the caller.
pub unsafe fn release_subscription_transport(
    transport: WindowsComSubscriptionTransport,
) -> Result<(), String> {
    if let WindowsComSubscriptionTransport::NativeConnectionPoint(native) = transport {
        unadvise_connection_point(native)?;
    }
    Ok(())
}

pub fn event_signature_arity_for_binding(
    binding: &ComBinding,
    event: ComMemberToken,
) -> Option<usize> {
    binding
        .event_specs
        .get(&event)
        .map(|spec| spec.callback_arity)
}

pub fn event_is_source_interface_only(binding: &ComBinding, event: ComMemberToken) -> bool {
    binding
        .event_specs
        .get(&event)
        .is_some_and(|spec| matches!(spec.path, ComEventPath::SourceInterface))
}

pub fn event_callback_args_from_member_token(
    binding: &ComBinding,
    member: ComMemberToken,
    args: &[i32],
) -> Option<(ComMemberToken, Vec<ComValue>)> {
    let trigger_spec = binding.event_trigger_specs.get(&member)?;
    if args.len() < trigger_spec.callback_arity {
        return None;
    }
    let mut values: Vec<ComValue> = args
        .iter()
        .copied()
        .take(trigger_spec.callback_arity)
        .map(ComValue::I32)
        .collect();
    if trigger_spec.second_arg_is_incremented
        && values.len() >= 2
        && let ComValue::I32(first) = values[0]
    {
        values[1] = ComValue::I32(first.saturating_add(1));
    }
    Some((trigger_spec.event_token, values))
}

pub fn collect_stale_callbacks_for_subscription(
    state: &WindowsComClientState,
    subscription: ComSubscriptionToken,
    object: ComObjectToken,
) -> BTreeSet<ComCallbackToken> {
    state
        .callbacks
        .iter()
        .filter_map(|(callback, payload)| {
            if payload.subscription == subscription && payload.object == object {
                Some(*callback)
            } else {
                None
            }
        })
        .collect()
}
