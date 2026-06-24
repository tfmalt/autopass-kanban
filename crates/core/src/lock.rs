//! Advisory file locking for read-modify-write sequences on backlog files.
//!
//! The kanban backlog is a markdown source of truth that can be mutated
//! concurrently by the CLI and the local web server. To prevent silent lost
//! updates, every mutation entry point acquires an exclusive advisory lock on
//! `.kanban/.lock` for the duration of its read-modify-write sequence
//! (US-013).
//!
//! Model: **per-repo, blocking with a timeout**. A single `.kanban/.lock` file
//! guards the whole backlog so cross-sprint operations like roster
//! regeneration are also protected. The lock blocks up to
//! [`DEFAULT_LOCK_TIMEOUT`] (5 s) and then fails fast with a clear "backlog is
//! locked" error. The lock is advisory: processes that bypass this helper are
//! not stopped, which is documented in the ADR and `AGENTS.md`.
//!
//! The lock is held by keeping a [`std::fs::File`] open with an exclusive
//! `fs4` byte-range lock; dropping [`RepoLock`] releases it.

use std::time::{Duration, Instant};

use fs4::fs_std::FileExt;

use crate::config::resolve_repo_root;
use crate::prelude::*;

/// Default time to wait for a contended repo lock before failing fast.
pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(5);

/// Polling interval used while waiting for a contended lock.
const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Advisory lock guard acquired around read-modify-write sequences.
///
/// Dropping the guard releases the exclusive lock and closes the lock file
/// handle. Hold it for the shortest practical scope.
pub struct RepoLock {
    file: fs::File,
}

impl std::fmt::Debug for RepoLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RepoLock").finish_non_exhaustive()
    }
}

impl RepoLock {
    /// Acquire the per-repo advisory lock with the default timeout.
    pub fn acquire(repo_root: impl AsRef<Path>) -> Result<Self> {
        Self::acquire_with_timeout(repo_root, DEFAULT_LOCK_TIMEOUT)
    }

    /// Acquire the per-repo advisory lock, blocking up to `timeout` before
    /// failing with a "backlog is locked" error.
    pub fn acquire_with_timeout(repo_root: impl AsRef<Path>, timeout: Duration) -> Result<Self> {
        let repo_root = resolve_repo_root(repo_root)?;
        let kanban_dir = repo_root.join(".kanban");
        fs::create_dir_all(&kanban_dir)
            .with_context(|| format!("create .kanban dir {}", kanban_dir.display()))?;
        let lock_path = kanban_dir.join(".lock");
        let file = fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("open repo lock {}", lock_path.display()))?;

        let deadline = Instant::now() + timeout;
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(RepoLock { file }),
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        bail!(
                            "Backlog is locked by another kanban process. Wait for it to finish and retry. (lock: {})",
                            lock_path.display()
                        );
                    }
                    std::thread::sleep(LOCK_POLL_INTERVAL);
                }
                Err(err) => {
                    bail!(
                        "Failed to acquire repo lock at {}: {err}",
                        lock_path.display()
                    );
                }
            }
        }
    }
}

impl Drop for RepoLock {
    fn drop(&mut self) {
        // Best-effort unlock; the OS releases the lock when the handle closes
        // anyway, but explicit unlock keeps the file descriptor tidy.
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn repo_lock_serializes_concurrent_acquirers() {
        let temp_root = tempdir().unwrap();
        // Initialize a minimal .kanban so resolve_repo_root succeeds.
        crate::config::init_config(temp_root.path()).unwrap();
        let repo_root = temp_root.path().to_path_buf();

        let lock = RepoLock::acquire(&repo_root).expect("first acquire succeeds");
        // A second acquire with a short timeout must fail fast while the first
        // is held.
        let err =
            RepoLock::acquire_with_timeout(&repo_root, Duration::from_millis(150)).unwrap_err();
        assert!(
            err.to_string().contains("locked"),
            "expected a locked error, got: {err}"
        );
        drop(lock);
        // After release, a new acquire succeeds immediately.
        let _relock = RepoLock::acquire_with_timeout(&repo_root, Duration::from_millis(500))
            .expect("re-acquire succeeds after release");
    }

    #[test]
    fn repo_lock_releases_on_early_return() {
        let temp_root = tempdir().unwrap();
        crate::config::init_config(temp_root.path()).unwrap();
        let repo_root = temp_root.path().to_path_buf();

        fn guarded(repo_root: &Path) -> Result<()> {
            let _lock = RepoLock::acquire(repo_root)?;
            // Simulate a failing write mid-sequence; the lock must still
            // release when `_lock` drops on the early return.
            bail!("simulated write failure");
        }

        let _ = guarded(&repo_root);
        // Must be able to re-acquire immediately because the guard released.
        let _lock = RepoLock::acquire_with_timeout(&repo_root, Duration::from_millis(500))
            .expect("lock released after early return");
    }
}
