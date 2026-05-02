//! Integration test: every public trait must remain object-safe so the
//! runtime can hold them as `Arc<dyn _>`. Pure compile-time check.

use std::sync::Arc;

use hivecore_runtime_core::{ContextTransform, ModelAdapter, Tool, ToolHook};

#[allow(dead_code)]
fn assert_object_safe(
    _t: Arc<dyn Tool>,
    _h: Arc<dyn ToolHook>,
    _c: Arc<dyn ContextTransform>,
    _m: Arc<dyn ModelAdapter>,
) {
}
