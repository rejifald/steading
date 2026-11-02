//! One writer per clone (rule 4). The tray's timers, a CLI run and *Sync now*
//! can all reach the same clone; each sync holds an exclusive lock on
//! `<git common dir>/tether.lock` for its whole length. The name is a contract
//! (ADR 0004 rule 5): any other tool that writes to a listed clone takes the
//! same lock, and a repo's `after` runs only once a sync has released it.
//!
//! The lock is an OS file lock (`flock`), released when the process exits
//! however it exits: there is never a stale lock to clean up.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

/// How long a lock that looks held is retried even with no wait. A process being spawned on any
/// thread holds a copy of this process's descriptors, the lock's among them, until its program
/// starts, so a lock just released can look held for some microseconds; a real holder keeps it
/// for a whole run.
const SPAWN_GRACE: Duration = Duration::from_millis(50);

/// The pause between tries.
const RETRY: Duration = Duration::from_millis(10);

/// Held for as long as it lives.
#[derive(Debug)]
pub struct Lock {
    _file: File,
}

/// The lock file inside the clone's common git dir (`git_dir`).
#[must_use]
pub fn path(git_dir: &Path) -> PathBuf {
    git_dir.join("tether.lock")
}

/// Takes the lock, waiting up to `wait` for another run to finish; `None`
/// when it is still held after that. A zero `wait` gives up after [`SPAWN_GRACE`].
///
/// # Errors
///
/// Fails when the lock file cannot be created, opened or locked.
pub fn acquire(git_dir: &Path, wait: Duration) -> Result<Option<Lock>> {
    acquire_file(&path(git_dir), wait)
}

/// [`acquire`] on any lock file, for tether's own locks that are not the
/// shared contract (the per-repo `after` lock in [`crate::paths::Paths::after_lock`]).
///
/// # Errors
///
/// Fails when the lock file cannot be created, opened or locked.
pub fn acquire_file(p: &Path, wait: Duration) -> Result<Option<Lock>> {
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(p)
        .with_context(|| format!("opening {}", p.display()))?;
    let deadline = Instant::now() + wait.max(SPAWN_GRACE);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(Some(Lock { _file: file })),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(RETRY);
            }
            Err(TryLockError::WouldBlock) => return Ok(None),
            Err(TryLockError::Error(e)) => {
                return Err(e).with_context(|| format!("locking {}", p.display()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[test]
    fn a_released_lock_is_free_at_once_while_other_threads_spawn() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.lock");
        let stop = Arc::new(AtomicBool::new(false));
        let spawners: Vec<_> = (0..4)
            .map(|_| {
                let stop = Arc::clone(&stop);
                std::thread::spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        Command::new("true").status().unwrap();
                    }
                })
            })
            .collect();

        let busy = (0..300)
            .filter(|_| acquire_file(&p, Duration::ZERO).unwrap().is_none())
            .count();

        stop.store(true, Ordering::Relaxed);
        for spawner in spawners {
            spawner.join().unwrap();
        }
        assert_eq!(busy, 0, "a lock this process had released looked held");
    }

    #[test]
    fn a_held_lock_is_still_busy_with_no_wait() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.lock");
        let _held = acquire_file(&p, Duration::ZERO).unwrap().unwrap();

        assert!(acquire_file(&p, Duration::ZERO).unwrap().is_none());
    }
}
