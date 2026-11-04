//! The bounded walk (ADR 0004 rule 6).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Where discovery looks when `scan` is absent, relative to `$HOME`: those that exist.
/// Protected folders (`~/Desktop`, `~/Documents`, …) are never here (rule 5).
pub const DEFAULT_ROOT_NAMES: [&str; 6] = ["Development", "Code", "Projects", "src", "dev", "work"];

/// Folders the walk never enters, besides hidden ones.
const SKIPPED_NAMES: [&str; 2] = ["node_modules", "target"];

/// How far and how long one root is walked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// Levels below the root; the root is level 0.
    pub max_depth: usize,
    /// Folders read before the walk stops early.
    pub max_folders: usize,
    pub budget: Duration,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            max_depth: 4,
            max_folders: 20_000,
            budget: Duration::from_secs(2),
        }
    }
}

/// What walking one root found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootScan {
    pub root: PathBuf,
    /// Main clones, sorted by path.
    pub clones: Vec<PathBuf>,
    /// The folder or time cap was hit, so the list may be incomplete.
    pub stopped_early: bool,
    /// The root itself could not be read. Unreadable folders below it are skipped silently.
    pub error: Option<String>,
}

/// The default roots that exist under `home`.
#[must_use]
pub fn default_roots(home: &Path) -> Vec<PathBuf> {
    DEFAULT_ROOT_NAMES
        .iter()
        .map(|n| home.join(n))
        .filter(|p| p.is_dir())
        .collect()
}

/// Walks one root. Never fails: a root that cannot be read comes back with `error` set.
#[must_use]
pub fn scan_root(root: &Path, bounds: &Bounds) -> RootScan {
    let mut scan = RootScan {
        root: root.to_path_buf(),
        clones: Vec::new(),
        stopped_early: false,
        error: None,
    };
    match fs::metadata(root) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => {
            scan.error = Some("not a folder".into());
            return scan;
        }
        Err(e) => {
            scan.error = Some(reason(&e));
            return scan;
        }
    }
    if let Err(e) = fs::read_dir(root) {
        scan.error = Some(reason(&e));
        return scan;
    }
    let mut walk = Walk {
        bounds,
        started: Instant::now(),
        read: 0,
        clones: Vec::new(),
        stopped_early: false,
    };
    walk.folder(root, 0);
    walk.clones.sort();
    scan.clones = walk.clones;
    scan.stopped_early = walk.stopped_early;
    scan
}

fn reason(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => "not allowed to read it".into(),
        std::io::ErrorKind::NotFound => "no such folder".into(),
        _ => e.to_string(),
    }
}

struct Walk<'a> {
    bounds: &'a Bounds,
    started: Instant,
    read: usize,
    clones: Vec<PathBuf>,
    stopped_early: bool,
}

impl Walk<'_> {
    /// Reads `dir` (at `depth` below the root) and the folders under it.
    fn folder(&mut self, dir: &Path, depth: usize) {
        if self.stopped_early {
            return;
        }
        if self.read >= self.bounds.max_folders || self.started.elapsed() >= self.bounds.budget {
            self.stopped_early = true;
            return;
        }
        self.read += 1;
        // `symlink_metadata`: a `.git` that is a link is neither a clone nor a worktree.
        match fs::symlink_metadata(dir.join(".git")) {
            Ok(m) if m.is_dir() => {
                self.clones.push(dir.to_path_buf());
                return;
            }
            // A `.git` file is a linked worktree or a submodule: never suggested.
            Ok(m) if m.is_file() => return,
            _ => {}
        }
        if depth >= self.bounds.max_depth {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut children: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                !name.starts_with('.') && !SKIPPED_NAMES.contains(&name.as_ref())
            })
            .map(|e| e.path())
            .collect();
        children.sort();
        for child in children {
            self.folder(&child, depth + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clone_at(root: &Path, rel: &str) -> PathBuf {
        let dir = root.join(rel);
        fs::create_dir_all(dir.join(".git")).unwrap();
        dir
    }

    fn folder_at(root: &Path, rel: &str) -> PathBuf {
        let dir = root.join(rel);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_main_clones_sorted_and_does_not_enter_them() {
        let tmp = tempfile::tempdir().unwrap();
        let b = clone_at(tmp.path(), "b");
        let a = clone_at(tmp.path(), "group/a");
        // Inside a clone: not entered, so not found.
        clone_at(&b, "vendor/inner");
        let scan = scan_root(tmp.path(), &Bounds::default());
        assert_eq!(scan.clones, vec![b, a]);
        assert!(!scan.stopped_early);
        assert_eq!(scan.error, None);
    }

    #[test]
    fn a_git_file_is_a_worktree_or_submodule_and_is_never_found() {
        let tmp = tempfile::tempdir().unwrap();
        let wt = folder_at(tmp.path(), "wt");
        fs::write(wt.join(".git"), "gitdir: /elsewhere/.git/worktrees/wt\n").unwrap();
        clone_at(tmp.path(), "wt/nested");
        assert_eq!(
            scan_root(tmp.path(), &Bounds::default()).clones,
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn hidden_node_modules_and_target_folders_are_not_entered() {
        let tmp = tempfile::tempdir().unwrap();
        clone_at(tmp.path(), ".hidden/a");
        clone_at(tmp.path(), "node_modules/a");
        clone_at(tmp.path(), "target/a");
        let kept = clone_at(tmp.path(), "src/a");
        assert_eq!(scan_root(tmp.path(), &Bounds::default()).clones, vec![kept]);
    }

    #[test]
    fn stops_at_the_depth_bound() {
        let tmp = tempfile::tempdir().unwrap();
        let four = clone_at(tmp.path(), "1/2/3/4");
        clone_at(tmp.path(), "1/2/3/4/5");
        clone_at(tmp.path(), "a/b/c/d/e");
        let scan = scan_root(tmp.path(), &Bounds::default());
        assert_eq!(scan.clones, vec![four]);
        assert!(!scan.stopped_early);
    }

    #[test]
    fn a_root_that_is_itself_a_clone_is_found() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join(".git")).unwrap();
        assert_eq!(
            scan_root(tmp.path(), &Bounds::default()).clones,
            vec![tmp.path().to_path_buf()]
        );
    }

    #[test]
    fn symlinks_are_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        clone_at(outside.path(), "far");
        let root = folder_at(tmp.path(), "root");
        std::os::unix::fs::symlink(outside.path(), root.join("link")).unwrap();
        // A link to a clone is not a clone either.
        std::os::unix::fs::symlink(outside.path().join("far"), root.join("linked-clone")).unwrap();
        assert_eq!(
            scan_root(&root, &Bounds::default()).clones,
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn stops_early_at_the_folder_cap_and_says_so() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..10 {
            folder_at(tmp.path(), &format!("d{i}"));
        }
        clone_at(tmp.path(), "z");
        let bounds = Bounds {
            max_folders: 4,
            ..Bounds::default()
        };
        let scan = scan_root(tmp.path(), &bounds);
        assert!(scan.stopped_early);
        assert_eq!(scan.clones, Vec::<PathBuf>::new());
        assert_eq!(scan.error, None);
    }

    #[test]
    fn stops_early_when_the_time_is_up() {
        let tmp = tempfile::tempdir().unwrap();
        clone_at(tmp.path(), "a");
        let bounds = Bounds {
            budget: Duration::ZERO,
            ..Bounds::default()
        };
        let scan = scan_root(tmp.path(), &bounds);
        assert!(scan.stopped_early);
    }

    #[test]
    fn a_missing_or_file_root_is_an_error_not_a_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = scan_root(&tmp.path().join("nope"), &Bounds::default());
        assert_eq!(missing.error.as_deref(), Some("no such folder"));
        assert_eq!(missing.clones, Vec::<PathBuf>::new());
        let file = tmp.path().join("f");
        fs::write(&file, "").unwrap();
        assert_eq!(
            scan_root(&file, &Bounds::default()).error.as_deref(),
            Some("not a folder")
        );
    }

    #[test]
    fn an_unreadable_root_reports_the_error() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let locked = folder_at(tmp.path(), "locked");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let scan = scan_root(&locked, &Bounds::default());
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        // Running as root reads it anyway; everywhere else it is an error.
        if scan.error.is_some() {
            assert_eq!(scan.error.as_deref(), Some("not allowed to read it"));
        }
    }

    #[test]
    fn default_roots_are_the_ones_that_exist_and_never_protected_folders() {
        let home = tempfile::tempdir().unwrap();
        for d in ["Development", "work", "Documents", "Desktop", "Downloads"] {
            folder_at(home.path(), d);
        }
        // A file named like a root is not a root.
        fs::write(home.path().join("Code"), "").unwrap();
        assert_eq!(
            default_roots(home.path()),
            vec![home.path().join("Development"), home.path().join("work")]
        );
    }
}
