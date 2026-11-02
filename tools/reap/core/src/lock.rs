//! One writer per clone (rule 7). A pass holds an OS file lock (`flock`) on
//! `<git common dir>/reap.lock`, released when the process exits however it exits, so there is
//! never a stale lock to clean up. It also takes git-tether's `tether.lock` when that file
//! exists, and skips a clone whose `index.lock` exists, so reap, tether and git never write the
//! same clone at once.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

/// The locks a pass holds for as long as this lives.
#[derive(Debug)]
pub struct RepoLock {
    _files: Vec<File>,
}

/// Takes the clone's locks, waiting up to `wait` for another run; `Ok(Err(why))` when the
/// clone is busy after that. A zero `wait` tries once.
///
/// # Errors
///
/// Fails when a lock file cannot be opened or locked for a reason other than being held.
pub fn acquire(common_dir: &Path, wait: Duration) -> Result<Result<RepoLock, String>> {
    let deadline = Instant::now() + wait;
    let Some(reap) = take(&common_dir.join("reap.lock"), true, deadline)? else {
        return Ok(Err("another reap run holds it".into()));
    };
    let mut files = vec![reap];
    // git-tether's lock file persists between its syncs, so its existence says nothing; whether
    // it is held does.
    match take(&common_dir.join("tether.lock"), false, deadline)? {
        Some(tether) => files.push(tether),
        None if common_dir.join("tether.lock").exists() => {
            return Ok(Err("git-tether is syncing it".into()));
        }
        None => {}
    }
    if common_dir.join("index.lock").exists() {
        return Ok(Err("git is writing it (index.lock exists)".into()));
    }
    Ok(Ok(RepoLock { _files: files }))
}

/// Locks `path`, creating it only when `may_create`; `None` when it is held past `deadline`, or
/// when it does not exist and may not be created.
fn take(path: &Path, may_create: bool, deadline: Instant) -> Result<Option<File>> {
    let opened = OpenOptions::new()
        .create(may_create)
        .truncate(false)
        .write(true)
        .open(path);
    let file = match opened {
        Ok(file) => file,
        Err(e) if !may_create && e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("opening {}", path.display())),
    };
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(Some(file)),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(TryLockError::WouldBlock) => return Ok(None),
            Err(TryLockError::Error(e)) => {
                return Err(e).with_context(|| format!("locking {}", path.display()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A process another test forks while a lock is released holds a copy of the lock's
    /// descriptor until its exec closes it, so a released lock can stay held for a moment.
    const RELEASE_DELAY: Duration = Duration::from_secs(2);

    #[test]
    fn a_second_pass_on_the_same_clone_is_turned_away() {
        let dir = tempfile::tempdir().unwrap();

        let first = acquire(dir.path(), Duration::ZERO).unwrap();
        let second = acquire(dir.path(), Duration::ZERO).unwrap();

        assert!(first.is_ok());
        assert_eq!(second.unwrap_err(), "another reap run holds it");
        drop(first);
        assert!(acquire(dir.path(), RELEASE_DELAY).unwrap().is_ok());
    }

    #[test]
    fn a_held_tether_lock_skips_the_clone_and_a_free_one_is_taken() {
        let dir = tempfile::tempdir().unwrap();
        let tether = File::create(dir.path().join("tether.lock")).unwrap();
        tether.lock().unwrap();

        let busy = acquire(dir.path(), Duration::ZERO).unwrap();
        tether.unlock().unwrap();
        let free = acquire(dir.path(), RELEASE_DELAY).unwrap();

        assert_eq!(busy.unwrap_err(), "git-tether is syncing it");
        assert!(free.is_ok());
        assert!(
            tether.try_lock().is_err(),
            "reap holds tether.lock during its pass"
        );
    }

    #[test]
    fn an_index_lock_skips_the_clone() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.lock"), "").unwrap();

        let busy = acquire(dir.path(), Duration::ZERO).unwrap();

        assert_eq!(busy.unwrap_err(), "git is writing it (index.lock exists)");
    }
}
