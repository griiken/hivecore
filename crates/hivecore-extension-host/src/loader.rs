//! Extension loader. Reads a `.wasm` Component file off disk, instantiates
//! it inside a wasmtime `Store<HostState>`, and exposes the
//! `list-tools` / `run-tool` / `init` exports.

use std::path::Path;
use std::sync::Arc;

use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};
use wasmtime_wasi::add_to_linker_sync;

use crate::bindings::{hivecore::extension::host as host_iface, Extension as Bindings, ToolSpec};
use crate::error::ExtensionError;
use crate::host::HostState;

#[derive(Clone)]
pub struct ExtensionLoader {
    engine: Engine,
}

impl std::fmt::Debug for ExtensionLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionLoader").finish()
    }
}

impl Default for ExtensionLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtensionLoader {
    pub fn new() -> Self {
        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true);
        Self {
            engine: Engine::new(&config).expect("wasmtime engine"),
        }
    }

    /// Load and instantiate an extension component.
    pub fn load(
        &self,
        path: impl AsRef<Path>,
        host: HostState,
    ) -> Result<Extension, ExtensionError> {
        let component =
            Component::from_file(&self.engine, path.as_ref()).map_err(ExtensionError::Wasmtime)?;

        let mut linker: Linker<HostState> = Linker::new(&self.engine);
        add_to_linker_sync(&mut linker).map_err(ExtensionError::Wasmtime)?;
        host_iface::add_to_linker(&mut linker, |state: &mut HostState| state)
            .map_err(ExtensionError::Wasmtime)?;

        let mut store = Store::new(&self.engine, host);
        let bindings = Bindings::instantiate(&mut store, &component, &linker)
            .map_err(ExtensionError::Wasmtime)?;

        // Lifecycle: call `init` once now.
        bindings
            .call_init(&mut store)
            .map_err(ExtensionError::Wasmtime)?;

        let specs = bindings
            .call_list_tools(&mut store)
            .map_err(ExtensionError::Wasmtime)?;
        Ok(Extension {
            store,
            bindings,
            specs: Arc::new(specs),
        })
    }
}

/// A loaded, instantiated extension. Cheap to clone the spec list, expensive
/// to clone the store — keep one `Extension` per `.wasm` file and share via
/// `Arc<Mutex<Extension>>` if multiple agent loops need to call into it.
pub struct Extension {
    pub(crate) store: Store<HostState>,
    pub(crate) bindings: Bindings,
    pub(crate) specs: Arc<Vec<ToolSpec>>,
}

impl std::fmt::Debug for Extension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extension")
            .field("tools", &self.specs.len())
            .finish()
    }
}

impl Extension {
    pub fn tools(&self) -> &[ToolSpec] {
        &self.specs
    }

    pub fn call_run_tool(
        &mut self,
        name: &str,
        args_json: &str,
    ) -> Result<crate::bindings::ToolResult, ExtensionError> {
        let res = self
            .bindings
            .call_run_tool(&mut self.store, name, args_json)
            .map_err(ExtensionError::Wasmtime)?;
        match res {
            Ok(t) => Ok(t),
            Err(msg) => Err(ExtensionError::ToolFailed(msg)),
        }
    }
}
