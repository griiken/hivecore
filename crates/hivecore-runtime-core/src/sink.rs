//! `EventSink` — the pure subscriber contract. Concrete impls (file-backed
//! audit, websocket bridge, KG ingest) live in higher layers, but the trait
//! is I/O-free and belongs at Layer 1 so any consumer can speak it.
//!
//! Sinks are infallible by contract — a broken sink must not stall the
//! runtime. Implementors are expected to log + swallow internal errors.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::event::AgentEvent;

#[async_trait]
pub trait EventSink: Send + Sync {
    async fn emit(&self, event: AgentEvent);
}

#[derive(Debug, Default, Clone)]
pub struct NoopSink;

#[async_trait]
impl EventSink for NoopSink {
    async fn emit(&self, _event: AgentEvent) {}
}

/// Buffers all events into an in-memory `Vec`. Test-friendly.
#[derive(Debug, Default, Clone)]
pub struct VecSink {
    inner: Arc<Mutex<Vec<AgentEvent>>>,
}

impl VecSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> Vec<AgentEvent> {
        self.inner.lock().expect("vec sink poisoned").clone()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("vec sink poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().expect("vec sink poisoned").is_empty()
    }
}

#[async_trait]
impl EventSink for VecSink {
    async fn emit(&self, event: AgentEvent) {
        self.inner.lock().expect("vec sink poisoned").push(event);
    }
}

/// Fans events out to multiple sinks in registration order.
#[derive(Clone)]
pub struct FanOutSink {
    sinks: Vec<Arc<dyn EventSink>>,
}

impl FanOutSink {
    pub fn new(sinks: Vec<Arc<dyn EventSink>>) -> Self {
        Self { sinks }
    }
}

#[async_trait]
impl EventSink for FanOutSink {
    async fn emit(&self, event: AgentEvent) {
        for sink in &self.sinks {
            sink.emit(event.clone()).await;
        }
    }
}

impl std::fmt::Debug for FanOutSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FanOutSink")
            .field("sinks", &self.sinks.len())
            .finish()
    }
}
