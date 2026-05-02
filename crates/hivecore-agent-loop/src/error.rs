use hivecore_runtime_core::RuntimeError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoopError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error("hook aborted: {0}")]
    HookAborted(String),
    #[error("manual attention required: {0}")]
    ManualAttention(String),
    #[error("max iterations exceeded ({0})")]
    MaxIterations(usize),
}
