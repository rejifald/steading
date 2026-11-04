//! `repos.toml` (rules 1, 7, 10, 13 and ADRs dirty-merged-worktrees-are-salvaged and
//! low-disk-skips-the-grace): one `[[repo]]`
//! per main clone, with the sweep settings globally and per repo. Nothing in a clone that is not
//! listed is ever touched.
//!
//! ```toml
//! interval = "5m"       # optional, every setting below too
//! policy = "auto"       # or "ask", "off"
//! grace = "24h"
//! stale_after = "14d"
//! dirty_grace = "7d"    # a merged worktree with changes: unchanged this long, then saved
//! salvage_max_untracked = "100MB"
//! salvage_max_age = "30d"
//! low_disk = "10%"      # or a size ("20GB"), or "off": below this, removable worktrees go at once
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
pub const DEFAULT_DIRTY_GRACE: Duration = Duration::from_secs(7 * 24 * 3600);
pub const DEFAULT_SALVAGE_MAX_UNTRACKED: u64 = 100_000_000;
pub const DEFAULT_SALVAGE_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 3600);
pub const DEFAULT_LOW_DISK: LowDisk = LowDisk::Percent(10);

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

/// When a volume counts as low (ADR low-disk-skips-the-grace): the free space below which a
/// removable worktree goes without waiting out its grace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LowDisk {
    /// Free space below this share of the volume, in whole percent (1 to 50).
    Percent(u8),
    /// Free space below this many bytes.
    Size(u64),
    /// Never low: the grace always applies.
    Off,
}

impl LowDisk {
    /// `10%`, a size such as `20GB`, or `off`.
    ///
    /// # Errors
    ///
    /// Fails on any other form, on a percentage outside 1 to 50, and on a zero size.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("off") {
            return Ok(Self::Off);
        }
        let form = || {
            format!(
                "{text:?}: expected a percentage such as \"10%\", a size such as \"20GB\", or \"off\""
            )
        };
        if let Some(number) = text.strip_suffix('%') {
            let percent: u8 = number.trim().parse().with_context(form)?;
            if !(1..=50).contains(&percent) {
                bail!("{text:?}: a percentage must be from 1% to 50%");
            }
            return Ok(Self::Percent(percent));
        }
        match parse_size(text).with_context(form)? {
            0 => bail!("{text:?}: a size must be more than 0; use \"off\" to turn it off"),
            bytes => Ok(Self::Size(bytes)),
        }
    }

    /// The free space, in bytes, below which a volume of `total` bytes is low; `None` when off.
    #[must_use]
    pub fn threshold(self, total: u64) -> Option<u64> {
        match self {
            Self::Percent(percent) => {
                let bytes = u128::from(total) * u128::from(percent) / 100;
                Some(u64::try_from(bytes).unwrap_or(u64::MAX))
            }
            Self::Size(bytes) => Some(bytes),
            Self::Off => None,
        }
    }

    /// The free space at which a pressure pass stops: 1.5 times [`LowDisk::threshold`].
    #[must_use]
    pub fn healthy_at(self, total: u64) -> Option<u64> {
        self.threshold(total)
            .map(|threshold| threshold.saturating_add(threshold / 2))
    }

    /// Whether `available` bytes free is low on a volume of `total` bytes.
    #[must_use]
    pub fn is_low(self, available: u64, total: u64) -> bool {
        self.threshold(total)
            .is_some_and(|threshold| available < threshold)
    }

    /// As written in `repos.toml`, for messages.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Percent(percent) => format!("{percent}%"),
            Self::Size(bytes) => crate::report::shown_size(bytes),
            Self::Off => "off".into(),
        }
    }
}

/// One `[[repo]]` entry as written in the file. Every setting but `path`, `remote` and `base`
/// overrides the file's global value.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty_grace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salvage_max_untracked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salvage_max_age: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_disk: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty_grace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salvage_max_untracked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salvage_max_age: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_disk: Option<String>,
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
    /// How long a merged worktree's changes must stay untouched before they are saved to a ref
    /// and it becomes removable.
    pub dirty_grace: Duration,
    /// The most untracked data, in bytes, a save copies into the clone's objects.
    pub salvage_max_untracked: u64,
    /// How long a salvage ref is kept before a sweep deletes it.
    pub salvage_max_age: Duration,
    /// When a volume holding this repo's worktrees counts as low.
    pub low_disk: LowDisk,
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

/// `100MB`, `1GB`, `500kB`, or a bare number of bytes; decimal units, as Finder counts.
///
/// # Errors
///
/// Fails on any other form.
pub fn parse_size(text: &str) -> Result<u64> {
    let text = text.trim();
    let (number, unit) = text
        .find(|c: char| !c.is_ascii_digit())
        .map_or((text, ""), |at| text.split_at(at));
    let count: u64 = number
        .parse()
        .with_context(|| format!("{text:?}: expected <n>, <n>kB, <n>MB or <n>GB"))?;
    let bytes_per_unit: u64 = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" => 1_000,
        "m" | "mb" => 1_000_000,
        "g" | "gb" => 1_000_000_000,
        _ => bail!("{text:?}: expected <n>, <n>kB, <n>MB or <n>GB"),
    };
    count
        .checked_mul(bytes_per_unit)
        .with_context(|| format!("{text:?} is too large"))
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
            dirty_grace: duration(
                "dirty_grace",
                &entry.dirty_grace,
                &self.dirty_grace,
                DEFAULT_DIRTY_GRACE,
            )?,
            salvage_max_untracked: entry
                .salvage_max_untracked
                .as_ref()
                .or(self.salvage_max_untracked.as_ref())
                .map_or(Ok(DEFAULT_SALVAGE_MAX_UNTRACKED), |text| {
                    parse_size(text).context("salvage_max_untracked")
                })?,
            salvage_max_age: duration(
                "salvage_max_age",
                &entry.salvage_max_age,
                &self.salvage_max_age,
                DEFAULT_SALVAGE_MAX_AGE,
            )?,
            low_disk: entry
                .low_disk
                .as_ref()
                .or(self.low_disk.as_ref())
                .map_or(Ok(DEFAULT_LOW_DISK), |text| {
                    LowDisk::parse(text).context("low_disk")
                })?,
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
    fn sizes_parse_with_decimal_units() {
        assert_eq!(parse_size("100MB").unwrap(), DEFAULT_SALVAGE_MAX_UNTRACKED);
        assert_eq!(parse_size("1GB").unwrap(), 1_000_000_000);
        assert_eq!(parse_size("500kB").unwrap(), 500_000);
        assert_eq!(parse_size("2 mb").unwrap(), 2_000_000);
        assert_eq!(parse_size("4096").unwrap(), 4096);
        assert!(parse_size("5TB").is_err());
        assert!(parse_size("MB").is_err());
        assert!(parse_size("99999999999999GB").is_err());
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
        assert_eq!(repos[0].dirty_grace, DEFAULT_DIRTY_GRACE);
        assert_eq!(
            repos[0].salvage_max_untracked,
            DEFAULT_SALVAGE_MAX_UNTRACKED
        );
        assert_eq!(repos[0].salvage_max_age, DEFAULT_SALVAGE_MAX_AGE);
    }

    #[test]
    fn the_salvage_settings_are_read_globally_and_per_repo() {
        let paths = Paths::new(Path::new("/Users/me"), Path::new("/c"));
        let config: Config = toml::from_str(
            "dirty_grace = \"3d\"\nsalvage_max_untracked = \"10MB\"\n\n\
             [[repo]]\npath = \"/a\"\n\n\
             [[repo]]\npath = \"/b\"\ndirty_grace = \"12h\"\nsalvage_max_age = \"60d\"\n",
        )
        .unwrap();

        let repos = config.repos(&paths).unwrap();

        assert_eq!(repos[0].dirty_grace, Duration::from_secs(3 * 24 * 3600));
        assert_eq!(repos[0].salvage_max_untracked, 10_000_000);
        assert_eq!(repos[1].dirty_grace, Duration::from_secs(12 * 3600));
        assert_eq!(repos[1].salvage_max_untracked, 10_000_000);
        assert_eq!(
            repos[1].salvage_max_age,
            Duration::from_secs(60 * 24 * 3600)
        );
    }

    #[test]
    fn low_disk_is_a_percentage_a_size_or_off() {
        assert_eq!(LowDisk::parse("10%").unwrap(), LowDisk::Percent(10));
        assert_eq!(LowDisk::parse(" 5 % ").unwrap(), LowDisk::Percent(5));
        assert_eq!(
            LowDisk::parse("20GB").unwrap(),
            LowDisk::Size(20_000_000_000)
        );
        assert_eq!(LowDisk::parse("OFF").unwrap(), LowDisk::Off);
        for bad in [
            "", "%", "0%", "51%", "10.5%", "-5%", "lots", "20TB", "0", "0GB",
        ] {
            assert!(LowDisk::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn low_disk_levels_follow_the_volume_size() {
        let total = 1_000_000_000_000;

        assert_eq!(LowDisk::Percent(10).threshold(total), Some(100_000_000_000));
        assert_eq!(
            LowDisk::Percent(10).healthy_at(total),
            Some(150_000_000_000)
        );
        assert_eq!(
            LowDisk::Size(20_000_000_000).threshold(total),
            Some(20_000_000_000)
        );
        assert_eq!(LowDisk::Off.threshold(total), None);
        assert!(LowDisk::Percent(10).is_low(99_999_999_999, total));
        assert!(!LowDisk::Percent(10).is_low(100_000_000_000, total));
        assert!(!LowDisk::Off.is_low(0, total));
    }

    #[test]
    fn low_disk_is_read_globally_and_per_repo_and_defaults_to_ten_percent() {
        let paths = Paths::new(Path::new("/Users/me"), Path::new("/c"));
        let config: Config = toml::from_str(
            "low_disk = \"15%\"\n\n\
             [[repo]]\npath = \"/a\"\n\n\
             [[repo]]\npath = \"/b\"\nlow_disk = \"off\"\n\n\
             [[repo]]\npath = \"/c\"\nlow_disk = \"20GB\"\n",
        )
        .unwrap();
        let defaults: Config = toml::from_str("[[repo]]\npath = \"/a\"\n").unwrap();

        let repos = config.repos(&paths).unwrap();

        assert_eq!(repos[0].low_disk, LowDisk::Percent(15));
        assert_eq!(repos[1].low_disk, LowDisk::Off);
        assert_eq!(repos[2].low_disk, LowDisk::Size(20_000_000_000));
        assert_eq!(
            defaults.repos(&paths).unwrap()[0].low_disk,
            DEFAULT_LOW_DISK
        );
    }

    #[test]
    fn a_bad_entry_fails_the_whole_file() {
        let paths = Paths::new(Path::new("/h"), Path::new("/c"));
        for bad in [
            "[[repo]]\npath = \"relative\"\n",
            "[[repo]]\npath = \"/a\"\ngrace = \"soon\"\n",
            "[[repo]]\npath = \"/a\"\nsalvage_max_untracked = \"lots\"\n",
            "[[repo]]\npath = \"/a\"\nlow_disk = \"soon\"\n",
            "low_disk = \"99%\"\n[[repo]]\npath = \"/a\"\n",
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
