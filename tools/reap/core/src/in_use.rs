//! Which worktrees a process is running inside (rule 5): one `lsof -d cwd` per pass, matched
//! by path prefix. The same pass tells [`crate::servers`] which process each directory belongs to.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// The current directories of every process this user can see, each with its process's pid.
#[derive(Debug, Clone, Default)]
pub struct WorkingDirectories(Vec<(u32, PathBuf)>);

impl WorkingDirectories {
    /// Asks `lsof` for every process's current directory.
    ///
    /// # Errors
    ///
    /// Fails when `lsof` cannot run. The caller must then treat every worktree as in use: a
    /// guard that cannot be checked does not pass.
    pub fn read() -> Result<Self> {
        let output = Command::new("lsof")
            .args(["-n", "-w", "-d", "cwd", "-F", "n"])
            .output()
            .context("running lsof")?;
        // lsof exits 1 when some processes could not be read, which is normal for other users'.
        if output.stdout.is_empty() && !output.status.success() {
            bail!(
                "lsof failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(Self::parse(&String::from_utf8_lossy(&output.stdout)))
    }

    /// Parses `lsof -F n` output: a `p<pid>` line per process, then one `n<path>` line per open
    /// current directory. A directory lsof printed before any pid gets pid 0, which matches no
    /// process but still counts for [`Self::is_inside`].
    #[must_use]
    pub fn parse(output: &str) -> Self {
        let mut pid = 0;
        let mut directories = Vec::new();
        for line in output.lines() {
            if let Some(number) = line.strip_prefix('p') {
                pid = number.parse().unwrap_or(0);
            } else if let Some(path) = line.strip_prefix('n') {
                directories.push((pid, PathBuf::from(path)));
            }
        }
        Self(directories)
    }

    /// Every process's pid and current directory, as lsof listed them.
    pub fn entries(&self) -> impl Iterator<Item = (u32, &Path)> {
        self.0.iter().map(|(pid, path)| (*pid, path.as_path()))
    }

    /// Whether any process's current directory is `worktree` or inside it.
    #[must_use]
    pub fn is_inside(&self, worktree: &Path) -> bool {
        self.0
            .iter()
            .any(|(_, directory)| directory.starts_with(worktree))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_worktree_and_folders_inside_it_only() {
        let directories =
            WorkingDirectories::parse("p1\nfcwd\nn/w/feat/src\np2\nfcwd\nn/w/other\n");

        assert!(directories.is_inside(Path::new("/w/feat")));
        assert!(directories.is_inside(Path::new("/w/other")));
        assert!(!directories.is_inside(Path::new("/w/fe")));
        assert!(!directories.is_inside(Path::new("/w/feature")));
    }

    #[test]
    fn keeps_each_directory_with_its_pid() {
        let directories = WorkingDirectories::parse("p41\nfcwd\nn/w/feat\np7\nfcwd\nn/\n");

        let entries: Vec<_> = directories.entries().collect();
        assert_eq!(entries, [(41, Path::new("/w/feat")), (7, Path::new("/"))]);
    }

    #[test]
    fn sees_a_process_started_inside_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let folder = std::fs::canonicalize(dir.path()).unwrap();
        let mut child = Command::new("sleep")
            .arg("30")
            .current_dir(&folder)
            .spawn()
            .unwrap();

        let directories = WorkingDirectories::read();
        child.kill().unwrap();
        child.wait().unwrap();

        assert!(directories.unwrap().is_inside(&folder));
    }
}
