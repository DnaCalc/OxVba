use std::{marker::PhantomData, rc::Rc, sync::Arc};

use oxvba_compiler::ProjectManifest;
use oxvba_host::Engine;

use crate::config::DebugCoreConfig;

/// Raw, stateful debugger core.
///
/// B03 moves the existing `oxvba-host` debugger implementation behind this
/// type. The private `Rc` marker makes the skeleton explicitly `!Send` and
/// `!Sync`, matching the worker-owned architecture before the core move lands.
pub struct DebugSessionCore {
    engine: Arc<Engine>,
    manifest: ProjectManifest,
    config: DebugCoreConfig,
    not_send_sync: PhantomData<Rc<()>>,
}

impl DebugSessionCore {
    pub fn engine(&self) -> &Arc<Engine> {
        &self.engine
    }

    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    pub fn config(&self) -> &DebugCoreConfig {
        &self.config
    }
}

/// Raw-core run result placeholder; B03 maps current host debugger results here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugCoreRunResult {
    Paused,
    Exited { exit_code: Option<i32> },
}
