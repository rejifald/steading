//! `repos.toml` (rules 1, 7, 10, 13): one `[[repo]]` per main clone, with the sweep settings
//! globally and per repo. Nothing in a clone that is not listed is ever touched.
//!
//! ```toml
//! interval = "5m"       # optional, every setting below too
//! policy = "auto"       # or "ask", "off"
//! grace = "24h"
//! stale_after = "14d"
//!
//! [[repo]]
//! path = "~/Development/app"
//! remote = "origin"
//! base = "main"         # default: the remote's default branch (<remote>/HEAD)
//! policy = "ask"        # overrides the global value for this repo
//! ```

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::paths::{Paths, absolute, same_path};

pub const DEFAULT_REMOTE: &str = "origin";
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// A pass runs `ls-remote` and `gh`; more often than this only adds load on the host.
pub const MIN_INTERVAL: Duration = Duration::from_secs(60);
pub const DEFAULT_GRACE: Duration = Duration::from_secs(24 * 3600);
pub const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(14 * 24 * 3600);

/// What reap does with a removable worktree (rule 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Policy {
    /// Remove once the grace period has passed.
    #[default]
    Auto,
    /// Notify, and remove only on confirm.
    Ask,
    /// Show, never remove.
    Off,
}

impl Policy {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Ask => "ask",
            Self::Off => "off",
        }
    }
}

/// One `[[repo]]` entry as written in the file. `policy`, `grace` and `stale_after` override
/// the file's global values.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoEntry {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<Policy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_after: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<Policy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_after: Option<String>,
    #[serde(default)]
    pub repo: Vec<RepoEntry>,
}

/// A validated entry with its path expanded and every default applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub path: PathBuf,
    pub remote: String,
    /// The base branch on the remote; `None` means the remote's default branch.
    pub base: Option<String>,
    pub policy: Policy,
    pub grace: Duration,
    pub stale_after: Duration,
}

impl Repo {
    /// The short name used in messages and `git reap sweep <repo>`: the folder's name.
    #[must_use]
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map_or_else(|| "repo".into(), |name| name.to_string_lossy().into_owned())
    }

    /// Whether `selector` (a name or a path) names this repo.
    #[must_use]
    pub fn matches(&self, selector: &str, paths: &Paths) -> bool {
        selector == self.name() || same_path(&absolute(&paths.expand(selector)), &self.path)
    }
}

/// `5m`, `90s`, `24h`, `14d`, or a bare number of seconds.
///
/// # Errors
///
/// Fails on any other form.
pub fn parse_duration(text: &str) -> Result<Duration> {
    let text = text.trim();
    let (number, unit) = text
        .find(|c: char| !c.is_ascii_digit())
        .map_or((text, "s"), |at| text.split_at(at));
    let count: u64 = number
        .parse()
        .with_context(|| format!("{text:?}: expected <n>s, <n>m, <n>h or <n>d"))?;
    let seconds_per_unit = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 24 * 3600,
        _ => bail!("{text:?}: expected <n>s, <n>m, <n>h or <n>d"),
    };
    Ok(Duration::from_secs(count * seconds_per_unit))
}

/// The pass interval, at least [`MIN_INTERVAL`].
///
/// # Errors
///
/// Fails on a malformed or too short interval.
pub fn parse_interval(text: &str) -> Result<Duration> {
    let interval = parse_duration(text).context("interval")?;
    if interval < MIN_INTERVAL {
        bail!(
            "interval {text:?} is shorter than the {}s minimum",
            MIN_INTERVAL.as_secs()
        );
    }
    Ok(interval)
}

impl Config {
    /// Reads `repos.toml`; a missing file is an empty list.
    ///
    /// # Errors
    ///
    /// Fails when the file cannot be read or is not valid.
    pub fn load(paths: &Paths) -> Result<Self> {
        let file = paths.config_file();
        match std::fs::read_to_string(&file) {
            Ok(text) => {
                toml::from_str(&text).with_context(|| format!("reading {}", file.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", file.display())),
        }
    }

    /// Writes beside and renames, so a crash never leaves half a file.
    ///
    /// # Errors
    ///
    /// Fails when the config folder cannot be written.
    pub fn save(&self, paths: &Paths) -> Result<()> {
        let file = paths.config_file();
        std::fs::create_dir_all(&paths.config)
            .with_context(|| format!("creating {}", paths.config.display()))?;
        let body = format!(
            "# reap (ADR 0001): one [[repo]] per main clone. Edited by `git reap add` / `git reap remove`.\n\n{}",
            toml::to_string(self).context("serialising repos.toml")?
        );
        let temporary = file.with_extension("toml.tmp");
        std::fs::write(&temporary, body)
            .with_context(|| format!("writing {}", temporary.display()))?;
        std::fs::rename(&temporary, &file).with_context(|| format!("replacing {}", file.display()))
    }

    /// The pass interval.
    ///
    /// # Errors
    ///
    /// Fails on a malformed or too short interval.
    pub fn interval(&self) -> Result<Duration> {
        self.interval
            .as_deref()
            .map_or(Ok(DEFAULT_INTERVAL), parse_interval)
    }

    /// Every entry validated; the first bad one fails the whole file, so a typo never quietly
    /// changes what reap may remove.
    ///
    /// # Errors
    ///
    /// Fails on the first invalid or duplicate entry.
    pub fn repos(&self, paths: &Paths) -> Result<Vec<Repo>> {
        self.interval()?;
        let mut repos: Vec<Repo> = Vec::new();
        for entry in &self.repo {
            let repo = self
                .resolve(entry, paths)
                .with_context(|| format!("repos.toml entry {}", entry.path))?;
            if repos.iter().any(|other| same_path(&other.path, &repo.path)) {
                bail!("repos.toml lists {} twice", entry.path);
            }
            repos.push(repo);
        }
        Ok(repos)
    }

    fn resolve(&self, entry: &RepoEntry, paths: &Paths) -> Result<Repo> {
        let path = paths.expand(&entry.path);
        if !path.is_absolute() {
            bail!("path must be absolute or start with ~/");
        }
        let duration = |name: &str, own: &Option<String>, global: &Option<String>, default| {
            own.as_ref()
                .or(global.as_ref())
                .map_or(Ok(default), |text| {
                    parse_duration(text).context(name.to_owned())
                })
        };
        Ok(Repo {
            path,
            remote: entry
                .remote
                .clone()
                .unwrap_or_else(|| DEFAULT_REMOTE.into()),
            base: entry.base.clone().filter(|base| !base.trim().is_empty()),
            policy: entry.policy.or(self.policy).unwrap_or_default(),
            grace: duration("grace", &entry.grace, &self.grace, DEFAULT_GRACE)?,
            stale_after: duration(
                "stale_after",
                &entry.stale_after,
                &self.stale_after,
                DEFAULT_STALE_AFTER,
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn durations_parse_with_units() {
        assert_eq!(parse_duration("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_duration("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(parse_duration("24h").unwrap(), DEFAULT_GRACE);
        assert_eq!(parse_duration("14d").unwrap(), DEFAULT_STALE_AFTER);
        assert_eq!(parse_duration("120").unwrap(), Duration::from_secs(120));
        assert!(parse_duration("5w").is_err());
        assert!(parse_duration("m").is_err());
    }

    #[test]
    fn the_interval_has_a_one_minute_floor() {
        assert!(parse_interval("30s").is_err());
        assert_eq!(parse_interval("1m").unwrap(), MIN_INTERVAL);
    }

    #[test]
    fn entries_take_their_own_settings_then_the_global_ones_then_the_defaults() {
        let paths = Paths::new(Path::new("/Users/me"), Path::new("/c"));
        let config: Config = toml::from_str(
            "grace = \"2h\"\npolicy = \"ask\"\n\n\
             [[repo]]\npath = \"~/Development/app\"\n\n\
             [[repo]]\npath = \"/srv/api\"\nremote = \"up\"\nbase = \"develop\"\npolicy = \"off\"\nstale_after = \"3d\"\n",
        )
        .unwrap();

        let repos = config.repos(&paths).unwrap();

        assert_eq!(repos[0].path, PathBuf::from("/Users/me/Development/app"));
        assert_eq!(
            (repos[0].remote.as_str(), &repos[0].base, repos[0].policy),
            ("origin", &None, Policy::Ask)
        );
        assert_eq!(repos[0].grace, Duration::from_secs(7200));
        assert_eq!(repos[0].stale_after, DEFAULT_STALE_AFTER);
        assert_eq!(repos[1].base.as_deref(), Some("develop"));
        assert_eq!(repos[1].policy, Policy::Off);
        assert_eq!(repos[1].stale_after, Duration::from_secs(3 * 24 * 3600));
        assert_eq!(repos[1].name(), "api");
    }

    #[test]
    fn a_bad_entry_fails_the_whole_file() {
        let paths = Paths::new(Path::new("/h"), Path::new("/c"));
        for bad in [
            "[[repo]]\npath = \"relative\"\n",
            "[[repo]]\npath = \"/a\"\ngrace = \"soon\"\n",
            "interval = \"10s\"\n[[repo]]\npath = \"/a\"\n",
            "[[repo]]\npath = \"/a\"\n[[repo]]\npath = \"/a\"\n",
        ] {
            let config: Config = toml::from_str(bad).unwrap();
            assert!(config.repos(&paths).is_err(), "{bad}");
        }
        assert!(
            toml::from_str::<Config>("[[repo]]\npath = \"/a\"\npolicy = \"sometimes\"\n").is_err()
        );
        assert!(toml::from_str::<Config>("[[repo]]\npath = \"/a\"\nbranch = \"main\"\n").is_err());
    }

    #[test]
    fn a_saved_file_reads_back_the_same() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path(), &dir.path().join("cfg"));
        let config = Config {
            repo: vec![RepoEntry {
                path: "~/Development/app".into(),
                base: Some("main".into()),
                ..RepoEntry::default()
            }],
            ..Config::default()
        };

        config.save(&paths).unwrap();

        assert_eq!(Config::load(&paths).unwrap().repo, config.repo);
    }
}
