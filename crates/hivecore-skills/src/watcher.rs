//! Live skill reloader. Spawns a debounced filesystem watcher; on change,
//! reloads every directory the loader knows about and replaces the registry
//! atomically. Callers pass a `SharedSkillRegistry` (Arc<RwLock<...>>) — the
//! same handle they hand to their `ToolRegistry` builder — and this module
//! mutates it in place.
//!
//! Why a separate file: keeps the runtime-free `loader` / `registry` modules
//! free of `notify` so they stay portable to wasm-targets if anyone wants
//! that later (the loader can be reused server-side without dragging in
//! inotify).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use tokio::sync::RwLock;

use crate::error::SkillError;
use crate::loader::SkillLoader;
use crate::registry::SkillRegistry;

pub type SharedSkillRegistry = Arc<RwLock<SkillRegistry>>;

#[derive(Debug)]
pub struct SkillWatcher {
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

impl SkillWatcher {
    /// Watch `paths` and replace the registry inside `shared` on any change.
    /// `debounce` collapses bursty filesystem events (e.g. an editor's
    /// rename-and-truncate save sequence) into a single reload.
    pub fn start(
        paths: Vec<PathBuf>,
        shared: SharedSkillRegistry,
        debounce: Duration,
    ) -> Result<Self, SkillError> {
        let watch_paths = paths.clone();
        let handle = tokio::runtime::Handle::current();

        let mut debouncer = new_debouncer(debounce, move |res: DebounceEventResult| match res {
            Ok(events) if !events.is_empty() => {
                let shared = shared.clone();
                let paths = paths.clone();
                handle.spawn(async move {
                    if let Err(e) = reload(&paths, &shared).await {
                        tracing::warn!(error = %e, "skill reload failed");
                    }
                });
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "skill watcher error");
            }
        })
        .map_err(|e| SkillError::Io(std::io::Error::other(format!("watcher: {e}"))))?;

        for p in &watch_paths {
            if p.exists() {
                debouncer
                    .watcher()
                    .watch(p, RecursiveMode::Recursive)
                    .map_err(|e| {
                        SkillError::Io(std::io::Error::other(format!("watch {p:?}: {e}")))
                    })?;
            }
        }

        Ok(Self {
            _debouncer: debouncer,
        })
    }
}

async fn reload(paths: &[PathBuf], shared: &SharedSkillRegistry) -> Result<(), SkillError> {
    let loader = paths
        .iter()
        .fold(SkillLoader::new(), |l, p| l.add_dir(p.clone()));
    let skills = loader.load()?;
    let new = SkillRegistry::from_skills(skills)?;
    *shared.write().await = new;
    tracing::info!("skills reloaded");
    Ok(())
}

#[cfg(test)]
#[path = "watcher_tests.rs"]
mod tests;
