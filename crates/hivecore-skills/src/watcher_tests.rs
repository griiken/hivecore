use std::time::Duration;

use tempfile::TempDir;
use tokio::sync::RwLock;

use super::*;

const VALID: &str = "---
name: alpha
description: alpha skill
---
body
";

const VALID_BETA: &str = "---
name: beta
description: beta skill
---
body
";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn picks_up_new_skill_file() {
    let d = TempDir::new().unwrap();
    let shared: SharedSkillRegistry = Arc::new(RwLock::new(SkillRegistry::new()));

    let _watcher = SkillWatcher::start(
        vec![d.path().to_path_buf()],
        shared.clone(),
        Duration::from_millis(50),
    )
    .unwrap();

    // Initially empty.
    assert_eq!(shared.read().await.len(), 0);

    // Drop a skill file — watcher should fire and reload.
    std::fs::write(d.path().join("alpha.md"), VALID).unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(shared.read().await.len(), 1);

    // Add another.
    std::fs::write(d.path().join("beta.md"), VALID_BETA).unwrap();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(shared.read().await.len(), 2);
}
