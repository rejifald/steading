//! How much disk a worktree's folder takes: the space removing it would give back.
//!
//! A file hard-linked from outside the folder (a package manager's shared store) stays on disk
//! after the removal, so it counts only when every one of its links lies inside the folder.
//! Space shared through APFS clones cannot be seen this way and counts in full.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::thread;

/// `st_blocks` counts 512-byte units on every Unix, whatever the file system's block size.
const BLOCK_SIZE: u64 = 512;

/// The bytes freed by deleting the folder at `root`: the blocks allocated to everything under
/// it, each file once, without following symlinks.
///
/// # Errors
///
/// Fails when any folder or file under `root` cannot be read, since a partial sum would claim
/// less than the folder holds.
pub fn bytes_freed_by_removing(root: &Path) -> io::Result<u64> {
    let mut total = 0;
    // (device, inode) of a file with several links, to the links not yet seen.
    let mut unseen_links: HashMap<(u64, u64), u64> = HashMap::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        // A build or test run deletes files under a live worktree: what vanished mid-walk no
        // longer takes space, but a missing `root` is still an error.
        let Some(metadata) = skip_vanished(fs::symlink_metadata(&folder), folder != root)? else {
            continue;
        };
        total += metadata.blocks() * BLOCK_SIZE;
        let Some(entries) = skip_vanished(fs::read_dir(&folder), folder != root)? else {
            continue;
        };
        for entry in entries {
            let Some(entry) = skip_vanished(entry, true)? else {
                continue;
            };
            let Some(metadata) = skip_vanished(entry.metadata(), true)? else {
                continue;
            };
            if metadata.is_dir() {
                folders.push(entry.path());
                continue;
            }
            if metadata.nlink() <= 1 {
                total += metadata.blocks() * BLOCK_SIZE;
                continue;
            }
            let unseen = unseen_links
                .entry((metadata.dev(), metadata.ino()))
                .or_insert(metadata.nlink());
            *unseen -= 1;
            if *unseen == 0 {
                total += metadata.blocks() * BLOCK_SIZE;
            }
        }
    }
    Ok(total)
}

/// `None` for a `NotFound` error when `may_vanish`; every other error is passed on.
fn skip_vanished<T>(result: io::Result<T>, may_vanish: bool) -> io::Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(e) if may_vanish && e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// [`bytes_freed_by_removing`] for each folder at once, one thread each, in the same order;
/// `None` where a folder could not be measured.
#[must_use]
pub fn measure_all(roots: &[PathBuf]) -> Vec<Option<u64>> {
    thread::scope(|scope| {
        let measurements: Vec<_> = roots
            .iter()
            .map(|root| scope.spawn(move || bytes_freed_by_removing(root).ok()))
            .collect();
        measurements
            .into_iter()
            .map(|measurement| measurement.join().ok().flatten())
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIB: usize = 1024;

    fn write(path: &Path, size: usize) {
        fs::write(path, vec![b'x'; size]).unwrap();
    }

    #[test]
    fn counts_files_in_nested_folders() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("a/b")).unwrap();
        write(&root.path().join("a/b/big"), 64 * KIB);

        let bytes = bytes_freed_by_removing(root.path()).unwrap();

        assert!(bytes >= 64 * 1024, "{bytes}");
    }

    #[test]
    fn counts_a_file_linked_twice_inside_once() {
        let root = tempfile::tempdir().unwrap();
        write(&root.path().join("one"), 64 * KIB);
        let once = bytes_freed_by_removing(root.path()).unwrap();
        fs::hard_link(root.path().join("one"), root.path().join("two")).unwrap();

        assert_eq!(bytes_freed_by_removing(root.path()).unwrap(), once);
    }

    #[test]
    fn leaves_out_a_file_also_linked_from_outside() {
        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let empty = bytes_freed_by_removing(root.path()).unwrap();
        write(&outside.path().join("store"), 64 * KIB);
        fs::hard_link(outside.path().join("store"), root.path().join("linked")).unwrap();

        assert_eq!(bytes_freed_by_removing(root.path()).unwrap(), empty);
    }

    #[test]
    fn does_not_follow_a_symlink() {
        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        write(&outside.path().join("big"), 256 * KIB);
        std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();

        assert!(bytes_freed_by_removing(root.path()).unwrap() < 256 * 1024);
    }

    #[test]
    fn something_that_vanished_mid_walk_is_skipped_but_a_missing_root_is_not() {
        let gone = io::Error::from(io::ErrorKind::NotFound);
        assert!(skip_vanished::<()>(Err(gone), true).unwrap().is_none());
        let gone = io::Error::from(io::ErrorKind::NotFound);
        assert!(skip_vanished::<()>(Err(gone), false).is_err());
        let denied = io::Error::from(io::ErrorKind::PermissionDenied);
        assert!(skip_vanished::<()>(Err(denied), true).is_err());
    }

    #[test]
    fn a_folder_that_is_gone_has_no_size() {
        let root = tempfile::tempdir().unwrap();
        let gone = root.path().join("gone");

        assert_eq!(measure_all(&[gone, root.path().to_path_buf()])[0], None);
    }
}
