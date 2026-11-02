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

/// Held for as long as it lives.
#[derive(Debug)]
pub struct Lock {
    _file: File,
}

/// The lock file inside the clone's common git dir (`git_dir`).
pub fn path(git_dir: &Path) -> PathBuf {
    git_dir.join("tether.lock")
}

/// Takes the lock, waiting up to `wait` for another run to finish; `None`
/// when it is still held after that. A zero `wait` tries once.
pub fn acquire(git_dir: &Path, wait: Duration) -> Result<Option<Lock>> {
    acquire_file(&path(git_dir), wait)
}

/// [`acquire`] on any lock file, for tether's own locks that are not the
/// shared contract (the per-repo `after` lock in [`crate::paths::Paths::after_lock`]).
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
    let deadline = Instant::now() + wait;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(Some(Lock { _file: file })),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(TryLockError::WouldBlock) => return Ok(None),
            Err(TryLockError::Error(e)) => {
                return Err(e).with_context(|| format!("locking {}", p.display()));
            }
        }
    }
}
