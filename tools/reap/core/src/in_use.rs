//! Which worktrees a process is running inside (rule 5): one `lsof -d cwd` per pass, matched
//! by path prefix.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// The current directories of every process this user can see.
#[derive(Debug, Clone, Default)]
pub struct WorkingDirectories(Vec<PathBuf>);

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

    /// Parses `lsof -F n` output: one `n<path>` line per open current directory.
    pub fn parse(output: &str) -> Self {
        Self(
            output
                .lines()
                .filter_map(|line| line.strip_prefix('n'))
                .map(PathBuf::from)
                .collect(),
        )
    }

    /// Whether any process's current directory is `worktree` or inside it.
    #[must_use]
    pub fn is_inside(&self, worktree: &Path) -> bool {
        self.0
            .iter()
            .any(|directory| directory.starts_with(worktree))
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
