//! `wit-bindgen`-style host bindings, hand-written.
//!
//! We don't pull `wit-bindgen` as a build dep for the host — wasmtime's
//! `bindgen!` macro handles host-side codegen. This module is the single
//! place that macro lives so call sites elsewhere (`loader.rs`,
//! `tool_adapter.rs`) import named types instead of generated paths.

use wasmtime::component::bindgen;

bindgen!({
    world: "extension",
    path: "wit/since_v0.1.0",
    async: false,
    with: {
        // Map host imports to our `host.rs` types.
    },
    trappable_imports: true,
});

pub use hivecore::extension::host::LogLevel;
