use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex, Weak},
};

use crossbeam_channel::TryRecvError;
use serde::{Deserialize, Serialize};

use crate::{
    config::DebugEventChannelMode,
    views::{DebugBreakpointView, DebugModuleView, DebugSourceLocationView, DebugStopReasonView},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugOutputChannel {
    Stdout,
    Stderr,
    Host,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugBreakpointChangeKind {
    Added,
    Changed,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugEvent {
    Stopped {
        seq: u64,
        session_id: String,
        reason: DebugStopReasonView,
        thread_id: Option<u32>,
        frame_id: String,
        location: Option<DebugSourceLocationView>,
    },
    Output {
        seq: u64,
        session_id: String,
        channel: DebugOutputChannel,
        text: String,
    },
    Continued {
        seq: u64,
        session_id: String,
        all_threads_continued: bool,
    },
    Exited {
        seq: u64,
        session_id: String,
        exit_code: Option<i32>,
    },
    BreakpointChanged {
        seq: u64,
        session_id: String,
        change: DebugBreakpointChangeKind,
        breakpoint: DebugBreakpointView,
    },
    ModuleLoaded {
        seq: u64,
        session_id: String,
        module: DebugModuleView,
    },
    ThreadStarted {
        seq: u64,
        session_id: String,
        thread_id: u32,
    },
}

impl DebugEvent {
    pub fn seq(&self) -> u64 {
        match self {
            Self::Stopped { seq, .. }
            | Self::Output { seq, .. }
            | Self::Continued { seq, .. }
            | Self::Exited { seq, .. }
            | Self::BreakpointChanged { seq, .. }
            | Self::ModuleLoaded { seq, .. }
            | Self::ThreadStarted { seq, .. } => *seq,
        }
    }

    pub fn session_id(&self) -> &str {
        match self {
            Self::Stopped { session_id, .. }
            | Self::Output { session_id, .. }
            | Self::Continued { session_id, .. }
            | Self::Exited { session_id, .. }
            | Self::BreakpointChanged { session_id, .. }
            | Self::ModuleLoaded { session_id, .. }
            | Self::ThreadStarted { session_id, .. } => session_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugEventLag {
    pub dropped: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DebugEventDelivery {
    Event(DebugEvent),
    Lag(DebugEventLag),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugEventRecvError {
    Empty,
    Disconnected,
}

#[derive(Debug)]
struct EventQueueState {
    events: VecDeque<DebugEvent>,
    dropped: usize,
    disconnected: bool,
}

#[derive(Debug)]
struct EventQueue {
    mode: DebugEventChannelMode,
    state: Mutex<EventQueueState>,
    available: Condvar,
}

impl EventQueue {
    fn new(mode: DebugEventChannelMode) -> Self {
        Self {
            mode,
            state: Mutex::new(EventQueueState {
                events: VecDeque::new(),
                dropped: 0,
                disconnected: false,
            }),
            available: Condvar::new(),
        }
    }

    fn push(&self, event: DebugEvent) {
        let mut state = self.state.lock().expect("debug event queue poisoned");
        match self.mode {
            DebugEventChannelMode::Bounded(0) => {
                state.dropped += 1;
            }
            DebugEventChannelMode::Bounded(capacity) => {
                while state.events.len() >= capacity {
                    state.events.pop_front();
                    state.dropped += 1;
                }
                state.events.push_back(event);
            }
            DebugEventChannelMode::Unbounded => state.events.push_back(event),
        }
        self.available.notify_all();
    }

    fn disconnect(&self) {
        let mut state = self.state.lock().expect("debug event queue poisoned");
        state.disconnected = true;
        self.available.notify_all();
    }

    fn recv_delivery(&self) -> Result<DebugEventDelivery, DebugEventRecvError> {
        let mut state = self.state.lock().expect("debug event queue poisoned");
        loop {
            if state.dropped > 0 {
                let dropped = std::mem::take(&mut state.dropped);
                return Ok(DebugEventDelivery::Lag(DebugEventLag { dropped }));
            }
            if let Some(event) = state.events.pop_front() {
                return Ok(DebugEventDelivery::Event(event));
            }
            if state.disconnected {
                return Err(DebugEventRecvError::Disconnected);
            }
            state = self
                .available
                .wait(state)
                .expect("debug event queue poisoned");
        }
    }

    fn try_recv_delivery(&self) -> Result<DebugEventDelivery, DebugEventRecvError> {
        let mut state = self.state.lock().expect("debug event queue poisoned");
        if state.dropped > 0 {
            let dropped = std::mem::take(&mut state.dropped);
            return Ok(DebugEventDelivery::Lag(DebugEventLag { dropped }));
        }
        if let Some(event) = state.events.pop_front() {
            return Ok(DebugEventDelivery::Event(event));
        }
        if state.disconnected {
            Err(DebugEventRecvError::Disconnected)
        } else {
            Err(DebugEventRecvError::Empty)
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DebugEventHub {
    mode: DebugEventChannelMode,
    subscribers: Arc<Mutex<Vec<Weak<EventQueue>>>>,
}

impl DebugEventHub {
    pub(crate) fn new(mode: DebugEventChannelMode) -> Self {
        Self {
            mode,
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn subscribe(&self) -> DebugEventReceiver {
        let queue = Arc::new(EventQueue::new(self.mode));
        self.subscribers
            .lock()
            .expect("debug event subscriber list poisoned")
            .push(Arc::downgrade(&queue));
        DebugEventReceiver { inner: queue }
    }

    pub(crate) fn publish(&self, event: DebugEvent) {
        let mut subscribers = self
            .subscribers
            .lock()
            .expect("debug event subscriber list poisoned");
        subscribers.retain(|subscriber| {
            if let Some(queue) = subscriber.upgrade() {
                queue.push(event.clone());
                true
            } else {
                false
            }
        });
    }

    pub(crate) fn close(&self) {
        let mut subscribers = self
            .subscribers
            .lock()
            .expect("debug event subscriber list poisoned");
        subscribers.retain(|subscriber| {
            if let Some(queue) = subscriber.upgrade() {
                queue.disconnect();
                true
            } else {
                false
            }
        });
    }
}

/// Sync event receiver wrapper.
#[derive(Debug, Clone)]
pub struct DebugEventReceiver {
    inner: Arc<EventQueue>,
}

impl DebugEventReceiver {
    pub fn recv_delivery(&self) -> Result<DebugEventDelivery, DebugEventRecvError> {
        self.inner.recv_delivery()
    }

    pub fn try_recv_delivery(&self) -> Result<DebugEventDelivery, DebugEventRecvError> {
        self.inner.try_recv_delivery()
    }

    pub fn recv(&self) -> Result<DebugEvent, DebugEventRecvError> {
        loop {
            match self.recv_delivery()? {
                DebugEventDelivery::Event(event) => return Ok(event),
                DebugEventDelivery::Lag(_) => continue,
            }
        }
    }

    #[cfg(feature = "tokio")]
    pub async fn recv_async(&self) -> Result<DebugEvent, DebugEventRecvError> {
        let receiver = self.clone();
        tokio::task::spawn_blocking(move || receiver.recv())
            .await
            .map_err(|_| DebugEventRecvError::Disconnected)?
    }

    #[cfg(feature = "tokio")]
    pub async fn recv_delivery_async(&self) -> Result<DebugEventDelivery, DebugEventRecvError> {
        let receiver = self.clone();
        tokio::task::spawn_blocking(move || receiver.recv_delivery())
            .await
            .map_err(|_| DebugEventRecvError::Disconnected)?
    }

    pub fn try_recv(&self) -> Result<DebugEvent, TryRecvError> {
        loop {
            match self.try_recv_delivery() {
                Ok(DebugEventDelivery::Event(event)) => return Ok(event),
                Ok(DebugEventDelivery::Lag(_)) => continue,
                Err(DebugEventRecvError::Empty) => return Err(TryRecvError::Empty),
                Err(DebugEventRecvError::Disconnected) => return Err(TryRecvError::Disconnected),
            }
        }
    }
}
