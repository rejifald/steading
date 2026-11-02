//! Where tether reads and writes. Every location can be redirected so tests
//! never touch the real ones (ADR 0001 rule 13).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// concord's clone relative to `$HOME` (ADR 0002 rule 1).
pub const DEFAULT_CONCORD_REPO: &str = "Development/handbook";
/// concord's login item label; its plist marks concord as installed.
pub const CONCORD_LABEL: &str = "io.github.rejifald.concord";

#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
    /// `~/.config/tether`, or `TETHER_CONFIG_DIR`.
    pub config: PathBuf,
    /// The clone concord owns: `TETHER_CONCORD_REPO_DIR`, else concord's own
    /// `CONCORD_REPO_DIR`, else `~/Development/handbook` (rule 2).
    pub concord_repo: PathBuf,
    /// The `git` binary: `TETHER_GIT`, else `git` on `PATH`. Tests point it at
    /// a wrapper that logs every call.
    pub git: OsString,
}

impl Paths {
    pub fn from_env() -> Result<Self> {
        let home = PathBuf::from(std::env::var("HOME").context("HOME is not set")?);
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
        Ok(Self {
            config: var("TETHER_CONFIG_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config/tether")),
            concord_repo: var("TETHER_CONCORD_REPO_DIR")
                .or_else(|| var("CONCORD_REPO_DIR"))
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(DEFAULT_CONCORD_REPO)),
            git: var("TETHER_GIT").unwrap_or_else(|| "git".into()),
            home,
        })
    }

    /// Explicit construction for tests: concord's clone at its default place.
    pub fn new(home: &Path, config: &Path) -> Self {
        Self {
            home: home.to_path_buf(),
            config: config.to_path_buf(),
            concord_repo: home.join(DEFAULT_CONCORD_REPO),
            git: "git".into(),
        }
    }

    pub fn config_file(&self) -> PathBuf {
        self.config.join("repos.toml")
    }

    /// Last result per repo (`record`), so `status` and the tray can say when
    /// a repo last synced and why it was skipped.
    pub fn state_file(&self) -> PathBuf {
        self.config.join("state.json")
    }

    /// Rule 9: `backups/<repo>/<timestamp>.diff`.
    pub fn backups(&self, repo_name: &str) -> PathBuf {
        self.config.join("backups").join(repo_name)
    }

    /// The lock a sync of a repo with an `after` holds from reading its record
    /// to writing it back, so two runs never interleave `after_failed` (ADR 0004
    /// rule 3). Keyed by the repo as listed (`key`, escaped into one file name):
    /// it lives here, not in the clone, because it is taken before the gate,
    /// which runs before any git command, and before a mirror is cloned.
    pub fn after_lock(&self, key: &str) -> PathBuf {
        let name = key.replace('%', "%25").replace('/', "%2F");
        self.config.join("after-locks").join(format!("{name}.lock"))
    }

    /// Present while background sync is paused (menu *Pause*, `git tether pause`);
    /// the tray's timers and `git tether sync --due` both honour it.
    pub fn pause_file(&self) -> PathBuf {
        self.config.join("paused")
    }

    pub fn is_paused(&self) -> bool {
        self.pause_file().exists()
    }

    pub fn set_paused(&self, paused: bool) -> std::io::Result<()> {
        if paused {
            std::fs::create_dir_all(&self.config)?;
            std::fs::write(self.pause_file(), "")
        } else {
            match std::fs::remove_file(self.pause_file()) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        }
    }

    /// concord counts as installed when its app or its login item is present.
    pub fn concord_installed(&self) -> bool {
        self.home.join("Applications/concord.app").exists()
            || self
                .home
                .join(format!("Library/LaunchAgents/{CONCORD_LABEL}.plist"))
                .exists()
    }

    /// `path` with `$HOME` shown as `~`, for the config file and messages.
    pub fn tilde(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        }
    }

    /// The inverse of [`Paths::tilde`]: `~/x` is under `$HOME`.
    pub fn expand(&self, path: &str) -> PathBuf {
        match path.strip_prefix("~/") {
            Some(rest) => self.home.join(rest),
            None if path == "~" => self.home.clone(),
            None => PathBuf::from(path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_round_trips_under_the_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        let p = Paths::new(dir.path(), &dir.path().join("cfg/tether"));
        assert!(!p.is_paused());
        p.set_paused(false).unwrap();
        p.set_paused(true).unwrap();
        assert!(p.is_paused());
        p.set_paused(false).unwrap();
        assert!(!p.is_paused());
    }

    #[test]
    fn tilde_and_expand_are_inverse() {
        let p = Paths::new(Path::new("/Users/me"), Path::new("/c"));
        assert_eq!(
            p.tilde(Path::new("/Users/me/Development/notes")),
            "~/Development/notes"
        );
        assert_eq!(
            p.expand("~/Development/notes"),
            PathBuf::from("/Users/me/Development/notes")
        );
        assert_eq!(p.expand("/srv/x"), PathBuf::from("/srv/x"));
        assert_eq!(p.tilde(Path::new("/srv/x")), "/srv/x");
    }

    #[test]
    fn concord_counts_as_installed_by_app_or_login_item() {
        let dir = tempfile::tempdir().unwrap();
        let p = Paths::new(dir.path(), &dir.path().join("cfg"));
        assert!(!p.concord_installed());
        let agent = dir
            .path()
            .join(format!("Library/LaunchAgents/{CONCORD_LABEL}.plist"));
        std::fs::create_dir_all(agent.parent().unwrap()).unwrap();
        std::fs::write(&agent, "").unwrap();
        assert!(p.concord_installed());
        std::fs::remove_file(&agent).unwrap();
        std::fs::create_dir_all(dir.path().join("Applications/concord.app")).unwrap();
        assert!(p.concord_installed());
    }
}
