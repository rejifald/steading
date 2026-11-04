//! `repos.toml` (rule 1): one `[[repo]]` per clone. Nothing that is not listed
//! is ever synced.
//!
//! ```toml
//! scan = ["~/Development", "~/Code"]  # optional (repository ADR 0004 rule 4): where `discover` looks; [] turns it off
//! skip = ["~/Development/scratch"]    # optional (repository ADR 0004 rule 3): clones `discover` never suggests
//!
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
    #[must_use]
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
    /// Where clone discovery looks. Absent means the default roots; `[]` means nowhere.
    /// Declared before `repo`: TOML writes the tables last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan: Option<Vec<String>>,
    /// Clones discovery never suggests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip: Vec<String>,
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
    #[must_use]
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map_or_else(|| "repo".into(), |n| n.to_string_lossy().into_owned())
    }

    #[must_use]
    pub fn tracking_ref(&self) -> String {
        format!("refs/remotes/{}/{}", self.remote, self.branch)
    }

    /// Whether `selector` (a name or a path) names this repo.
    #[must_use]
    pub fn matches(&self, selector: &str, paths: &Paths) -> bool {
        if selector == self.name() {
            return true;
        }
        let wanted = absolute(&paths.expand(selector));
        same_path(&wanted, &self.path)
    }
}

/// `5m`, `90s`, `1h`, or a bare number of seconds.
///
/// # Errors
///
/// Fails when `s` is empty, has no number, has an unknown unit, or is zero.
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
///
/// # Errors
///
/// Fails when `command` is present but empty.
pub fn parse_command(key: &str, command: Option<&str>) -> Result<Option<String>> {
    match command {
        None => Ok(None),
        Some(c) if c.trim().is_empty() => bail!("{key} is empty: remove it or give a command"),
        Some(c) => Ok(Some(c.to_string())),
    }
}

/// The path made absolute against the current directory, resolving symlinks
/// when it exists (macOS `/var` vs `/private/var`).
#[must_use]
pub fn absolute(p: &Path) -> PathBuf {
    let p = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    std::fs::canonicalize(&p).unwrap_or(p)
}

#[must_use]
pub fn same_path(a: &Path, b: &Path) -> bool {
    absolute(a) == absolute(b)
}

impl Config {
    /// # Errors
    ///
    /// Fails when the file exists but cannot be read or is not valid TOML.
    pub fn load(paths: &Paths) -> Result<Self> {
        let file = paths.config_file();
        match std::fs::read_to_string(&file) {
            Ok(s) => toml::from_str(&s).with_context(|| format!("reading {}", file.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", file.display())),
        }
    }

    /// Writes beside and renames, so a crash never leaves half a file.
    ///
    /// # Errors
    ///
    /// Fails when the config directory or file cannot be written or renamed into place.
    pub fn save(&self, paths: &Paths) -> Result<()> {
        let file = paths.config_file();
        std::fs::create_dir_all(&paths.config)
            .with_context(|| format!("creating {}", paths.config.display()))?;
        let body = format!(
            "# tether (ADR 0001): one [[repo]] per clone. Edited by `git tether add` / `set` / `remove`.\n\
             # Optional: scan = [\"~/Development\"] (where `discover` looks) and skip = [...] (clones it never suggests).\n\n{}",
            toml::to_string(self).context("serialising repos.toml")?
        );
        let tmp = file.with_extension("toml.tmp");
        std::fs::write(&tmp, body).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &file).with_context(|| format!("replacing {}", file.display()))
    }

    /// Every entry validated; the first bad one fails the whole file, so a typo
    /// never silently drops a repo from the sync.
    ///
    /// # Errors
    ///
    /// Fails when any entry is invalid, two entries list the same path, or a `scan` or `skip` entry is not an absolute or `~/` path.
    pub fn repos(&self, paths: &Paths) -> Result<Vec<Repo>> {
        self.scan_roots(paths)?;
        self.skipped(paths)?;
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

impl Config {
    /// The `scan` entries expanded, or `None` while `scan` is absent (the defaults apply).
    ///
    /// # Errors
    ///
    /// Fails, naming the entry, when one is not an absolute or `~/` path.
    pub fn scan_roots(&self, paths: &Paths) -> Result<Option<Vec<PathBuf>>> {
        self.scan
            .as_ref()
            .map(|list| expand_list("scan", list, paths))
            .transpose()
    }

    /// The `skip` entries expanded.
    ///
    /// # Errors
    ///
    /// Fails, naming the entry, when one is not an absolute or `~/` path.
    pub fn skipped(&self, paths: &Paths) -> Result<Vec<PathBuf>> {
        expand_list("skip", &self.skip, paths)
    }
}

fn expand_list(key: &str, list: &[String], paths: &Paths) -> Result<Vec<PathBuf>> {
    list.iter()
        .map(|e| {
            let path = paths.expand(e);
            if path.is_absolute() {
                Ok(path)
            } else {
                bail!("repos.toml {key} entry {e}: path must be absolute or start with ~/")
            }
        })
        .collect()
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

    #[test]
    fn a_file_without_scan_or_skip_round_trips_byte_for_byte() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Path::new("/Users/me"), dir.path());
        let cfg: Config = toml::from_str(
            "[[repo]]\npath = \"~/Development/notes\"\nbranch = \"main\"\n\n\
             [[repo]]\npath = \"/srv/docs\"\nbranch = \"gh-pages\"\nmode = \"mirror\"\n",
        )
        .unwrap();
        cfg.save(&paths).unwrap();
        let first = std::fs::read_to_string(paths.config_file()).unwrap();
        assert!(
            !first.contains("\nscan") && !first.contains("\nskip"),
            "{first}"
        );
        Config::load(&paths).unwrap().save(&paths).unwrap();
        assert_eq!(std::fs::read_to_string(paths.config_file()).unwrap(), first);
    }

    #[test]
    fn scan_and_skip_round_trip_and_come_before_the_tables() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(Path::new("/Users/me"), dir.path());
        let cfg: Config = toml::from_str(
            "scan = [\"~/Code\", \"/srv\"]\nskip = [\"~/Code/scratch\"]\n\n[[repo]]\npath = \"/a\"\nbranch = \"main\"\n",
        )
        .unwrap();
        cfg.save(&paths).unwrap();
        let first = std::fs::read_to_string(paths.config_file()).unwrap();
        assert!(
            first.find("\nscan = ").unwrap() < first.find("\n[[repo]]").unwrap(),
            "{first}"
        );
        assert!(
            first.find("\nskip = ").unwrap() < first.find("\n[[repo]]").unwrap(),
            "{first}"
        );
        let back = Config::load(&paths).unwrap();
        assert_eq!(
            back.scan,
            Some(vec!["~/Code".to_string(), "/srv".to_string()])
        );
        assert_eq!(back.skip, ["~/Code/scratch"]);
        back.save(&paths).unwrap();
        assert_eq!(std::fs::read_to_string(paths.config_file()).unwrap(), first);
        assert_eq!(
            back.scan_roots(&paths).unwrap().unwrap(),
            [PathBuf::from("/Users/me/Code"), PathBuf::from("/srv")]
        );
        assert_eq!(
            back.skipped(&paths).unwrap(),
            [PathBuf::from("/Users/me/Code/scratch")]
        );
    }

    #[test]
    fn an_empty_scan_is_kept_and_differs_from_an_absent_one() {
        let paths = Paths::new(Path::new("/h"), Path::new("/c"));
        let off: Config = toml::from_str("scan = []\n").unwrap();
        assert_eq!(off.scan_roots(&paths).unwrap(), Some(vec![]));
        assert!(toml::to_string(&off).unwrap().contains("scan = []"));
        let absent: Config = toml::from_str("").unwrap();
        assert_eq!(absent.scan_roots(&paths).unwrap(), None);
    }

    #[test]
    fn a_bad_scan_or_skip_entry_fails_the_file_naming_it() {
        let paths = Paths::new(Path::new("/h"), Path::new("/c"));
        for (bad, named) in [
            ("scan = [\"code\"]\n", "code"),
            ("scan = [\"/ok\", \"../up\"]\n", "../up"),
            ("skip = [\"scratch\"]\n", "scratch"),
        ] {
            let cfg: Config = toml::from_str(bad).unwrap();
            let e = format!("{:#}", cfg.repos(&paths).unwrap_err());
            assert!(e.contains(named), "{e}");
        }
        assert!(toml::from_str::<Config>("scan = \"~/Code\"\n").is_err());
        assert!(toml::from_str::<Config>("watch = []\n").is_err());
    }
}
