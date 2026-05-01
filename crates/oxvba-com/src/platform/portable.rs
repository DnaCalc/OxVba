//! Portable COM projection: universal host object registration API.
//!
//! Available on every platform (AP-5). On Windows, host-registered objects
//! coexist with real COM objects (portable registry is checked first).
//! On Linux/macOS, it's the only COM-like mechanism. On WASM, the JS host
//! registers objects into it.

use oxvba_runtime::{Variant, compat::RuntimeValue};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Factory for creating portable dispatch objects.
pub trait PortableObjectFactory: Send + Sync {
    fn create(&self) -> Box<dyn PortableDispatch>;
}

/// Late-bound dispatch interface for host-registered objects.
///
/// This portable host API carries retained [`Variant`] values. Legacy callers
/// that still need `RuntimeValue` projections must enter through
/// [`compat::RuntimeValueCompatPortableDispatch`].
pub trait PortableDispatch: Send + Sync {
    fn invoke(&self, member: &str, args: &[Variant]) -> Result<Variant, String>;
    fn get(&self, member: &str) -> Result<Variant, String>;
    fn put(&self, member: &str, value: Variant) -> Result<(), String>;
    fn member_names(&self) -> Vec<String>;
}

pub mod compat {
    use super::{PortableDispatch, RuntimeValue, Variant};

    /// Compatibility projection over the retained `Variant` portable dispatch
    /// contract for legacy host-registered objects.
    pub trait RuntimeValueCompatPortableDispatch {
        fn invoke_runtime_value(
            &self,
            member: &str,
            args: &[RuntimeValue],
        ) -> Result<RuntimeValue, String>;
        fn get_runtime_value(&self, member: &str) -> Result<RuntimeValue, String>;
        fn put_runtime_value(&self, member: &str, value: RuntimeValue) -> Result<(), String>;
    }

    impl<T> RuntimeValueCompatPortableDispatch for T
    where
        T: PortableDispatch + ?Sized,
    {
        fn invoke_runtime_value(
            &self,
            member: &str,
            args: &[RuntimeValue],
        ) -> Result<RuntimeValue, String> {
            let args = args
                .iter()
                .map(Variant::try_from_runtime_value)
                .collect::<Result<Vec<_>, _>>()?;
            self.invoke(member, &args)?.to_runtime_value()
        }

        fn get_runtime_value(&self, member: &str) -> Result<RuntimeValue, String> {
            self.get(member)?.to_runtime_value()
        }

        fn put_runtime_value(&self, member: &str, value: RuntimeValue) -> Result<(), String> {
            self.put(member, Variant::try_from_runtime_value(&value)?)
        }
    }
}

/// Registry of host-provided COM-like objects, keyed by ProgID.
#[derive(Default)]
pub struct PortableComProjection {
    registry: Mutex<HashMap<String, Arc<dyn PortableObjectFactory>>>,
}

impl PortableComProjection {
    pub fn new() -> Self {
        Self {
            registry: Mutex::new(HashMap::new()),
        }
    }

    /// Register an object factory for a ProgID. Any VBA `CreateObject()` call
    /// matching this ProgID will use this factory.
    pub fn register_object(&self, prog_id: &str, factory: Arc<dyn PortableObjectFactory>) {
        let mut reg = self.registry.lock().expect("portable registry lock");
        reg.insert(prog_id.to_string(), factory);
    }

    pub fn unregister_object(&self, prog_id: &str) {
        let mut reg = self.registry.lock().expect("portable registry lock");
        reg.remove(prog_id);
    }

    pub fn has_object(&self, prog_id: &str) -> bool {
        let reg = self.registry.lock().expect("portable registry lock");
        reg.contains_key(prog_id)
    }

    pub fn create_object(&self, prog_id: &str) -> Option<Box<dyn PortableDispatch>> {
        let reg = self.registry.lock().expect("portable registry lock");
        reg.get(prog_id).map(|factory| factory.create())
    }
}

impl std::fmt::Debug for PortableComProjection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.registry.lock().map(|r| r.len()).unwrap_or(0);
        f.debug_struct("PortableComProjection")
            .field("registered_count", &count)
            .finish()
    }
}

#[derive(Debug, Default)]
pub struct PortableComBridge;
