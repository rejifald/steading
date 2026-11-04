//! The tray's decisions without any UI: which worktrees just started to need the owner, which
//! icon to show, and where the popover opens. Kept apart from `tray` so it can be unit-tested.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use reap_core::classify::State;
use reap_core::report::{Overall, Report};
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

/// The notification of something that needs the owner now but did not before, as (title,
/// message). `previous` is replaced by the current set, keyed by path and state, so a path that
/// recovers and breaks again is reported again, and a held stale worktree that becomes saveable
/// is reported as abandoned (ADR 0006 rule 6).
pub fn new_needs_you(previous: &mut HashSet<String>, report: &Report) -> Vec<(String, String)> {
    let mut current = HashSet::new();
    let mut fresh = Vec::new();
    for repo in &report.repos {
        let mut found = |key: String, title: &str, message: String| {
            if !previous.contains(&key) {
                fresh.push((title.to_string(), message));
            }
            current.insert(key);
        };
        if let Some(error) = &repo.error {
            found(
                format!("error:{}", repo.path),
                NEEDS_YOU,
                format!("{}: {error}", repo.path),
            );
        }
        for row in &repo.worktrees {
            match row.state {
                State::Stale => found(
                    format!("stale:{}", row.path),
                    NEEDS_YOU,
                    format!("{}: {}", row.path, row.reason),
                ),
                State::Abandoned => found(
                    format!("abandoned:{}", row.path),
                    TO_SAVE,
                    format!(
                        "{}: {}; Save and remove keeps its work in a ref",
                        row.path, row.reason
                    ),
                ),
                _ => {}
            }
        }
    }
    *previous = current;
    fresh
}

const NEEDS_YOU: &str = "reap needs you";
/// The title of an abandoned worktree's one notification.
const TO_SAVE: &str = "reap: worktree to save";

/// What a pass tells the owner (rules 10 and 11): a grace clock started, a removal, a refusal.
pub fn notifications(outcomes: &[Outcome]) -> Vec<(String, String)> {
    let mut messages = Vec::new();
    for outcome in outcomes {
        for started in &outcome.started {
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
        (TO_SAVE, "to save"),
        (NEEDS_YOU, "need you"),
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
    use reap_core::report::{RepoReport, Row};

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
            local: Vec::new(),
            held_by: None,
        }
    }

    fn report(worktrees: Vec<Row>, error: Option<&str>) -> Report {
        Report {
            paused: false,
            overall: Overall::NeedsYou,
            removable: 0,
            needs_you: 1,
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
    fn an_abandoned_worktree_is_reported_once_even_after_it_was_held() {
        let mut previous = HashSet::new();
        let held = report(vec![row("~/w/a", State::Stale)], None);
        let abandoned = report(vec![row("~/w/a", State::Abandoned)], None);

        new_needs_you(&mut previous, &held);
        let fresh = new_needs_you(&mut previous, &abandoned);

        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].0, "reap: worktree to save");
        assert!(
            fresh[0]
                .1
                .ends_with("Save and remove keeps its work in a ref")
        );
        assert_eq!(new_needs_you(&mut previous, &abandoned).len(), 0);
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
                "reap needs you".to_string(),
                "~/app: origin/main does not exist".to_string()
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
