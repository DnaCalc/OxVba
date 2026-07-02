//! Thread-local native callback thunks for `AddressOf` procedure references.
//!
//! The runtime owns only the ABI trampoline table and an opaque executor trait.
//! The VM supplies the executor implementation and decides how a proc token maps
//! to executable VBA code.

use std::{
    cell::RefCell,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr::NonNull,
};

const MAX_CALLBACK_SLOTS: usize = 32;

/// Executes a registered VBA callback token.
pub trait CallbackExecutor {
    /// Invoke `proc_token` with the raw pointer-sized callback arguments.
    ///
    /// The caller has already crossed the native ABI boundary; implementations
    /// must not panic across this call.
    fn invoke_callback(&mut self, proc_token: usize, args: &[isize]) -> isize;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackThunkError {
    UnsupportedPlatform,
    Exhausted,
}

impl fmt::Display for CallbackThunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallbackThunkError::UnsupportedPlatform => {
                write!(f, "native callback thunks are unsupported on this platform")
            }
            CallbackThunkError::Exhausted => write!(f, "native callback thunk table exhausted"),
        }
    }
}

#[derive(Clone, Copy)]
struct CallbackSlot {
    owner: usize,
    proc_token: usize,
    executor: NonNull<()>,
    invoke: unsafe fn(NonNull<()>, usize, &[isize]) -> isize,
    ref_count: u32,
}

thread_local! {
    static CALLBACK_SLOTS: RefCell<[Option<CallbackSlot>; MAX_CALLBACK_SLOTS]> =
        const { RefCell::new([None; MAX_CALLBACK_SLOTS]) };
}

/// A scoped native callback registration. Dropping it releases its thunk slot.
#[derive(Debug)]
pub struct CallbackRegistration {
    slot: usize,
    owner: usize,
    proc_token: usize,
    address: usize,
}

impl CallbackRegistration {
    pub fn address(&self) -> usize {
        self.address
    }
}

impl Drop for CallbackRegistration {
    fn drop(&mut self) {
        CALLBACK_SLOTS.with(|slots| {
            let mut slots = slots.borrow_mut();
            let Some(entry) = &mut slots[self.slot] else {
                return;
            };
            if entry.owner != self.owner || entry.proc_token != self.proc_token {
                return;
            }
            entry.ref_count = entry.ref_count.saturating_sub(1);
            if entry.ref_count == 0 {
                slots[self.slot] = None;
            }
        });
    }
}

/// Register a callback executor for a single `AddressOf` proc token.
///
/// # Safety
/// `executor` must remain valid, uniquely mutable for callback execution, and on
/// the same thread until every returned [`CallbackRegistration`] has been
/// dropped. This mirrors the synchronous VBA callback contract used by
/// `CallWindowProc`-style probes.
pub unsafe fn register_callback<T: CallbackExecutor>(
    owner: usize,
    proc_token: usize,
    executor: NonNull<T>,
) -> Result<CallbackRegistration, CallbackThunkError> {
    unsafe fn invoke_executor<T: CallbackExecutor>(
        executor: NonNull<()>,
        proc_token: usize,
        args: &[isize],
    ) -> isize {
        // SAFETY: register_callback's caller guarantees the executor pointer remains
        // valid and uniquely mutable while the scoped registration is alive.
        unsafe {
            executor
                .cast::<T>()
                .as_mut()
                .invoke_callback(proc_token, args)
        }
    }

    let (slot, address) = CALLBACK_SLOTS
        .with(|slots| {
            let mut slots = slots.borrow_mut();
            for (index, entry) in slots.iter_mut().enumerate() {
                if let Some(entry) = entry
                    && entry.owner == owner
                    && entry.proc_token == proc_token
                {
                    let address = callback_address(index)?;
                    entry.ref_count = entry.ref_count.saturating_add(1);
                    return Ok(Some((index, address)));
                }
            }
            for (index, entry) in slots.iter_mut().enumerate() {
                if entry.is_none() {
                    let address = callback_address(index)?;
                    *entry = Some(CallbackSlot {
                        owner,
                        proc_token,
                        executor: executor.cast(),
                        invoke: invoke_executor::<T>,
                        ref_count: 1,
                    });
                    return Ok(Some((index, address)));
                }
            }
            Ok(None)
        })?
        .ok_or(CallbackThunkError::Exhausted)?;
    Ok(CallbackRegistration {
        slot,
        owner,
        proc_token,
        address,
    })
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn callback_address(slot: usize) -> Result<usize, CallbackThunkError> {
    THUNKS
        .get(slot)
        .map(|thunk| *thunk as usize)
        .ok_or(CallbackThunkError::Exhausted)
}

#[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
fn callback_address(_slot: usize) -> Result<usize, CallbackThunkError> {
    Err(CallbackThunkError::UnsupportedPlatform)
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn invoke_slot(slot: usize, args: [isize; 4]) -> isize {
    let entry = CALLBACK_SLOTS.with(|slots| slots.borrow().get(slot).copied().flatten());
    let Some(entry) = entry else {
        return 0;
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: register_callback's caller guarantees the executor remains
        // valid and uniquely mutable while the scoped registration is alive.
        unsafe { (entry.invoke)(entry.executor, entry.proc_token, &args) }
    }));
    result.unwrap_or(0)
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
macro_rules! define_callback_thunks {
    ($($name:ident => $slot:expr),+ $(,)?) => {
        $(
            extern "system" fn $name(a0: isize, a1: u32, a2: usize, a3: isize) -> isize {
                invoke_slot($slot, [a0, a1 as isize, a2 as isize, a3])
            }
        )+

        type CallbackThunk = extern "system" fn(isize, u32, usize, isize) -> isize;

        const THUNKS: [CallbackThunk; MAX_CALLBACK_SLOTS] = [
            $($name),+
        ];
    };
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
define_callback_thunks! {
    thunk_00 => 0, thunk_01 => 1, thunk_02 => 2, thunk_03 => 3,
    thunk_04 => 4, thunk_05 => 5, thunk_06 => 6, thunk_07 => 7,
    thunk_08 => 8, thunk_09 => 9, thunk_10 => 10, thunk_11 => 11,
    thunk_12 => 12, thunk_13 => 13, thunk_14 => 14, thunk_15 => 15,
    thunk_16 => 16, thunk_17 => 17, thunk_18 => 18, thunk_19 => 19,
    thunk_20 => 20, thunk_21 => 21, thunk_22 => 22, thunk_23 => 23,
    thunk_24 => 24, thunk_25 => 25, thunk_26 => 26, thunk_27 => 27,
    thunk_28 => 28, thunk_29 => 29, thunk_30 => 30, thunk_31 => 31,
}
