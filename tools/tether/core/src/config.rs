//! `repos.toml` (rule 1): one `[[repo]]` per clone. Nothing that is not listed
//! is ever synced.
//!
//! ```toml
//! [[repo]]
//! path = "~/Development/notes"
//! branch = "main"
//! mode = "follow"     # or "mirror"
//! remote = "origin"   # optional
//! interval = "5m"     # optional: <n>s, <n>m or <n>h
//! gate = "my-tool idle"       # optional (ADR 0004): exit 0 lets a background sync run
//! after = "my-tool reconcile" # optional (ADR 0004): run when a sync moved the tip
//! ```

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::paths::Paths;

pub const DEFAULT_REMOTE: &str = "origin";
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// A poll is one `ls-remote`; more often than this only adds load on the host.
pub const MIN_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Fast-forward or skip; never destroys anything (rules 5, 6).
    #[default]
    Follow,
    /// Shallow, forced to the branch tip; local changes are discarded (rule 7).
    Mirror,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Follow => "follow",
            Mode::Mirror => "mirror",
        }
    }
}

/// One `[[repo]]` entry as written in the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoEntry {
    pub path: String,
    pub branch: String,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    /// Where a `mirror` clones from when its path is missing or empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub repo: Vec<RepoEntry>,
}

/// A validated entry with its path expanded and its defaults applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub path: PathBuf,
    pub branch: String,
    pub mode: Mode,
    pub remote: String,
    pub interval: Duration,
    pub url: Option<String>,
    /// A shell command a background sync asks first; exit 0 means go.
    pub gate: Option<String>,
    /// A shell command run after a sync moved the tip, once the lock is free.
    pub after: Option<String>,
}

impl Repo {
    /// The short name used for backups and `tether sync <repo>`: the directory's name.
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "repo".into())
    }

    pub fn tracking_ref(&self) -> String {
        format!("refs/remotes/{}/{}", self.remote, self.branch)
    }

    /// Whether `selector` (a name or a path) names this repo.
    pub fn matches(&self, selector: &str, paths: &Paths) -> bool {
        if selector == self.name() {
            return true;
        }
        let wanted = absolute(&paths.expand(selector));
        same_path(&wanted, &self.path)
    }
}

/// `5m`, `90s`, `1h`, or a bare number of seconds.
pub fn parse_interval(s: &str) -> Result<Duration> {
    let s = s.trim();
    let (num, unit) = match s.find(|c: char| !c.is_ascii_digit()) {
        Some(i) => s.split_at(i),
        None => (s, "s"),
    };
    let n: u64 = num
        .parse()
        .with_context(|| format!("interval {s:?}: expected <n>s, <n>m or <n>h"))?;
    let secs = match unit {
        "s" => n,
        "m" => n * 60,
        "h" => n * 3600,
        _ => bail!("interval {s:?}: expected <n>s, <n>m or <n>h"),
    };
    let d = Duration::from_secs(secs);
    if d < MIN_INTERVAL {
        bail!(
            "interval {s:?} is shorter than the {}s minimum",
            MIN_INTERVAL.as_secs()
        );
    }
    Ok(d)
}

/// A `gate` or `after` command as given: `None` when absent, an error when
/// empty, since an empty command would gate nothing and run nothing.
pub fn parse_command(key: &str, command: Option<&str>) -> Result<Option<String>> {
    match command {
        None => Ok(None),
        Some(c) if c.trim().is_empty() => bail!("{key} is empty: remove it or give a command"),
        Some(c) => Ok(Some(c.to_string())),
    }
}

/// The path made absolute against the current directory, resolving symlinks
/// when it exists (macOS `/var` vs `/private/var`).
pub fn absolute(p: &Path) -> PathBuf {
    let p = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    std::fs::canonicalize(&p).unwrap_or(p)
}

pub fn same_path(a: &Path, b: &Path) -> bool {
    absolute(a) == absolute(b)
}

impl Config {
    pub fn load(paths: &Paths) -> Result<Self> {
        let file = paths.config_file();
        match std::fs::read_to_string(&file) {
            Ok(s) => toml::from_str(&s).with_context(|| format!("reading {}", file.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", file.display())),
        }
    }

    /// Writes beside and renames, so a crash never leaves half a file.
    pub fn save(&self, paths: &Paths) -> Result<()> {
        let file = paths.config_file();
        std::fs::create_dir_all(&paths.config)
            .with_context(|| format!("creating {}", paths.config.display()))?;
        let body = format!(
            "# tether (ADR 0001): one [[repo]] per clone. Edited by `git tether add` / `set` / `remove`.\n\n{}",
            toml::to_string(self).context("serialising repos.toml")?
        );
        let tmp = file.with_extension("toml.tmp");
        std::fs::write(&tmp, body).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &file).with_context(|| format!("replacing {}", file.display()))
    }

    /// Every entry validated; the first bad one fails the whole file, so a typo
    /// never silently drops a repo from the sync.
    pub fn repos(&self, paths: &Paths) -> Result<Vec<Repo>> {
        let mut out: Vec<Repo> = Vec::new();
        for e in &self.repo {
            let r = resolve(e, paths).with_context(|| format!("repos.toml entry {}", e.path))?;
            if out.iter().any(|o| same_path(&o.path, &r.path)) {
                bail!("repos.toml lists {} twice", e.path);
            }
            out.push(r);
        }
        Ok(out)
    }
}

fn resolve(e: &RepoEntry, paths: &Paths) -> Result<Repo> {
    if e.branch.trim().is_empty() {
        bail!("branch is empty");
    }
    let path = paths.expand(&e.path);
    if !path.is_absolute() {
        bail!("path must be absolute or start with ~/");
    }
    Ok(Repo {
        path,
        branch: e.branch.clone(),
        mode: e.mode,
        remote: e.remote.clone().unwrap_or_else(|| DEFAULT_REMOTE.into()),
        interval: e
            .interval
            .as_deref()
            .map(parse_interval)
            .transpose()?
            .unwrap_or(DEFAULT_INTERVAL),
        url: e.url.clone(),
        gate: parse_command("gate", e.gate.as_deref())?,
        after: parse_command("after", e.after.as_deref())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_parse_with_units_and_a_floor() {
        assert_eq!(parse_interval("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_interval("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_interval("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_interval("120").unwrap(), Duration::from_secs(120));
        assert!(parse_interval("5s").is_err());
        assert!(parse_interval("5d").is_err());
        assert!(parse_interval("m").is_err());
    }

    #[test]
    fn entries_get_defaults_and_expanded_paths() {
        let paths = Paths::new(Path::new("/Users/me"), Path::new("/c"));
        let cfg: Config = toml::from_str(
            "[[repo]]\npath = \"~/Development/notes\"\nbranch = \"main\"\n\n\
             [[repo]]\npath = \"/srv/docs\"\nbranch = \"gh-pages\"\nmode = \"mirror\"\nremote = \"up\"\ninterval = \"1h\"\n",
        )
        .unwrap();
        let repos = cfg.repos(&paths).unwrap();
        assert_eq!(repos[0].path, PathBuf::from("/Users/me/Development/notes"));
        assert_eq!(
            (repos[0].mode, repos[0].remote.as_str(), repos[0].interval),
            (Mode::Follow, "origin", DEFAULT_INTERVAL)
        );
        assert_eq!(
            (repos[1].mode, repos[1].remote.as_str()),
            (Mode::Mirror, "up")
        );
        assert_eq!(repos[1].tracking_ref(), "refs/remotes/up/gh-pages");
        assert_eq!(repos[1].name(), "docs");
    }

    #[test]
    fn a_bad_entry_fails_the_file() {
        let paths = Paths::new(Path::new("/h"), Path::new("/c"));
        for bad in [
            "[[repo]]\npath = \"rel\"\nbranch = \"main\"\n",
            "[[repo]]\npath = \"/a\"\nbranch = \"main\"\ninterval = \"1s\"\n",
            "[[repo]]\npath = \"/a\"\nbranch = \"main\"\n[[repo]]\npath = \"/a\"\nbranch = \"dev\"\n",
            "[[repo]]\npath = \"/a\"\nbranch = \"main\"\ngate = \"\"\n",
            "[[repo]]\npath = \"/a\"\nbranch = \"main\"\nafter = \"  \"\n",
        ] {
            let cfg: Config = toml::from_str(bad).unwrap();
            assert!(cfg.repos(&paths).is_err(), "{bad}");
        }
        assert!(
            toml::from_str::<Config>(
                "[[repo]]\npath = \"/a\"\nbranch = \"main\"\nmode = \"rsync\"\n"
            )
            .is_err()
        );
        assert!(
            toml::from_str::<Config>(
                "[[repo]]\npath = \"/a\"\nbranch = \"main\"\nbefore = \"x\"\n"
            )
            .is_err()
        );
    }

    #[test]
    fn gate_and_after_are_read_as_written() {
        let paths = Paths::new(Path::new("/h"), Path::new("/c"));
        let cfg: Config = toml::from_str(
            "[[repo]]\npath = \"/a\"\nbranch = \"main\"\ngate = \"tool idle\"\nafter = \"tool reconcile\"\n\n\
             [[repo]]\npath = \"/b\"\nbranch = \"main\"\n",
        )
        .unwrap();
        let repos = cfg.repos(&paths).unwrap();
        assert_eq!(repos[0].gate.as_deref(), Some("tool idle"));
        assert_eq!(repos[0].after.as_deref(), Some("tool reconcile"));
        assert_eq!(
            (repos[1].gate.as_deref(), repos[1].after.as_deref()),
            (None, None)
        );
    }
}
