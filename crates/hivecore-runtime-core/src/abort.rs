//! Cooperative cancellation. Tools poll `AbortSignal::is_aborted` between
//! steps; the model adapter checks before each network read.

use tokio::sync::watch;

#[derive(Debug, Clone)]
pub struct AbortSignal {
    rx: watch::Receiver<bool>,
}

#[derive(Debug)]
pub struct AbortHandle {
    tx: watch::Sender<bool>,
}

impl AbortSignal {
    pub fn new() -> (AbortHandle, AbortSignal) {
        let (tx, rx) = watch::channel(false);
        (AbortHandle { tx }, AbortSignal { rx })
    }

    pub fn is_aborted(&self) -> bool {
        *self.rx.borrow()
    }
}

impl AbortHandle {
    pub fn abort(&self) {
        let _ = self.tx.send(true);
    }
}

#[cfg(test)]
#[path = "abort_tests.rs"]
mod tests;
