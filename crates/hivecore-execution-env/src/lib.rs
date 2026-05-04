//! Default `ExecutionEnv` impls (ADR-031).
//!
//! `LocalEnv` shells out to the host OS via `tokio::fs` + `tokio::process`.
//! Future sandbox providers (Firecracker, Docker, remote, leased runners)
//! ship their own crates implementing the same trait from
//! `hivecore-runtime-core::execution_env`.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod local;

pub use local::LocalEnv;
