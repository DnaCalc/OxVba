//! JitContext: execution context passed to all JIT-compiled code.
//!
//! The JIT function signature is `extern "C" fn(*mut JitContext) -> i32`.
//! Runtime helper functions receive a pointer to this struct and manipulate
//! slots, error state, and host services through it.

use std::collections::HashMap;
use std::sync::Arc;

use oxvba_compiler::bytecode::ExternalCallDescriptor;
use oxvba_hal::traits::HostServices;
use oxvba_runtime::{ObjectHandle, RuntimeValue};

use crate::slot_abi::{RtSlot, rtslot_from_runtime_value};

// ── JitHostState: Rust-side state accessed only through helpers ────────

/// Iterator state for WithEvents owner enumeration.
#[derive(Debug, Default, Clone)]
pub struct WithEventsOwnerIterator {
    pub owners: Vec<ObjectHandle>,
    pub next_index: usize,
}

/// Interpreter-equivalent state for JIT runtime helpers.
///
/// This struct is **not** `#[repr(C)]` — it is only accessed from Rust
/// helper functions via an opaque pointer in JitContext.
pub struct JitHostState {
    pub withevents_bindings: HashMap<i64, RuntimeValue>,
    pub withevents_owner_iters: Vec<WithEventsOwnerIterator>,
    /// Raw pointer into the inlined Bytecode's external_call_descriptors.
    /// Valid for the duration of JIT execution.
    external_call_descriptors_ptr: *const ExternalCallDescriptor,
    external_call_descriptors_len: usize,
}

impl JitHostState {
    /// Access the external call descriptors slice.
    ///
    /// # Safety
    /// The pointer must still be valid (inlined bytecode must be alive).
    pub unsafe fn external_call_descriptors(&self) -> &[ExternalCallDescriptor] {
        if self.external_call_descriptors_ptr.is_null() || self.external_call_descriptors_len == 0 {
            &[]
        } else {
            unsafe {
                std::slice::from_raw_parts(
                    self.external_call_descriptors_ptr,
                    self.external_call_descriptors_len,
                )
            }
        }
    }
}

// ── JitContext: #[repr(C)] ABI boundary ───────────────────────────────

/// Execution context for JIT-compiled VBA code.
///
/// All fields are laid out for C-ABI access. The JIT function receives
/// a pointer to this struct as its sole argument.
#[repr(C)]
pub struct JitContext {
    /// Pointer to the slot array. Cranelift accesses this directly.
    pub slots_ptr: *mut RtSlot,
    /// Number of allocated slots.
    pub slot_count: u32,
    /// Number of user-visible variable slots (for result extraction).
    pub user_slot_count: u32,
    /// On Error Resume Next flag (0 = off, 1 = on).
    pub on_error_resume_next: u32,
    /// On Error GoTo target PC (-1 = no handler).
    pub on_error_goto_target: i32,
    /// Last error code (Err.Number).
    pub last_error: i32,
    /// PC where the last error occurred (-1 = no error).
    pub last_error_pc: i32,
    /// Random number generator state.
    pub rnd_state: u32,
    /// Pointer to a Box<Arc<dyn HostServices>> (heap-stable, for runtime helpers).
    host_services_box_ptr: *const (),
    /// Pointer to a Box<JitHostState> (heap-stable, for runtime helpers).
    host_state_ptr: *const (),
}

/// Owned wrapper that manages the JitContext lifetime and slot storage.
pub struct JitContextOwned {
    /// The actual slot storage (kept alive for the duration of JIT execution).
    slots: Vec<RtSlot>,
    /// The context struct (slots_ptr points into `self.slots`).
    pub context: JitContext,
    /// Boxed Arc keeps the host services alive and provides a stable address.
    _host_services_box: Box<Arc<dyn HostServices>>,
    /// Boxed host state keeps the JitHostState alive and provides a stable address.
    _host_state_box: Box<JitHostState>,
}

impl JitContextOwned {
    /// Create a new JitContext with `slot_count` slots, all initialized to Empty.
    pub fn new(
        slot_count: usize,
        user_slot_count: usize,
        host_services: Arc<dyn HostServices>,
        external_call_descriptors: &[ExternalCallDescriptor],
    ) -> Self {
        let slot_count = slot_count.max(1);
        let mut slots = vec![RtSlot::default(); slot_count];
        let slots_ptr = slots.as_mut_ptr();

        // Box the Arc so its address is heap-stable.
        let host_services_box = Box::new(host_services);
        let host_services_box_ptr =
            &*host_services_box as *const Arc<dyn HostServices> as *const ();

        // Create the host state with borrowed descriptor pointer.
        let host_state = Box::new(JitHostState {
            withevents_bindings: HashMap::new(),
            withevents_owner_iters: Vec::new(),
            external_call_descriptors_ptr: if external_call_descriptors.is_empty() {
                std::ptr::null()
            } else {
                external_call_descriptors.as_ptr()
            },
            external_call_descriptors_len: external_call_descriptors.len(),
        });
        let host_state_ptr = &*host_state as *const JitHostState as *const ();

        let context = JitContext {
            slots_ptr,
            slot_count: slot_count as u32,
            user_slot_count: user_slot_count as u32,
            on_error_resume_next: 0,
            on_error_goto_target: -1,
            last_error: 0,
            last_error_pc: -1,
            rnd_state: 0,
            host_services_box_ptr,
            host_state_ptr,
        };

        JitContextOwned {
            slots,
            context,
            _host_services_box: host_services_box,
            _host_state_box: host_state,
        }
    }

    /// Extract the user-visible slot values as RuntimeValues.
    pub fn extract_user_values(&self) -> Vec<RuntimeValue> {
        let user_count = self.context.user_slot_count as usize;
        self.slots[..user_count]
            .iter()
            .map(|slot| unsafe { slot.to_runtime_value() })
            .collect()
    }

    /// Get a mutable pointer to the JitContext (for passing to JIT code).
    pub fn context_ptr(&mut self) -> *mut JitContext {
        // Re-establish slots_ptr in case Vec reallocated (it shouldn't after creation).
        self.context.slots_ptr = self.slots.as_mut_ptr();
        &mut self.context as *mut JitContext
    }
}

impl Drop for JitContextOwned {
    fn drop(&mut self) {
        // Drop all heap-allocated slot data.
        for slot in &mut self.slots {
            if slot.is_heap_type() {
                unsafe { slot.drop_heap() };
            }
        }
    }
}

// ── Helper access functions (called from runtime helpers) ─────────────

impl JitContext {
    /// Read a RuntimeValue from a slot.
    ///
    /// # Safety
    /// The slot index must be within bounds, and slots_ptr must be valid.
    pub unsafe fn read_slot(&self, slot: u32) -> RuntimeValue {
        debug_assert!(!self.slots_ptr.is_null(), "JitContext::read_slot: null slots_ptr");
        debug_assert!(slot < self.slot_count, "JitContext::read_slot: slot {} >= count {}", slot, self.slot_count);
        let rt_slot = unsafe { &*self.slots_ptr.add(slot as usize) };
        unsafe { rt_slot.to_runtime_value() }
    }

    /// Write a RuntimeValue to a slot.
    ///
    /// # Safety
    /// The slot index must be within bounds, and slots_ptr must be valid.
    pub unsafe fn write_slot(&mut self, slot: u32, value: RuntimeValue) {
        debug_assert!(!self.slots_ptr.is_null(), "JitContext::write_slot: null slots_ptr");
        debug_assert!(slot < self.slot_count, "JitContext::write_slot: slot {} >= count {}", slot, self.slot_count);
        let rt_slot = unsafe { &mut *self.slots_ptr.add(slot as usize) };
        // Drop old heap data if present.
        if rt_slot.is_heap_type() {
            unsafe { rt_slot.drop_heap() };
        }
        *rt_slot = rtslot_from_runtime_value(&value);
    }

    /// Get a reference to the host services Arc.
    ///
    /// # Safety
    /// host_services_box_ptr must be a valid pointer to a Box<Arc<dyn HostServices>>.
    pub unsafe fn host_services(&self) -> &Arc<dyn HostServices> {
        debug_assert!(
            !self.host_services_box_ptr.is_null(),
            "JitContext::host_services: null host_services_box_ptr"
        );
        unsafe { &*(self.host_services_box_ptr as *const Arc<dyn HostServices>) }
    }

    /// Get a reference to the JitHostState.
    ///
    /// # Safety
    /// host_state_ptr must be a valid pointer to a JitHostState.
    pub unsafe fn host_state(&self) -> &JitHostState {
        debug_assert!(
            !self.host_state_ptr.is_null(),
            "JitContext::host_state: null host_state_ptr"
        );
        unsafe { &*(self.host_state_ptr as *const JitHostState) }
    }

    /// Get a mutable reference to the JitHostState.
    ///
    /// # Safety
    /// host_state_ptr must be a valid pointer to a JitHostState,
    /// and no other references to it must exist.
    pub unsafe fn host_state_mut(&mut self) -> &mut JitHostState {
        debug_assert!(
            !self.host_state_ptr.is_null(),
            "JitContext::host_state_mut: null host_state_ptr"
        );
        unsafe { &mut *(self.host_state_ptr as *mut JitHostState) }
    }
}
