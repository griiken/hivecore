//! `hivecore-browser-runtime` — Layer 3 default `BrowserProvider` impl.
//!
//! ADR-028. Wraps `chromiumoxide` (CDP) into a `BrowserProvider` trait
//! impl. Per-(tenant, session) headless-Chrome process; refs versioned
//! per snapshot; tenant-scoped artifact directory.
//!
//! Patterns adopted from `gsd-build/gsd-browser` (Apache-2.0); see ADR-024
//! vendoring rules. This crate does NOT vendor source verbatim — patterns
//! only.

#![deny(missing_debug_implementations)]
#![warn(rust_2018_idioms, unreachable_pub)]

pub mod chrome_provider;
pub mod settle;

pub use chrome_provider::{ChromeConfig, ChromeProvider};
pub use settle::{settle_after_action, SettleOptions, SettleResult};
