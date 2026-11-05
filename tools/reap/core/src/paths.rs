//! Where reap reads and writes, and which `git` and `gh` it runs. Every location can be
//! redirected so tests never touch the real ones (rule 17).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
    /// `~/.config/reap`, or `REAP_CONFIG_DIR`.
    pub config: PathBuf,
    /// The `git` binary: `REAP_GIT`, else `git` on `PATH`.
    pub git: OsString,
    /// The `gh` binary: `REAP_GH`, else `gh` on `PATH`, else Homebrew's. Tests point it at a
    /// fake that answers from fixtures.
    pub gh: OsString,
}

/// Where Homebrew installs `gh` on Apple silicon and on Intel. An app started from Finder or at
/// login gets launchd's `PATH`, which has neither.
const HOMEBREW_GH: &[&str] = &["/opt/homebrew/bin/gh", "/usr/local/bin/gh"];

/// `gh` on `search_path` if it is there, else the first Homebrew install that exists, else the
/// bare name (which then fails as "not installed").
fn find_gh(search_path: Option<&std::ffi::OsStr>) -> OsString {
    let on_path = search_path
        .into_iter()
        .flat_map(std::env::split_paths)
        .map(|folder| folder.join("gh"))
        .find(|candidate| candidate.is_file());
    on_path
        .or_else(|| {
            HOMEBREW_GH
                .iter()
                .map(PathBuf::from)
                .find(|candidate| candidate.is_file())
        })
        .map_or_else(|| "gh".into(), PathBuf::into_os_string)
}

impl Paths {
    /// Reads the locations from the environment.
    ///
    /// # Errors
    ///
    /// Fails when `HOME` is not set.
    pub fn from_env() -> Result<Self> {
        let home = PathBuf::from(std::env::var("HOME").context("HOME is not set")?);
        let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        Ok(Self {
            config: var("REAP_CONFIG_DIR").map_or_else(|| home.join(".config/reap"), PathBuf::from),
            git: var("REAP_GIT").unwrap_or_else(|| "git".into()),
            gh: var("REAP_GH").unwrap_or_else(|| find_gh(std::env::var_os("PATH").as_deref())),
            home,
        })
    }

    /// Explicit construction for tests, with `git` and `gh` from `PATH`.
    #[must_use]
    pub fn new(home: &Path, config: &Path) -> Self {
        Self {
            home: home.to_path_buf(),
            config: config.to_path_buf(),
            git: "git".into(),
            gh: "gh".into(),
        }
    }

    #[must_use]
    pub fn config_file(&self) -> PathBuf {
        self.config.join("repos.toml")
    }

    /// What the last `discover` found (repo ADR 0004 rule 10); its own file, never a key in
    /// `state.json`, whose loader would drop every record on a key it does not know.
    #[must_use]
    pub fn discovery_file(&self) -> PathBuf {
        self.config.join("discovery.json")
    }

    /// Present while background sweeping is paused (`git reap pause`, the popover's *Pause*).
    #[must_use]
    pub fn state_file(&self) -> PathBuf {
        self.config.join("state.json")
    }

    #[must_use]
    pub fn pause_file(&self) -> PathBuf {
        self.config.join("paused")
    }

    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.pause_file().exists()
    }

    /// Creates or removes the pause file.
    ///
    /// # Errors
    ///
    /// Fails when the config folder or the file cannot be written.
    pub fn set_paused(&self, is_paused: bool) -> std::io::Result<()> {
        if is_paused {
            std::fs::create_dir_all(&self.config)?;
            return std::fs::write(self.pause_file(), "");
        }
        match std::fs::remove_file(self.pause_file()) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// `path` with `$HOME` shown as `~`, for the config file and messages.
    #[must_use]
    pub fn tilde(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        }
    }

    /// The inverse of [`Paths::tilde`]: `~/x` is under `$HOME`.
    #[must_use]
    pub fn expand(&self, path: &str) -> PathBuf {
        match path.strip_prefix("~/") {
            Some(rest) => self.home.join(rest),
            None if path == "~" => self.home.clone(),
            None => PathBuf::from(path),
        }
    }
}

/// The path made absolute against the current directory, resolving symlinks when it exists
/// (macOS `/var` vs `/private/var`).
#[must_use]
pub fn absolute(path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    std::fs::canonicalize(&path).unwrap_or(path)
}

#[must_use]
pub fn same_path(a: &Path, b: &Path) -> bool {
    absolute(a) == absolute(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_round_trips_under_the_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path(), &dir.path().join("cfg/reap"));
        assert!(!paths.is_paused());
        paths.set_paused(false).unwrap();
        paths.set_paused(true).unwrap();
        assert!(paths.is_paused());
        paths.set_paused(false).unwrap();
        assert!(!paths.is_paused());
    }

    #[test]
    fn gh_is_found_on_path_first() {
        let dir = tempfile::tempdir().unwrap();
        let gh = dir.path().join("gh");
        std::fs::write(&gh, "").unwrap();

        assert_eq!(find_gh(Some(dir.path().as_os_str())), gh.into_os_string());
    }

    #[test]
    fn without_gh_on_path_a_homebrew_install_or_the_bare_name_is_used() {
        let dir = tempfile::tempdir().unwrap();
        let expected = HOMEBREW_GH
            .iter()
            .find(|candidate| Path::new(candidate).is_file())
            .map_or_else(|| OsString::from("gh"), OsString::from);

        assert_eq!(find_gh(Some(dir.path().as_os_str())), expected);
        assert_eq!(find_gh(None), expected);
    }

    #[test]
    fn tilde_and_expand_are_inverse() {
        let paths = Paths::new(Path::new("/Users/me"), Path::new("/c"));
        assert_eq!(
            paths.tilde(Path::new("/Users/me/Development/app")),
            "~/Development/app"
        );
        assert_eq!(
            paths.expand("~/Development/app"),
            PathBuf::from("/Users/me/Development/app")
        );
        assert_eq!(paths.expand("/srv/x"), PathBuf::from("/srv/x"));
        assert_eq!(paths.tilde(Path::new("/srv/x")), "/srv/x");
    }
}
