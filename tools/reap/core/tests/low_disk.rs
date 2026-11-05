//! The pressure pass end to end (ADR low-disk-skips-the-grace): a faked low volume, real
//! repositories and worktrees, largest first, stopping at 1.5 times the threshold.

#[allow(dead_code)] // each test binary uses part of the sandbox
mod sandbox;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use reap_core::ctx::Ctx;
use reap_core::report::{self, Overall};
use reap_core::state_file;
use reap_core::sweep::{self, Swept};
use reap_core::volume::Volume;
use sandbox::{Sandbox, git};

const MB: u64 = 1_000_000;
/// A 100 MB volume: low below 10 MB (the default `10%`), healthy again from 15 MB.
const TOTAL: u64 = 100 * MB;

/// A world with a faked volume. Its free space is `base` plus the claimed size of every
/// worktree folder that is gone, so removing a worktree frees what it claims.
struct World {
    sandbox: Sandbox,
    base: Arc<AtomicU64>,
    claims: Vec<(PathBuf, u64)>,
}

impl World {
    fn new(free_mb: u64) -> Self {
        let sandbox = Sandbox::new();
        // Ignored files are not changes (rule 4), so the weight does not make a worktree dirty.
        std::fs::write(sandbox.clone.join(".git/info/exclude"), "*.bin\n").unwrap();
        Self {
            sandbox,
            base: Arc::new(AtomicU64::new(free_mb * MB)),
            claims: Vec::new(),
        }
    }

    /// A merged worktree (no commits of its own) weighing `mb` MB, claiming the same.
    fn worktree(&mut self, name: &str, mb: u64) -> PathBuf {
        let path = self.sandbox.worktree(name);
        std::fs::write(
            path.join("cache.bin"),
            vec![0_u8; usize::try_from(mb * MB).unwrap()],
        )
        .unwrap();
        self.claims.push((path.clone(), mb * MB));
        path
    }

    fn ctx(&self, is_dry_run: bool) -> Ctx {
        let base = self.base.clone();
        let claims = self.claims.clone();
        Ctx::new(self.sandbox.paths.clone(), is_dry_run).with_volumes(move |_| {
            let freed: u64 = claims
                .iter()
                .filter(|(folder, _)| !folder.exists())
                .map(|(_, bytes)| bytes)
                .sum();
            Ok(Volume {
                id: 1,
                mount: "/Volumes/Fake".into(),
                total: TOTAL,
                available: base.load(Ordering::SeqCst) + freed,
            })
        })
    }

    fn sweep(&self) -> Swept {
        sweep::sweep_all(&self.ctx(false), Duration::ZERO, Utc::now()).unwrap()
    }
}

fn set_global(sandbox: &Sandbox, line: &str) {
    let file = sandbox.paths.config_file();
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("{line}\n{text}")).unwrap();
}

fn removed_paths(swept: &Swept) -> Vec<&str> {
    swept
        .volumes
        .iter()
        .flat_map(|pressure| &pressure.removed)
        .map(|reclaimed| reclaimed.removed.path.as_str())
        .collect()
}

fn branch_exists(sandbox: &Sandbox, branch: &str) -> bool {
    !git(&sandbox.clone, &["branch", "--list", branch]).is_empty()
}

#[test]
fn a_low_volume_loses_its_largest_removable_worktrees_first_and_stops_at_one_and_a_half_times() {
    let mut world = World::new(8);
    let big = world.worktree("big", 6);
    let mid = world.worktree("mid", 4);
    let small = world.worktree("small", 2);
    world.sandbox.list_clone();

    let swept = world.sweep();

    // 8 -> 14 MB is short of 15; 14 -> 18 MB is not. `small` was never needed.
    assert_eq!(removed_paths(&swept), ["~/big", "~/mid"]);
    assert!(!big.exists() && !mid.exists() && small.exists());
    let pressure = &swept.volumes[0];
    assert_eq!(pressure.freed_bytes, 10 * MB);
    assert_eq!(pressure.volume.available_bytes, 18 * MB);
    assert!(!pressure.volume.is_low);
    // The guarded path: the branch goes with `git branch -d`, and the grace clock is gone.
    assert!(!branch_exists(&world.sandbox, "big"));
    let memory = state_file::load(&world.sandbox.paths)
        .remove("~/app")
        .unwrap();
    assert!(!memory.clocks.contains_key("~/big") && !memory.clocks.contains_key("~/mid"));
    // Worktrees it did not reach keep their clocks.
    assert!(memory.clocks.contains_key("~/small"));
    // The repo's report no longer lists what was removed.
    let rows: Vec<_> = swept.repos[0]
        .report
        .worktrees
        .iter()
        .map(|row| row.name.as_str())
        .collect();
    assert_eq!(rows, ["small"]);
    assert!(state_file::load_volumes(&world.sandbox.paths).is_empty());
}

#[test]
fn kept_dirty_and_ask_worktrees_are_never_removed_however_large() {
    let mut world = World::new(5);
    let kept = world.worktree("kept", 20);
    let dirty = world.worktree("dirty", 15);
    std::fs::write(dirty.join("notes"), "work in progress").unwrap();
    let removable = world.worktree("removable", 1);
    world.sandbox.list_clone();
    sweep::keep(&world.sandbox.paths, kept.to_str().unwrap()).unwrap();

    let swept = world.sweep();

    assert_eq!(removed_paths(&swept), ["~/removable"]);
    assert!(kept.exists() && dirty.exists() && !removable.exists());
    // Still low, nothing left: it says so, and names what holds the space, largest first.
    let pressure = &swept.volumes[0];
    assert!(pressure.volume.is_low && pressure.volume.removable == 0);
    let held: Vec<_> = pressure
        .volume
        .holders
        .iter()
        .map(|holder| (holder.path.as_str(), holder.why.as_str()))
        .collect();
    assert_eq!(held, [("~/kept", "kept"), ("~/dirty", "dirty")]);
    assert!(pressure.volume.holders[0].bytes.unwrap() >= 20 * MB);
}

#[test]
fn a_volume_that_stays_low_is_announced_once_per_episode() {
    let mut world = World::new(5);
    let kept = world.worktree("kept", 3);
    world.sandbox.list_clone();
    sweep::keep(&world.sandbox.paths, kept.to_str().unwrap()).unwrap();

    let first = world.sweep();
    let second = world.sweep();
    let status =
        report::report_with(&world.sandbox.paths, world.ctx(false).reader(), Utc::now()).unwrap();
    world.base.store(50 * MB, Ordering::SeqCst);
    let healthy = world.sweep();
    world.base.store(5 * MB, Ordering::SeqCst);
    let again = world.sweep();

    assert!(first.volumes[0].announce_stuck);
    assert!(!second.volumes[0].announce_stuck);
    assert!(second.volumes[0].volume.is_low);
    // Low with nothing reap can remove: the tray needs the owner, and `status` says so.
    assert_eq!(status.overall, Overall::NeedsYou);
    assert!(status.volumes[0].is_low);
    assert!(report::render(&status).contains("LOW"));
    // Healthy again ends the episode; the next one is announced afresh.
    assert!(!healthy.volumes[0].volume.is_low);
    assert!(again.volumes[0].announce_stuck);
}

#[test]
fn an_ask_repo_is_announced_on_a_low_volume_but_nothing_is_removed() {
    let mut world = World::new(5);
    let asked = world.worktree("asked", 4);
    world.sandbox.list_clone();
    set_global(&world.sandbox, "policy = \"ask\"");

    let first = world.sweep();
    let second = world.sweep();

    assert!(asked.exists());
    assert_eq!(removed_paths(&first), Vec::<&str>::new());
    assert_eq!(first.volumes[0].asked[0].path, "~/asked");
    assert!(first.volumes[0].announce_asked);
    assert!(!second.volumes[0].announce_asked);
}

#[test]
fn low_disk_off_and_policy_off_leave_the_grace_alone() {
    for line in ["low_disk = \"off\"", "policy = \"off\""] {
        let mut world = World::new(5);
        let worktree = world.worktree("quiet", 4);
        world.sandbox.list_clone();
        set_global(&world.sandbox, line);

        let swept = world.sweep();

        assert!(worktree.exists(), "{line}");
        assert!(
            swept
                .volumes
                .iter()
                .all(|pressure| pressure.removed.is_empty()),
            "{line}"
        );
    }
}

#[test]
fn a_dry_run_prints_the_removals_it_would_make_and_changes_nothing() {
    let mut world = World::new(8);
    let big = world.worktree("big", 6);
    let mid = world.worktree("mid", 4);
    let small = world.worktree("small", 2);
    world.sandbox.list_clone();
    // A healthy tick first, so there is a state file to compare.
    world.base.store(50 * MB, Ordering::SeqCst);
    world.sweep();
    world.base.store(8 * MB, Ordering::SeqCst);
    let state_before = std::fs::read_to_string(world.sandbox.paths.state_file()).unwrap();

    let swept = sweep::sweep_all(&world.ctx(true), Duration::ZERO, Utc::now()).unwrap();

    assert!(big.exists() && mid.exists() && small.exists());
    let pressure = &swept.volumes[0];
    assert_eq!(removed_paths(&swept), ["~/big", "~/mid"]);
    assert!(
        pressure
            .log
            .iter()
            .filter(|line| line.starts_with("would run: git -C") && line.contains("worktree remove"))
            .count()
            == 2,
        "{:?}",
        pressure.log
    );
    assert_eq!(
        std::fs::read_to_string(world.sandbox.paths.state_file()).unwrap(),
        state_before
    );
}

#[test]
fn a_due_sweep_frees_space_even_for_a_repo_that_is_not_due_and_pause_stops_it() {
    let mut world = World::new(50);
    let heavy = world.worktree("heavy", 6);
    world.sandbox.list_clone();
    let now: DateTime<Utc> = Utc::now();
    let ctx = world.ctx(false);

    let healthy = sweep::sweep_due(&ctx, now).unwrap();
    world.base.store(5 * MB, Ordering::SeqCst);
    world.sandbox.paths.set_paused(true).unwrap();
    let paused = sweep::sweep_due(&ctx, now + chrono::Duration::minutes(1)).unwrap();
    world.sandbox.paths.set_paused(false).unwrap();
    let low = sweep::sweep_due(&ctx, now + chrono::Duration::minutes(1)).unwrap();

    assert_eq!(healthy.repos.len(), 1);
    assert_eq!(removed_paths(&healthy), Vec::<&str>::new());
    assert!(paused.volumes.is_empty() && paused.repos.is_empty());
    // One minute after the last pass, under the 5 minute interval: the repo is not due.
    assert!(low.repos.is_empty());
    assert_eq!(removed_paths(&low), ["~/heavy"]);
    assert!(!heavy.exists());
}

#[test]
fn a_repo_another_run_holds_is_skipped_and_its_worktree_waits() {
    let mut world = World::new(5);
    let held = world.worktree("held", 4);
    world.sandbox.list_clone();
    let tether = std::fs::File::create(world.sandbox.clone.join(".git/tether.lock")).unwrap();
    tether.lock().unwrap();

    let swept = world.sweep();

    assert!(held.exists());
    let pressure = &swept.volumes[0];
    assert_eq!(removed_paths(&swept), Vec::<&str>::new());
    assert!(
        pressure
            .skipped
            .iter()
            .any(|skipped| skipped.message.contains("git-tether"))
    );
    // Busy is not stuck: the next tick tries again.
    assert!(!pressure.announce_stuck);
}
