//! The tray's decisions without any UI: which requests a burst of clicks
//! coalesces into, how long the worker sleeps before the next repo is due,
//! which repos just flipped to *needs you*, which icon to show, and where the
//! popover opens. Kept apart from `tray` so it can be unit-tested.

use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use tether_core::config::Repo;
use tether_core::inspect::Health;
use tether_core::record::Records;
use tether_core::report::Report;

/// What woke the worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A repo's timer may be due.
    Tick,
    /// *Sync now* for one repo (its path as in `repos.toml`), or all with `None`.
    SyncNow(Option<String>),
    /// Rebuild the report (after Pause, Add, or an action elsewhere).
    Refresh,
}

/// Every request seen in one burst, coalesced so none is lost and no repo is
/// synced twice for two clicks.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Pending {
    pub all: bool,
    pub repos: BTreeSet<String>,
    pub tick: bool,
    pub refresh: bool,
}

impl Pending {
    pub fn of(e: Event) -> Self {
        let mut p = Self::default();
        p.absorb(e);
        p
    }

    pub fn absorb(&mut self, e: Event) {
        match e {
            Event::Tick => self.tick = true,
            Event::SyncNow(None) => self.all = true,
            Event::SyncNow(Some(r)) => {
                self.repos.insert(r);
            }
            Event::Refresh => self.refresh = true,
        }
    }

    /// Whether `key` must sync now on request (Pause does not apply to these).
    pub fn requested(&self, key: &str) -> bool {
        self.all || self.repos.contains(key)
    }
}

/// When `repo` is next due, from its last recorded sync.
fn due_in(repo: &Repo, last: Option<DateTime<Local>>, now: DateTime<Local>) -> Duration {
    let Some(last) = last else {
        return Duration::ZERO;
    };
    let age = now
        .signed_duration_since(last)
        .to_std()
        .unwrap_or(Duration::ZERO);
    repo.interval.saturating_sub(age)
}

/// How long the worker may sleep: until the soonest repo is due, but never
/// longer than `cap`, so a hand-edited `repos.toml` is picked up.
pub fn next_wait(
    repos: &[(Repo, String)],
    records: &Records,
    now: DateTime<Local>,
    cap: Duration,
) -> Duration {
    repos
        .iter()
        .map(|(r, key)| {
            due_in(
                r,
                records
                    .get(key)
                    .and_then(tether_core::record::Record::last_sync_time),
                now,
            )
        })
        .min()
        .unwrap_or(cap)
        .min(cap)
}

/// Repos that are *needs you* now but were not before, with their reason and
/// what to do about it.
/// `prev` is updated to the current health of every repo, and forgets
/// repos that were unlisted.
pub fn flips(prev: &mut HashMap<String, Health>, r: &Report) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut now = HashMap::new();
    for row in &r.repos {
        let was = prev.get(&row.path).copied();
        if row.health == Health::NeedsYou && was != Some(Health::NeedsYou) {
            let why = row.reason_text().unwrap_or_else(|| "needs you".into());
            let text = match &row.hint {
                Some(hint) => format!("{why}. {hint}"),
                None => why,
            };
            out.push((row.path.clone(), text));
        }
        now.insert(row.path.clone(), row.health);
    }
    *prev = now;
    out
}

/// The three icon states of rule 11.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    InSync,
    Syncing,
    NeedsYou,
}

/// *Needs you* wins over *syncing*: a sync of one repo must not hide another
/// that is stuck. A skipped-this-round repo shows as in sync.
pub fn icon_for(report: Option<&Report>, syncing: bool) -> Icon {
    match report {
        Some(r) if r.overall == Health::NeedsYou => Icon::NeedsYou,
        _ if syncing => Icon::Syncing,
        None => Icon::Syncing,
        Some(_) => Icon::InSync,
    }
}

/// A rectangle in physical pixels, top-left origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Area {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        (self.x..self.x + self.w).contains(&x) && (self.y..self.y + self.h).contains(&y)
    }
}

/// Where the popover's top-left corner goes: centred under the icon, `gap`
/// below it, and moved left or up as far as needed to stay on `screen`.
pub fn popover_origin(
    icon: &Area,
    size: (f64, f64),
    screen: Option<&Area>,
    gap: f64,
) -> (f64, f64) {
    let x = icon.x + icon.w / 2.0 - size.0 / 2.0;
    let y = icon.y + icon.h + gap;
    let Some(s) = screen else {
        return (x, y);
    };
    let clamp = |v: f64, lo: f64, hi: f64| v.min(hi).max(lo);
    (
        clamp(x, s.x, s.x + s.w - size.0),
        clamp(y, s.y, s.y + s.h - size.1),
    )
}

/// Whether a click on the icon may open the popover, given when it last hid
/// on focus loss: a click on the icon of an open popover blurs it first.
pub fn reopens(hidden_at: Option<Instant>, now: Instant, guard: Duration) -> bool {
    hidden_at.is_none_or(|t| now.saturating_duration_since(t) >= guard)
}

/// The root a scan walks after `left`, which is what the popover shows as "looking in…";
/// `None` after the last one (or for a root that is not in the list), when the scan is done.
pub fn root_after(roots: &[String], left: &str) -> Option<String> {
    let at = roots.iter().position(|r| r == left)?;
    roots.get(at + 1).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tether_core::config::Mode;
    use tether_core::inspect::Reason;
    use tether_core::record::Record;
    use tether_core::report::Row;

    fn repo(interval: u64) -> Repo {
        Repo {
            path: PathBuf::from("/h/r"),
            branch: "main".into(),
            mode: Mode::Follow,
            remote: "origin".into(),
            interval: Duration::from_secs(interval),
            url: None,
            gate: None,
            after: None,
        }
    }

    fn row(path: &str, health: Health) -> Row {
        Row {
            name: path.into(),
            path: path.into(),
            branch: "main".into(),
            mode: Mode::Follow,
            remote: "origin".into(),
            interval_seconds: 300,
            gate: None,
            after: None,
            last_sync: None,
            health,
            reason: (health == Health::NeedsYou).then_some(Reason::Detached),
            hint: (health == Health::NeedsYou).then(|| "Check out main and tether resumes.".into()),
            behind: None,
        }
    }

    fn report(rows: Vec<Row>) -> Report {
        let needs_you = rows.iter().filter(|r| r.health == Health::NeedsYou).count();
        let overall = if needs_you > 0 {
            Health::NeedsYou
        } else {
            Health::InSync
        };
        Report {
            paused: false,
            overall,
            needs_you,
            repos: rows,
        }
    }

    #[test]
    fn clicks_in_one_burst_coalesce() {
        let mut p = Pending::of(Event::SyncNow(Some("~/a".into())));
        p.absorb(Event::SyncNow(Some("~/a".into())));
        p.absorb(Event::Refresh);
        assert_eq!(p.repos.len(), 1);
        assert!(p.requested("~/a") && !p.requested("~/b") && p.refresh);
        p.absorb(Event::SyncNow(None));
        assert!(p.requested("~/b"));
    }

    #[test]
    fn opening_the_popover_refreshes_without_forcing_a_sync() {
        let p = Pending::of(Event::Refresh);
        assert!(p.refresh);
        assert!(!p.all && p.repos.is_empty() && !p.requested("~/a"));
    }

    #[test]
    fn the_worker_sleeps_until_the_soonest_repo_is_due() {
        let now = Local::now();
        let cap = Duration::from_secs(60);
        let mut records = Records::new();
        let rec = |ago: i64| Record {
            last_sync: (now - chrono::Duration::seconds(ago)).to_rfc3339(),
            health: Health::InSync,
            reason: None,
            tip: None,
            after_failed: None,
        };
        records.insert("a".into(), rec(280));
        records.insert("b".into(), rec(0));
        let repos = vec![(repo(300), "a".to_string()), (repo(300), "b".to_string())];
        let w = next_wait(&repos, &records, now, cap);
        assert!(
            w <= Duration::from_secs(20) && w >= Duration::from_secs(19),
            "{w:?}"
        );
        assert_eq!(
            next_wait(&[(repo(300), "never".into())], &records, now, cap),
            Duration::ZERO
        );
        assert_eq!(next_wait(&[], &records, now, cap), cap);
        assert_eq!(
            next_wait(&[(repo(3600), "b".into())], &records, now, cap),
            cap
        );
    }

    #[test]
    fn only_a_flip_to_needs_you_notifies() {
        let mut prev = HashMap::new();
        assert_eq!(
            flips(&mut prev, &report(vec![row("~/a", Health::NeedsYou)])).len(),
            1
        );
        assert!(
            flips(&mut prev, &report(vec![row("~/a", Health::NeedsYou)])).is_empty(),
            "still stuck: no repeat"
        );
        assert_eq!(
            flips(&mut prev, &report(vec![row("~/a", Health::InSync)])),
            Vec::<(String, String)>::new()
        );
        assert_eq!(
            flips(&mut prev, &report(vec![row("~/a", Health::NeedsYou)]))[0].1,
            "HEAD is detached. Check out main and tether resumes."
        );
        assert_eq!(
            flips(&mut prev, &report(vec![row("~/a", Health::Skipped)])),
            Vec::<(String, String)>::new()
        );
    }

    #[test]
    fn needs_you_wins_over_syncing() {
        let stuck = report(vec![row("~/a", Health::NeedsYou)]);
        let fine = report(vec![row("~/a", Health::InSync)]);
        assert_eq!(icon_for(Some(&stuck), true), Icon::NeedsYou);
        assert_eq!(icon_for(Some(&fine), true), Icon::Syncing);
        assert_eq!(icon_for(Some(&fine), false), Icon::InSync);
        assert_eq!(icon_for(None, false), Icon::Syncing);
    }

    #[test]
    fn the_popover_opens_centred_under_the_icon() {
        let icon = Area {
            x: 1000.0,
            y: 0.0,
            w: 40.0,
            h: 48.0,
        };
        let screen = Area {
            x: 0.0,
            y: 48.0,
            w: 2880.0,
            h: 1752.0,
        };
        assert_eq!(
            popover_origin(&icon, (720.0, 960.0), Some(&screen), 8.0),
            (660.0, 56.0)
        );
    }

    #[test]
    fn the_popover_stays_on_screen() {
        let screen = Area {
            x: 0.0,
            y: 48.0,
            w: 2880.0,
            h: 1752.0,
        };
        let right = Area {
            x: 2840.0,
            y: 0.0,
            w: 40.0,
            h: 48.0,
        };
        assert_eq!(
            popover_origin(&right, (720.0, 960.0), Some(&screen), 8.0),
            (2160.0, 56.0)
        );
        let left = Area {
            x: 10.0,
            y: 0.0,
            w: 40.0,
            h: 48.0,
        };
        assert!(
            popover_origin(&left, (720.0, 960.0), Some(&screen), 8.0)
                .0
                .abs()
                < f64::EPSILON
        );
        // A second display to the right: clamped to its own edges.
        let second = Area {
            x: 2880.0,
            y: 48.0,
            w: 1920.0,
            h: 1032.0,
        };
        let icon = Area {
            x: 2900.0,
            y: 0.0,
            w: 40.0,
            h: 48.0,
        };
        assert!(
            (popover_origin(&icon, (720.0, 960.0), Some(&second), 8.0).0 - 2880.0).abs()
                < f64::EPSILON
        );
        assert_eq!(
            popover_origin(&left, (720.0, 960.0), None, 8.0),
            (-330.0, 56.0)
        );
    }

    #[test]
    fn a_click_right_after_a_blur_does_not_reopen() {
        let guard = Duration::from_millis(300);
        let t = Instant::now();
        assert!(reopens(None, t, guard));
        assert!(!reopens(Some(t), t + Duration::from_millis(100), guard));
        assert!(reopens(Some(t), t + Duration::from_millis(400), guard));
    }

    #[test]
    fn the_scan_moves_to_the_root_after_the_one_just_left() {
        let roots: Vec<String> = ["~/Development", "~/Code", "~/Projects"]
            .map(String::from)
            .to_vec();
        assert_eq!(
            root_after(&roots, "~/Development").as_deref(),
            Some("~/Code")
        );
        assert_eq!(root_after(&roots, "~/Code").as_deref(), Some("~/Projects"));
        assert_eq!(root_after(&roots, "~/Projects"), None);
        assert_eq!(root_after(&roots, "~/Elsewhere"), None);
        assert_eq!(root_after(&[], "~/Code"), None);
    }
}
