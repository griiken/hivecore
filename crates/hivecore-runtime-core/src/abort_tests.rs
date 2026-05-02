use super::*;

#[tokio::test]
async fn signal_starts_unaborted() {
    let (_handle, sig) = AbortSignal::new();
    assert!(!sig.is_aborted());
}

#[tokio::test]
async fn handle_flips_signal() {
    let (handle, sig) = AbortSignal::new();
    handle.abort();
    assert!(sig.is_aborted());
}

#[tokio::test]
async fn cloned_signals_share_state() {
    let (handle, sig) = AbortSignal::new();
    let sig2 = sig.clone();
    handle.abort();
    assert!(sig.is_aborted());
    assert!(sig2.is_aborted());
}
