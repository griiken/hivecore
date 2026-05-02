//! Per-extension host state. Carries the WASI context plus our own
//! capability surface (settings, log dispatch). The wasmtime `Store` holds
//! one of these for the lifetime of an extension instance.

use std::sync::Arc;

use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};

/// Settings backend the host exposes to extensions via the `get-setting`
/// import. Multi-tenant deployments wrap the real settings store in a
/// `TenantScoped` adapter so extensions cannot read keys outside their
/// tenant. v0.1 single-tenant uses `InMemorySettings` directly.
pub trait SettingsProvider: Send + Sync + std::fmt::Debug {
    fn get(&self, key: &str) -> Option<String>;
}

#[derive(Debug, Default, Clone)]
pub struct InMemorySettings {
    inner: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
}

impl InMemorySettings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, key: impl Into<String>, value: impl Into<String>) {
        self.inner.lock().unwrap().insert(key.into(), value.into());
    }
}

impl SettingsProvider for InMemorySettings {
    fn get(&self, key: &str) -> Option<String> {
        self.inner.lock().unwrap().get(key).cloned()
    }
}

pub struct HostState {
    pub wasi: WasiCtx,
    pub table: ResourceTable,
    pub settings: Arc<dyn SettingsProvider>,
    pub extension_id: String,
}

impl std::fmt::Debug for HostState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostState")
            .field("extension_id", &self.extension_id)
            .field("settings", &self.settings)
            .finish()
    }
}

impl HostState {
    pub fn new(extension_id: impl Into<String>, settings: Arc<dyn SettingsProvider>) -> Self {
        let wasi = WasiCtxBuilder::new()
            .inherit_stderr() // routed through wasmtime to our process stderr
            .build();
        Self {
            wasi,
            table: ResourceTable::new(),
            settings,
            extension_id: extension_id.into(),
        }
    }
}

impl WasiView for HostState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

// -- Host imports impl ----------------------------------------------------

use crate::bindings::hivecore::extension::host::{Host as HostImports, LogLevel};

impl HostImports for HostState {
    fn log(&mut self, level: LogLevel, message: String) -> wasmtime::Result<()> {
        let target = format!("ext::{}", self.extension_id);
        match level {
            LogLevel::Trace => tracing::trace!(target: "ext", id = %self.extension_id, "{message}"),
            LogLevel::Debug => tracing::debug!(target: "ext", id = %self.extension_id, "{message}"),
            LogLevel::Info => tracing::info!(target: "ext", id = %self.extension_id, "{message}"),
            LogLevel::Warn => tracing::warn!(target: "ext", id = %self.extension_id, "{message}"),
            LogLevel::Error => tracing::error!(target: "ext", id = %self.extension_id, "{message}"),
        }
        let _ = target; // silence unused if tracing target macro changes
        Ok(())
    }

    fn get_setting(&mut self, key: String) -> wasmtime::Result<Option<String>> {
        Ok(self.settings.get(&key))
    }
}
