//! The tray's decisions without any UI: which worktrees just started to need the owner, which
//! icon to show, and where the popover opens. Kept apart from `tray` so it can be unit-tested.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use reap_core::classify::State;
use reap_core::pressure::{Holder, Pressure};
use reap_core::report::{Overall, Report, shown_size};
use reap_core::sweep::Outcome;

/// More notifications than this in one pass become one summary, so a first pass over a clone
/// with a pile of merged worktrees does not flood Notification Center.
const MAX_NOTIFICATIONS: usize = 3;

/// What wakes the worker early; the pass interval passing wakes it too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Rebuild the report now (the popover opened, *Refresh*, *Pause*, *Add*).
    Refresh,
}

/// Paths that need the owner now but did not before, with why. `previous` is replaced by the
/// current set, so a path that recovers and breaks again is reported again.
pub fn new_needs_you(previous: &mut HashSet<String>, report: &Report) -> Vec<(String, String)> {
    let mut current = HashSet::new();
    let mut fresh = Vec::new();
    for repo in &report.repos {
        let mut found = |path: &str, why: &str| {
            if !previous.contains(path) {
                fresh.push((path.to_string(), why.to_string()));
            }
            current.insert(path.to_string());
        };
        if let Some(error) = &repo.error {
            found(&repo.path, error);
        }
        for row in repo
            .worktrees
            .iter()
            .filter(|row| row.state == State::Stale)
        {
            found(&row.path, &row.reason);
        }
    }
    *previous = current;
    fresh
}

/// The most holders a "still low" notice names; the rest are counted.
const MAX_HOLDERS: usize = 3;

/// A worktree's folder name, from its path as shown.
fn folder_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// What the pressure pass tells the owner about each volume (ADR low-disk-skips-the-grace):
/// what it removed, once per episode that nothing is left to remove and what holds the space,
/// and the `policy = "ask"` worktrees that could go.
pub fn pressure_notifications(volumes: &[Pressure]) -> Vec<(String, String)> {
    const TITLE: &str = "reap: low disk";
    let mut messages = Vec::new();
    for pressure in volumes {
        let name = &pressure.volume.name;
        if !pressure.removed.is_empty() {
            let names: Vec<&str> = pressure
                .removed
                .iter()
                .map(|reclaimed| folder_name(&reclaimed.removed.path))
                .collect();
            messages.push((
                TITLE.to_string(),
                format!(
                    "Low disk on {name}: removed {}, freed {}: {}",
                    plural(names.len(), "worktree"),
                    shown_size(pressure.freed_bytes),
                    names.join(", ")
                ),
            ));
        }
        if pressure.announce_stuck {
            messages.push((TITLE.to_string(), still_low(pressure)));
        }
        if pressure.announce_asked {
            let names: Vec<&str> = pressure
                .asked
                .iter()
                .map(|holder| folder_name(&holder.path))
                .collect();
            messages.push((
                TITLE.to_string(),
                format!(
                    "Low disk on {name}: {} can be removed; confirm with Remove now",
                    names.join(", ")
                ),
            ));
        }
        for refused in &pressure.refused {
            messages.push((
                "reap needs you".to_string(),
                format!(
                    "git refused to remove {}: {}",
                    refused.path, refused.message
                ),
            ));
        }
    }
    messages
}

/// "Low disk on X: 4.1 GB free, and nothing reap can remove. Holding space: …".
fn still_low(pressure: &Pressure) -> String {
    let volume = &pressure.volume;
    let holder = |holder: &Holder| {
        let size = holder
            .bytes
            .map(|bytes| format!(" {}", shown_size(bytes)))
            .unwrap_or_default();
        format!("{}{size} ({})", folder_name(&holder.path), holder.why)
    };
    let named: Vec<String> = volume
        .holders
        .iter()
        .take(MAX_HOLDERS)
        .map(holder)
        .collect();
    let more = volume.holders.len().saturating_sub(MAX_HOLDERS);
    let holding = match (named.is_empty(), more) {
        (true, _) => String::new(),
        (false, 0) => format!(" Holding space: {}.", named.join(", ")),
        (false, more) => format!(" Holding space: {}, and {more} more.", named.join(", ")),
    };
    format!(
        "Low disk on {}: {} free, and nothing reap can remove.{holding}",
        volume.name,
        shown_size(volume.available_bytes)
    )
}

/// What a pass tells the owner (rules 10 and 11): a grace clock started, a removal, a refusal,
/// and the pressure pass's notices. A clock the pressure pass already ran out is not announced.
pub fn notifications(outcomes: &[Outcome], volumes: &[Pressure]) -> Vec<(String, String)> {
    let mut messages = Vec::new();
    let is_reclaimed = |path: &str| {
        volumes
            .iter()
            .flat_map(|pressure| &pressure.removed)
            .any(|reclaimed| reclaimed.removed.path == path)
    };
    for outcome in outcomes {
        for started in outcome
            .started
            .iter()
            .filter(|started| !is_reclaimed(&started.path))
        {
            let when = started.removes_at.map_or_else(
                || "confirm with Remove now".to_string(),
                |time| {
                    format!(
                        "removes at {}",
                        time.with_timezone(&chrono::Local).format("%a %H:%M")
                    )
                },
            );
            messages.push((
                "reap: worktree to remove".into(),
                format!("{}: {}; {when}", started.path, started.reason),
            ));
        }
        for removed in &outcome.removed {
            let branch = match (&removed.branch, &removed.branch_kept) {
                (Some(branch), Some(_)) => format!("; branch {branch} kept"),
                (Some(branch), None) => format!(" and branch {branch}"),
                (None, _) => String::new(),
            };
            let salvaged = removed
                .salvaged_to
                .as_deref()
                .map(|refname| format!("; changes saved to {refname}"))
                .unwrap_or_default();
            messages.push((
                "reap removed a worktree".into(),
                format!("{}{branch}{salvaged}", removed.path),
            ));
        }
        for refused in &outcome.refused {
            messages.push((
                "reap needs you".into(),
                format!(
                    "git refused to remove {}: {}",
                    refused.path, refused.message
                ),
            ));
        }
    }
    messages.extend(pressure_notifications(volumes));
    messages
}

/// The messages as they are, or one summary when there are too many.
pub fn batched(messages: Vec<(String, String)>) -> Vec<(String, String)> {
    if messages.len() <= MAX_NOTIFICATIONS {
        return messages;
    }
    let count = |title: &str| messages.iter().filter(|(own, _)| own == title).count();
    let parts: Vec<String> = [
        ("reap: worktree to remove", "to remove"),
        ("reap removed a worktree", "removed"),
        ("reap: low disk", "low disk"),
        ("reap needs you", "need you"),
    ]
    .iter()
    .map(|(title, word)| (count(title), word))
    .filter(|(number, _)| *number > 0)
    .map(|(number, word)| format!("{number} {word}"))
    .collect();
    vec![(
        "reap".into(),
        format!("Worktrees: {}. Open reap to see them.", parts.join(", ")),
    )]
}

/// The three icon states of rule 15.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Tidy,
    Pending,
    NeedsYou,
}

/// The icon for a report; before the first one, *pending*, the outline, as "still looking".
pub fn icon_for(report: Option<&Report>) -> Icon {
    match report.map(|report| report.overall) {
        Some(Overall::Tidy) => Icon::Tidy,
        Some(Overall::NeedsYou) => Icon::NeedsYou,
        Some(Overall::Pending) | None => Icon::Pending,
    }
}

/// A rectangle in physical pixels, top-left origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Area {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        (self.x..self.x + self.width).contains(&x) && (self.y..self.y + self.height).contains(&y)
    }
}

/// Where the popover's top-left corner goes: centred under the icon, `gap` below it, and moved
/// left or up as far as needed to stay on `screen`.
pub fn popover_origin(
    icon: &Area,
    size: (f64, f64),
    screen: Option<&Area>,
    gap: f64,
) -> (f64, f64) {
    let x = icon.x + icon.width / 2.0 - size.0 / 2.0;
    let y = icon.y + icon.height + gap;
    let Some(screen) = screen else {
        return (x, y);
    };
    let clamp = |value: f64, low: f64, high: f64| value.min(high).max(low);
    (
        clamp(x, screen.x, screen.x + screen.width - size.0),
        clamp(y, screen.y, screen.y + screen.height - size.1),
    )
}

/// Whether a click on the icon may open the popover, given when it last hid on focus loss: a
/// click on the icon of an open popover blurs it first.
pub fn reopens(hidden_at: Option<Instant>, now: Instant, guard: Duration) -> bool {
    hidden_at.is_none_or(|time| now.saturating_duration_since(time) >= guard)
}

#[cfg(test)]
mod tests {
    use reap_core::pressure::{Reclaimed, VolumeStatus};
    use reap_core::report::{RepoReport, Row};
    use reap_core::sweep::{Refused, Removed, Started};

    use super::*;

    fn row(path: &str, state: State) -> Row {
        Row {
            name: path.into(),
            path: path.into(),
            branch: Some("feat".into()),
            head: None,
            pr: None,
            state,
            reason: "pull request #5 closed without merging".into(),
            last_activity: None,
            removes_at: None,
            saves_at: None,
            size_bytes: None,
        }
    }

    fn report(worktrees: Vec<Row>, error: Option<&str>) -> Report {
        Report {
            paused: false,
            overall: Overall::NeedsYou,
            removable: 0,
            needs_you: 1,
            volumes: Vec::new(),
            repos: vec![RepoReport {
                name: "app".into(),
                path: "~/app".into(),
                base: "origin/main".into(),
                policy: reap_core::config::Policy::Auto,
                gh_note: None,
                error: error.map(str::to_string),
                worktrees,
                salvaged: Vec::new(),
            }],
        }
    }

    fn holder(path: &str, why: &str, bytes: u64) -> Holder {
        Holder {
            path: path.into(),
            why: why.into(),
            bytes: Some(bytes),
        }
    }

    fn volume(is_low: bool, holders: Vec<Holder>) -> Pressure {
        Pressure {
            volume: VolumeStatus {
                name: "Backup".into(),
                mount: "/Volumes/Backup".into(),
                total_bytes: 1_000_000_000_000,
                available_bytes: 40_000_000_000,
                threshold_bytes: Some(100_000_000_000),
                is_low,
                removable: 0,
                holders,
            },
            removed: Vec::new(),
            freed_bytes: 0,
            refused: Vec::new(),
            skipped: Vec::new(),
            asked: Vec::new(),
            announce_asked: false,
            announce_stuck: false,
            log: Vec::new(),
        }
    }

    fn reclaimed(path: &str, bytes: u64) -> Reclaimed {
        Reclaimed {
            repo: "~/app".into(),
            removed: Removed {
                path: path.into(),
                branch: None,
                branch_kept: None,
                salvaged_to: None,
            },
            bytes,
        }
    }

    #[test]
    fn a_pressure_pass_says_what_it_removed_and_freed() {
        let mut pressure = volume(false, Vec::new());
        pressure.removed = vec![
            reclaimed("~/w/big", 6_000_000_000),
            reclaimed("~/w/small", 2_000_000_000),
        ];
        pressure.freed_bytes = 8_000_000_000;

        assert_eq!(
            pressure_notifications(&[pressure.clone()]),
            [(
                "reap: low disk".to_string(),
                "Low disk on Backup: removed 2 worktrees, freed 8.0 GB: big, small".to_string()
            )]
        );
        pressure.removed.truncate(1);
        assert!(
            pressure_notifications(&[pressure])[0]
                .1
                .contains("removed 1 worktree,")
        );
    }

    #[test]
    fn a_volume_that_stays_low_says_what_holds_the_space_once() {
        let mut pressure = volume(
            true,
            vec![
                holder("~/w/dirty", "dirty", 9_000_000_000),
                holder("~/w/kept", "kept", 3_000_000_000),
                holder("~/w/busy", "in use", 2_000_000_000),
                holder("~/w/more", "locked", 1_000_000_000),
            ],
        );
        assert_eq!(
            pressure_notifications(&[pressure.clone()]),
            Vec::<(String, String)>::new()
        );
        pressure.announce_stuck = true;

        assert_eq!(
            pressure_notifications(&[pressure]),
            [(
                "reap: low disk".to_string(),
                "Low disk on Backup: 40 GB free, and nothing reap can remove. Holding space: \
                 dirty 9.0 GB (dirty), kept 3.0 GB (kept), busy 2.0 GB (in use), and 1 more."
                    .to_string()
            )]
        );
    }

    #[test]
    fn an_ask_repo_gets_an_immediate_low_disk_notice_and_nothing_is_removed() {
        let mut pressure = volume(true, Vec::new());
        pressure.asked = vec![holder("~/w/asked", "removable", 5_000_000_000)];
        pressure.announce_asked = true;

        assert_eq!(
            pressure_notifications(&[pressure]),
            [(
                "reap: low disk".to_string(),
                "Low disk on Backup: asked can be removed; confirm with Remove now".to_string()
            )]
        );
    }

    #[test]
    fn a_clock_the_pressure_pass_ran_out_is_not_announced() {
        let started = |path: &str| Started {
            path: path.into(),
            reason: "merged".into(),
            removes_at: None,
        };
        let outcome = Outcome {
            repo: "~/app".into(),
            skipped: None,
            fetch_note: None,
            started: vec![started("~/w/gone"), started("~/w/stays")],
            removed: Vec::new(),
            refused: vec![Refused {
                path: "~/w/x".into(),
                message: "locked".into(),
            }],
            pruned: false,
            pruned_salvage: Vec::new(),
            log: Vec::new(),
            report: report(Vec::new(), None).repos.remove(0),
        };
        let mut pressure = volume(false, Vec::new());
        pressure.removed = vec![reclaimed("~/w/gone", 1)];

        let messages = notifications(&[outcome], &[pressure]);

        let started: Vec<_> = messages
            .iter()
            .filter(|(title, _)| title == "reap: worktree to remove")
            .collect();
        assert_eq!(started.len(), 1);
        assert!(started[0].1.starts_with("~/w/stays"));
        assert!(messages.iter().any(|(title, _)| title == "reap: low disk"));
    }

    #[test]
    fn a_worktree_is_reported_once_when_it_turns_stale() {
        let mut previous = HashSet::new();
        let stale = report(
            vec![row("~/w/a", State::Stale), row("~/w/b", State::Open)],
            None,
        );

        assert_eq!(new_needs_you(&mut previous, &stale).len(), 1);
        assert_eq!(new_needs_you(&mut previous, &stale).len(), 0);
        new_needs_you(
            &mut previous,
            &report(vec![row("~/w/a", State::Open)], None),
        );
        assert_eq!(new_needs_you(&mut previous, &stale).len(), 1);
    }

    #[test]
    fn an_unreadable_repo_needs_you() {
        let mut previous = HashSet::new();

        let fresh = new_needs_you(
            &mut previous,
            &report(Vec::new(), Some("origin/main does not exist")),
        );

        assert_eq!(
            fresh,
            [(
                "~/app".to_string(),
                "origin/main does not exist".to_string()
            )]
        );
    }

    #[test]
    fn many_notifications_become_one_summary() {
        let message = |title: &str| (title.to_string(), "~/w".to_string());
        let few = vec![message("reap removed a worktree"); 3];
        let many = [
            vec![message("reap: worktree to remove"); 4],
            vec![message("reap removed a worktree"); 2],
        ]
        .concat();

        assert_eq!(batched(few.clone()), few);
        assert_eq!(
            batched(many),
            [(
                "reap".to_string(),
                "Worktrees: 4 to remove, 2 removed. Open reap to see them.".to_string()
            )]
        );
    }

    #[test]
    fn the_icon_follows_the_overall_state() {
        let mut needs_you = report(Vec::new(), None);
        assert_eq!(icon_for(Some(&needs_you)), Icon::NeedsYou);
        needs_you.overall = Overall::Tidy;
        assert_eq!(icon_for(Some(&needs_you)), Icon::Tidy);
        assert_eq!(icon_for(None), Icon::Pending);
    }

    #[test]
    fn the_popover_stays_on_screen() {
        let screen = Area {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 800.0,
        };
        let icon = Area {
            x: 980.0,
            y: 0.0,
            width: 20.0,
            height: 24.0,
        };

        assert_eq!(
            popover_origin(&icon, (360.0, 480.0), Some(&screen), 4.0),
            (640.0, 28.0)
        );
    }

    #[test]
    fn a_click_right_after_a_blur_does_not_reopen() {
        let now = Instant::now();
        let guard = Duration::from_millis(300);

        assert!(reopens(None, now, guard));
        assert!(!reopens(Some(now), now, guard));
        assert!(reopens(Some(now), now + guard, guard));
    }
}
