//! Local-OS `ExecutionEnv` impl. Default for `hivecore-coder` and
//! `hivecore-acp-server` when no sandbox is wired.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use hivecore_runtime_core::{
    ExecOpts, ExecOutput, ExecutionEnv, FileStat, RemoveOpts, RuntimeError, RuntimeResult,
};
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub struct LocalEnv {
    cwd: PathBuf,
}

impl LocalEnv {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self { cwd: cwd.into() }
    }
}

fn io_err(e: std::io::Error) -> RuntimeError {
    RuntimeError::Other(format!("io: {e}"))
}

#[async_trait]
impl ExecutionEnv for LocalEnv {
    fn cwd(&self) -> &Path {
        &self.cwd
    }

    async fn exec(&self, command: &str, opts: ExecOpts) -> RuntimeResult<ExecOutput> {
        let cwd = opts.cwd.unwrap_or_else(|| self.cwd.clone());
        let mut cmd = Command::new("bash");
        cmd.arg("-lc")
            .arg(command)
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (k, v) in &opts.env {
            cmd.env(k, v);
        }

        if let Some(sig) = &opts.signal {
            if sig.is_aborted() {
                return Err(RuntimeError::Aborted);
            }
        }

        let dur = opts.timeout.unwrap_or(Duration::from_secs(120));
        let child = cmd.spawn().map_err(io_err)?;
        let output = match timeout(dur, child.wait_with_output()).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(io_err(e)),
            Err(_) => {
                return Err(RuntimeError::Other(format!(
                    "exec timeout after {} ms",
                    dur.as_millis()
                )));
            }
        };

        Ok(ExecOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    async fn read_file(&self, path: &Path) -> RuntimeResult<Vec<u8>> {
        fs::read(path).await.map_err(io_err)
    }

    async fn read_text_file(&self, path: &Path) -> RuntimeResult<String> {
        fs::read_to_string(path).await.map_err(io_err)
    }

    async fn write_file(&self, path: &Path, bytes: &[u8]) -> RuntimeResult<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).await.map_err(io_err)?;
            }
        }
        fs::write(path, bytes).await.map_err(io_err)
    }

    async fn stat(&self, path: &Path) -> RuntimeResult<FileStat> {
        let m = fs::symlink_metadata(path).await.map_err(io_err)?;
        let mtime = m.modified().map_err(io_err)?;
        Ok(FileStat {
            is_file: m.is_file(),
            is_dir: m.is_dir(),
            is_symlink: m.file_type().is_symlink(),
            size: m.len(),
            mtime,
        })
    }

    async fn list_dir(&self, path: &Path) -> RuntimeResult<Vec<PathBuf>> {
        let mut rd = fs::read_dir(path).await.map_err(io_err)?;
        let mut out = Vec::new();
        while let Some(entry) = rd.next_entry().await.map_err(io_err)? {
            out.push(entry.path());
        }
        out.sort();
        Ok(out)
    }

    async fn path_exists(&self, path: &Path) -> RuntimeResult<bool> {
        match fs::symlink_metadata(path).await {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_err(e)),
        }
    }

    async fn create_dir(&self, path: &Path, recursive: bool) -> RuntimeResult<()> {
        if recursive {
            fs::create_dir_all(path).await.map_err(io_err)
        } else {
            fs::create_dir(path).await.map_err(io_err)
        }
    }

    async fn remove(&self, path: &Path, opts: RemoveOpts) -> RuntimeResult<()> {
        let result = match fs::symlink_metadata(path).await {
            Ok(m) if m.is_dir() => {
                if opts.recursive {
                    fs::remove_dir_all(path).await
                } else {
                    fs::remove_dir(path).await
                }
            }
            Ok(_) => fs::remove_file(path).await,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if opts.force {
                    return Ok(());
                }
                Err(e)
            }
            Err(e) => Err(e),
        };
        match result {
            Ok(()) => Ok(()),
            Err(e) if opts.force && e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_err(e)),
        }
    }

    async fn create_temp_dir(&self, prefix: Option<&str>) -> RuntimeResult<PathBuf> {
        let prefix = prefix.unwrap_or("hivecore-");
        let suffix = uuid_v4_short();
        let p = std::env::temp_dir().join(format!("{prefix}{suffix}"));
        fs::create_dir_all(&p).await.map_err(io_err)?;
        Ok(p)
    }

    async fn create_temp_file(
        &self,
        prefix: Option<&str>,
        suffix: Option<&str>,
    ) -> RuntimeResult<PathBuf> {
        let prefix = prefix.unwrap_or("hivecore-");
        let suffix = suffix.unwrap_or("");
        let stamp = uuid_v4_short();
        let p = std::env::temp_dir().join(format!("{prefix}{stamp}{suffix}"));
        fs::write(&p, b"").await.map_err(io_err)?;
        Ok(p)
    }

    fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        }
    }

    async fn cleanup(&self) -> RuntimeResult<()> {
        Ok(())
    }
}

fn uuid_v4_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:016x}", nanos)
}

#[cfg(test)]
#[path = "local_tests.rs"]
mod tests;
