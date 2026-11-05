//! The volume a folder is on and how much of it is free (ADR low-disk-skips-the-grace).
//!
//! The reading is `statfs`'s: the space a process that is not root can still write
//! (`f_bavail`), not macOS's "available for important usage", which counts purgeable space as
//! free but needs Foundation. [`Ctx`](crate::ctx::Ctx) holds the reader, so tests fake a low
//! volume without filling a disk (rule 17).

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// A mounted volume and its space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    /// `st_dev` of the folder: folders with the same id share one pool of space.
    pub id: u64,
    /// Where it is mounted, such as `/System/Volumes/Data` or `/Volumes/Backup`.
    pub mount: PathBuf,
    pub total: u64,
    /// What a process that is not root can still write.
    pub available: u64,
}

impl Volume {
    /// How the owner knows it: `Backup` for `/Volumes/Backup`, the startup disk for the boot
    /// volume's mount points, else the mount point.
    #[must_use]
    pub fn name(&self) -> String {
        if let Ok(name) = self.mount.strip_prefix("/Volumes")
            && !name.as_os_str().is_empty()
        {
            return name.display().to_string();
        }
        if self.mount == Path::new("/") || self.mount == Path::new("/System/Volumes/Data") {
            return "the startup disk".into();
        }
        self.mount.display().to_string()
    }
}

/// Reads the volume `path` is on. Tests inject their own.
pub type ReadVolume = dyn Fn(&Path) -> io::Result<Volume> + Send + Sync;

/// The real reader: `stat` for the device and `statfs` for the space.
///
/// # Errors
///
/// Fails when `path` does not exist or cannot be read.
pub fn read(path: &Path) -> io::Result<Volume> {
    let id = std::fs::metadata(path)?.dev();
    let c_path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains a NUL"))?;
    // SAFETY: `statfs` only writes into `stat`, which is a zeroed plain-old-data struct, and
    // reads `c_path`, a NUL-terminated string that outlives the call.
    let stat = unsafe {
        let mut stat: libc::statfs = std::mem::zeroed();
        if libc::statfs(c_path.as_ptr(), &raw mut stat) != 0 {
            return Err(io::Error::last_os_error());
        }
        stat
    };
    let (block, blocks, available_blocks) = counts(&stat);
    Ok(Volume {
        id,
        mount: mount_of(&stat, path, id),
        total: blocks.saturating_mul(block),
        available: available_blocks.saturating_mul(block),
    })
}

/// The block size, the blocks in the volume and the blocks a non-root process can still use.
// The fields are u32, u64, i64 or u32 depending on the platform; this makes them all u64.
#[allow(
    clippy::useless_conversion,
    clippy::unnecessary_cast,
    clippy::cast_sign_loss
)]
fn counts(stat: &libc::statfs) -> (u64, u64, u64) {
    // macOS counts blocks in `f_bsize` units; Linux in `f_frsize` units.
    #[cfg(target_os = "macos")]
    let block = u64::from(stat.f_bsize);
    #[cfg(not(target_os = "macos"))]
    let block = stat.f_frsize as u64;
    (block, stat.f_blocks as u64, stat.f_bavail as u64)
}

/// macOS names the mount point in `statfs`; elsewhere it is the highest folder above `path` on
/// the same device.
#[cfg(target_os = "macos")]
fn mount_of(stat: &libc::statfs, _path: &Path, _id: u64) -> PathBuf {
    // SAFETY: the kernel NUL-terminates `f_mntonname`.
    let name = unsafe { std::ffi::CStr::from_ptr(stat.f_mntonname.as_ptr()) };
    PathBuf::from(std::ffi::OsStr::from_bytes(name.to_bytes()))
}

#[cfg(not(target_os = "macos"))]
fn mount_of(_stat: &libc::statfs, path: &Path, id: u64) -> PathBuf {
    let mut mount = path.to_path_buf();
    for folder in path.ancestors().skip(1) {
        if std::fs::metadata(folder).is_ok_and(|metadata| metadata.dev() == id) {
            mount = folder.to_path_buf();
        } else {
            break;
        }
    }
    mount
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(mount: &str) -> Volume {
        Volume {
            id: 1,
            mount: mount.into(),
            total: 100,
            available: 10,
        }
    }

    #[test]
    fn a_folder_has_a_volume_with_space() {
        let dir = tempfile::tempdir().unwrap();

        let volume = read(dir.path()).unwrap();

        assert!(volume.total > 0 && volume.available <= volume.total);
        assert!(volume.mount.is_absolute());
        assert_eq!(volume.id, read(dir.path()).unwrap().id);
    }

    #[test]
    fn a_missing_folder_has_no_volume() {
        let dir = tempfile::tempdir().unwrap();

        assert!(read(&dir.path().join("gone")).is_err());
    }

    #[test]
    fn volumes_are_named_as_the_owner_knows_them() {
        assert_eq!(volume("/Volumes/Backup").name(), "Backup");
        assert_eq!(volume("/System/Volumes/Data").name(), "the startup disk");
        assert_eq!(volume("/").name(), "the startup disk");
        assert_eq!(volume("/mnt/data").name(), "/mnt/data");
    }
}
