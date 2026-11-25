//! The pressure pass (ADR low-disk-skips-the-grace): when the volume a listed repo's worktrees
//! are on runs low on space, its removable worktrees go largest first, without waiting out their
//! grace, until the volume is healthy again.
//!
//! Every removal goes through the guarded path of a normal one ([`sweep::Judged`]): each repo is
//! inspected once under its lock when its first candidate comes up, and every candidate is
//! judged against that inspection, with a fresh look for processes after the first removal (ADR
//! unanswered-checks-keep-the-clock). Candidates go in batches of up to
//! [`sweep::PARALLEL_REMOVALS`], whose `git worktree remove` commands run side by side; the free
//! space is read once per batch. git is never forced beyond what rules 4 and 11 allow, and
//! a repo another run or git-tether holds is skipped until the next tick. Only
//! `State::Removable` worktrees of `policy = "auto"` repos are candidates; `policy = "ask"`
//! repos are reported for a notification, and dirty, kept, in-use and locked worktrees are
//! named as what still holds the space.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::classify::State;
use crate::config::{LowDisk, Policy, Repo};
use crate::ctx::Ctx;
use crate::paths::Paths;
use crate::report::{self, RepoReport};
use crate::state_file::{self, VolumeMemory};
use crate::sweep::{self, Attempt, Judged, PARALLEL_REMOVALS, Queued, Refused, Removed};
use crate::volume::{ReadVolume, Volume};

/// A worktree that holds space on a low volume, and why reap does not free it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Holder {
    pub path: String,
    /// Its state (`dirty`, `kept`, `in use`, ...), or the setting that keeps reap from removing it.
    pub why: String,
    /// The disk removing it would free; absent when it could not be measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

/// One volume that holds a listed repo's worktrees, as `status` and the tray show it.
#[derive(Debug, Clone, Serialize)]
pub struct VolumeStatus {
    /// How the owner knows it, such as `Backup`.
    pub name: String,
    pub mount: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    /// The free space below which it is low: the highest `low_disk` among the repos with
    /// worktrees here. Absent when every one of them has `low_disk = "off"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold_bytes: Option<u64>,
    pub is_low: bool,
    /// While low: the removable worktrees here that reap can still remove by itself (policy
    /// auto, `low_disk` on). Zero with the volume low means reap has nothing left to do.
    pub removable: usize,
    /// While low: what holds space reap will not free by itself, largest first.
    pub holders: Vec<Holder>,
}

/// One worktree a pressure pass removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reclaimed {
    /// Its repo, as shown (`~/…`).
    pub repo: String,
    #[serde(flatten)]
    pub removed: Removed,
    /// What its folder measured before it was removed.
    pub bytes: u64,
}

/// What the pressure pass did about one volume.
#[derive(Debug, Clone, Serialize)]
pub struct Pressure {
    /// The volume after the pass.
    pub volume: VolumeStatus,
    pub removed: Vec<Reclaimed>,
    /// What the volume actually gained: its free space after the pass less before it. The sizes
    /// of `removed` can claim more, since APFS clones count in full.
    pub freed_bytes: u64,
    pub refused: Vec<Refused>,
    /// Repos another run held, with why; their worktrees wait for the next tick.
    pub skipped: Vec<Refused>,
    /// The removable worktrees of `policy = "ask"` repos on the volume: nothing is removed.
    pub asked: Vec<Holder>,
    /// `asked` has a worktree not announced yet in this episode.
    pub announce_asked: bool,
    /// The volume is low, nothing was removed and reap has nothing left to remove: the notice
    /// goes out once per episode.
    pub announce_stuck: bool,
    /// Every command run, or, in a dry run, every command that would have run.
    pub log: Vec<String>,
}

impl Pressure {
    fn quiet(volume: VolumeStatus) -> Self {
        Self {
            volume,
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

    /// Whether this pass removed a worktree of the repo shown as `repo`.
    #[must_use]
    pub fn removed_from(&self, repo: &str) -> bool {
        self.removed.iter().any(|removed| removed.repo == repo)
    }
}

/// A removable worktree a pass may remove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidate {
    /// Index into the listed repos.
    pub repo: usize,
    /// Its path as shown.
    pub key: String,
    pub folder: PathBuf,
    pub bytes: u64,
}

/// Largest first, the order that frees the most per removal; ties by path, so a pass is
/// repeatable.
pub(crate) fn largest_first(candidates: &mut [Candidate]) {
    candidates.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.key.cmp(&b.key)));
}

/// Walks `candidates` in order, removing them in batches through `remove_batch` until the volume
/// is healthy. `healthy_at(repo)` is the free space at which that repo's worktrees stop being
/// removed, so a repo with a lower `low_disk` is spared before one with a higher.
///
/// A batch takes the next candidates, up to [`PARALLEL_REMOVALS`], while the free space plus
/// what the batch holds so far is still short of the candidate's healthy point: it never
/// deliberately removes more than the estimate says is needed. A candidate the estimate leaves
/// out waits for the next batch, judged then against the free space as it really is.
/// `remove_batch` gets the batch and the free space now, and answers which of the batch it
/// removed and the free space after. Returns the indices of the candidates removed and the free
/// space at the end.
pub(crate) fn reclaim(
    candidates: &[Candidate],
    mut available: u64,
    healthy_at: impl Fn(usize) -> u64,
    mut remove_batch: impl FnMut(&[&Candidate], u64) -> (Vec<bool>, u64),
) -> (Vec<usize>, u64) {
    let mut removed = Vec::new();
    let mut waiting: Vec<usize> = (0..candidates.len()).collect();
    while !waiting.is_empty() {
        let mut batch: Vec<usize> = Vec::new();
        let mut estimated = available;
        let mut left_for_later = Vec::new();
        for &index in &waiting {
            let candidate = &candidates[index];
            let is_needed = estimated < healthy_at(candidate.repo);
            if is_needed && batch.len() < PARALLEL_REMOVALS {
                batch.push(index);
                estimated = estimated.saturating_add(candidate.bytes);
            } else if is_needed || !batch.is_empty() {
                left_for_later.push(index);
            }
            // Otherwise the volume is already healthy for its repo: it is not removed.
        }
        if batch.is_empty() {
            break;
        }
        let members: Vec<&Candidate> = batch.iter().map(|&index| &candidates[index]).collect();
        let (was_removed, after) = remove_batch(&members, available);
        removed.extend(
            batch
                .iter()
                .zip(was_removed)
                .filter_map(|(&index, was_removed)| was_removed.then_some(index)),
        );
        available = after;
        waiting = left_for_later;
    }
    (removed, available)
}

/// A linked worktree's place: which repo, and its path as shown.
struct Placed {
    repo: usize,
    key: String,
}

/// The listed repos' worktree folders by the volume they are on.
struct Placement {
    volumes: BTreeMap<u64, Volume>,
    on: BTreeMap<u64, Vec<Placed>>,
}

/// Reads the volume of each folder; `folders[i]` are repo `i`'s. A folder that cannot be read is
/// left out.
fn place(paths: &Paths, read: &ReadVolume, folders: &[Vec<PathBuf>]) -> Placement {
    let mut placement = Placement {
        volumes: BTreeMap::new(),
        on: BTreeMap::new(),
    };
    for (repo, list) in folders.iter().enumerate() {
        for folder in list {
            let Ok(volume) = read(folder) else { continue };
            placement.on.entry(volume.id).or_default().push(Placed {
                repo,
                key: paths.tilde(folder),
            });
            placement.volumes.entry(volume.id).or_insert(volume);
        }
    }
    placement
}

/// The existing folders of a repo's linked worktrees.
fn linked_folders(paths: &Paths, repo: &Repo) -> Vec<PathBuf> {
    report::list_worktrees(paths, &repo.path)
        .unwrap_or_default()
        .into_iter()
        .skip(1)
        .filter(|worktree| !worktree.is_bare && worktree.path.is_dir())
        .map(|worktree| worktree.path)
        .collect()
}

/// What the rows of one volume hold.
#[derive(Default)]
struct Survey {
    eligible: Vec<Candidate>,
    asked: Vec<Holder>,
    holders: Vec<Holder>,
}

/// Sorts the rows of the repos on one volume. `active` are the repos for which the volume is
/// low; only their removable worktrees are candidates.
fn survey(
    paths: &Paths,
    repos: &[Repo],
    reports: &BTreeMap<usize, &RepoReport>,
    here: &[Placed],
    active: &[usize],
) -> Survey {
    let mut survey = Survey::default();
    let here: HashSet<(usize, &str)> = here
        .iter()
        .map(|placed| (placed.repo, placed.key.as_str()))
        .collect();
    for (&index, report) in reports {
        let repo = &repos[index];
        for row in &report.worktrees {
            if !here.contains(&(index, row.path.as_str())) {
                continue;
            }
            let held = |why: &str| Holder {
                path: row.path.clone(),
                why: why.to_string(),
                bytes: row.size_bytes,
            };
            let is_active = active.contains(&index);
            match (row.state, repo.policy) {
                (State::Removable, Policy::Auto) if is_active => {
                    survey.eligible.push(Candidate {
                        repo: index,
                        key: row.path.clone(),
                        folder: paths.expand(&row.path),
                        bytes: row.size_bytes.unwrap_or(0),
                    });
                }
                (State::Removable, Policy::Ask) if is_active => {
                    survey
                        .asked
                        .push(held("removable, waits for your confirmation"));
                }
                (State::Removable, Policy::Off) => {
                    survey.holders.push(held("removable, policy off"));
                }
                (State::Removable, _) if repo.low_disk == LowDisk::Off => {
                    survey.holders.push(held("removable, low_disk off"));
                }
                (State::Dirty | State::Kept | State::InUse | State::Locked | State::Refused, _) => {
                    survey.holders.push(held(row.state.label()));
                }
                _ => {}
            }
        }
    }
    let by_size = |a: &Holder, b: &Holder| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path));
    survey.holders.sort_by(by_size);
    survey.asked.sort_by(by_size);
    survey
}

/// The repos on a volume for which it is low.
fn low_for(repos: &[Repo], volume: &Volume, available: u64, here: &BTreeSet<usize>) -> Vec<usize> {
    here.iter()
        .copied()
        .filter(|&index| repos[index].low_disk.is_low(available, volume.total))
        .collect()
}

/// The free space at which every repo on the volume is satisfied (1.5 times its `low_disk`).
fn healthy_at(repos: &[Repo], volume: &Volume, here: &BTreeSet<usize>) -> u64 {
    here.iter()
        .filter_map(|&index| repos[index].low_disk.healthy_at(volume.total))
        .max()
        .unwrap_or(0)
}

fn threshold_of(repos: &[Repo], volume: &Volume, here: &BTreeSet<usize>) -> Option<u64> {
    here.iter()
        .filter_map(|&index| repos[index].low_disk.threshold(volume.total))
        .max()
}

fn status_of(
    repos: &[Repo],
    volume: &Volume,
    available: u64,
    here: &BTreeSet<usize>,
    survey: Option<(usize, Vec<Holder>)>,
) -> VolumeStatus {
    let is_low = !low_for(repos, volume, available, here).is_empty();
    // What holds the space matters only while the volume is low.
    let (removable, holders) = survey.filter(|_| is_low).unwrap_or_default();
    VolumeStatus {
        name: volume.name(),
        mount: volume.mount.display().to_string(),
        total_bytes: volume.total,
        available_bytes: available,
        threshold_bytes: threshold_of(repos, volume, here),
        is_low,
        removable,
        holders,
    }
}

fn repos_here(here: &[Placed]) -> BTreeSet<usize> {
    here.iter().map(|placed| placed.repo).collect()
}

/// Every volume the listed repos' worktrees are on, as `reports` (one per repo, in order) show
/// them: read-only, for `git reap status`.
pub(crate) fn statuses(
    paths: &Paths,
    read: &ReadVolume,
    repos: &[Repo],
    reports: &[RepoReport],
) -> Vec<VolumeStatus> {
    let folders: Vec<Vec<PathBuf>> = reports
        .iter()
        .map(|report| {
            report
                .worktrees
                .iter()
                .map(|row| paths.expand(&row.path))
                .filter(|folder| folder.is_dir())
                .collect()
        })
        .collect();
    let placement = place(paths, read, &folders);
    let by_repo: BTreeMap<usize, &RepoReport> = reports.iter().enumerate().collect();
    let mut statuses: Vec<VolumeStatus> = placement
        .volumes
        .values()
        .map(|volume| {
            let here = &placement.on[&volume.id];
            let set = repos_here(here);
            let active = low_for(repos, volume, volume.available, &set);
            let held = (!active.is_empty()).then(|| {
                let survey = survey(paths, repos, &by_repo, here, &active);
                (survey.eligible.len(), survey.holders)
            });
            status_of(repos, volume, volume.available, &set, held)
        })
        .collect();
    statuses.sort_by(|a, b| a.mount.cmp(&b.mount));
    statuses
}

/// The repos' reports, read once and only for the repos that need one.
struct Reports<'a> {
    ctx: &'a Ctx,
    repos: &'a [Repo],
    now: DateTime<Utc>,
    cache: BTreeMap<usize, Option<RepoReport>>,
}

impl Reports<'_> {
    /// Reads (and sizes) each of `wanted` the first time it is asked for.
    fn load(&mut self, wanted: &BTreeSet<usize>) {
        for &index in wanted {
            self.cache.entry(index).or_insert_with(|| {
                let mut report =
                    sweep::read_only(&self.ctx.paths, &self.repos[index], self.now).ok()?;
                report::measure(&self.ctx.paths, &mut report);
                Some(report)
            });
        }
    }

    fn of(&self, wanted: &BTreeSet<usize>) -> BTreeMap<usize, &RepoReport> {
        wanted
            .iter()
            .filter_map(|index| Some((*index, self.cache.get(index)?.as_ref()?)))
            .collect()
    }
}

/// The pressure pass over every volume that holds a listed repo's worktrees. Looks at the free
/// space of each, whether or not its repos were due for a pass of their own; inspects and
/// removes only while one is low. A dry run removes nothing and assumes each removal frees its
/// measured size.
///
/// # Errors
///
/// Fails when `state.json` cannot be written.
pub(crate) fn pass(ctx: &Ctx, repos: &[Repo], now: DateTime<Utc>) -> Result<Vec<Pressure>> {
    let paths = &ctx.paths;
    let folders: Vec<Vec<PathBuf>> = repos
        .iter()
        .map(|repo| linked_folders(paths, repo))
        .collect();
    let placement = place(paths, ctx.reader(), &folders);
    let loaded = state_file::load_volumes(paths);
    let mut episodes = loaded.clone();
    let mut reports = Reports {
        ctx,
        repos,
        now,
        cache: BTreeMap::new(),
    };
    let mut pressures = Vec::new();
    for volume in placement.volumes.values() {
        let here = &placement.on[&volume.id];
        let set = repos_here(here);
        let mount = volume.mount.display().to_string();
        let active = low_for(repos, volume, volume.available, &set);
        if active.is_empty() {
            if volume.available >= healthy_at(repos, volume, &set) {
                episodes.remove(&mount);
            }
            pressures.push(Pressure::quiet(status_of(
                repos,
                volume,
                volume.available,
                &set,
                None,
            )));
            continue;
        }
        reports.load(&set);
        let found = survey(paths, repos, &reports.of(&set), here, &active);
        pressures.push(relieve(
            ctx,
            repos,
            volume,
            &set,
            found,
            episodes.entry(mount).or_insert_with(|| VolumeMemory {
                since: now,
                has_announced_stuck: false,
                asked: BTreeSet::new(),
            }),
            now,
        ));
        let healthy = healthy_at(repos, volume, &set);
        if pressures
            .last()
            .is_some_and(|last| last.volume.available_bytes >= healthy)
        {
            episodes.remove(&volume.mount.display().to_string());
        }
    }
    pressures.sort_by(|a, b| a.volume.mount.cmp(&b.volume.mount));
    if !ctx.dry_run && episodes != loaded {
        state_file::update_volumes(paths, |volumes| *volumes = episodes)?;
    }
    Ok(pressures)
}

/// What the removals of one volume came to.
#[derive(Default)]
struct Attempts {
    /// Repos another writer held: the rest of their worktrees wait for the next tick.
    busy: HashSet<usize>,
    /// Repos held and inspected for this volume, by index: one inspection serves all of a
    /// repo's candidates, and the locks are released when the volume is done.
    held: BTreeMap<usize, Judged>,
    removed: Vec<Reclaimed>,
    refused: Vec<Refused>,
    skipped: Vec<Refused>,
}

impl Attempts {
    /// The repo's lock and inspection, taken the first time one of its candidates comes up.
    fn judged(
        &mut self,
        ctx: &Ctx,
        repo: &Repo,
        index: usize,
        now: DateTime<Utc>,
    ) -> Result<Result<&mut Judged, String>> {
        Ok(match self.held.entry(index) {
            std::collections::btree_map::Entry::Occupied(held) => Ok(held.into_mut()),
            std::collections::btree_map::Entry::Vacant(slot) => {
                match Judged::take(ctx, repo, Duration::ZERO, None, now)? {
                    Ok(judged) => Ok(slot.insert(judged)),
                    Err(why) => Err(why),
                }
            }
        })
    }

    /// Tries to remove `batch` through the guarded path: each candidate is judged and prepared
    /// in order, the `git worktree remove` commands of those that may go run side by side, and
    /// each is finished in order. Answers which of `batch` were removed, and the volume's free
    /// space after (read once). A candidate that was refused, is no longer removable or sits in
    /// a busy repo does not stop the others.
    fn remove_batch(
        &mut self,
        ctx: &Ctx,
        repos: &[Repo],
        volume: &Volume,
        batch: &[&Candidate],
        available: u64,
        now: DateTime<Utc>,
    ) -> (Vec<bool>, u64) {
        let mut ready = Vec::new();
        for (position, candidate) in batch.iter().enumerate() {
            if self.busy.contains(&candidate.repo) {
                continue;
            }
            let repo = &repos[candidate.repo];
            let queued =
                self.judged(ctx, repo, candidate.repo, now)
                    .and_then(|judged| match judged {
                        Ok(judged) => judged.queue(ctx, repo, &candidate.key, now).map(Ok),
                        Err(why) => Ok(Err(why)),
                    });
            match queued {
                Ok(Ok(Queued::Ready(prepared))) => ready.push((position, prepared)),
                Ok(Ok(Queued::Decided(attempt))) => {
                    self.settle(ctx, repo, candidate, Ok(attempt));
                }
                Ok(Err(why)) => {
                    self.busy.insert(candidate.repo);
                    self.skipped.push(Refused {
                        path: ctx.paths.tilde(&repo.path),
                        message: why,
                    });
                }
                Err(e) => {
                    self.settle(ctx, repo, candidate, Err(e));
                }
            }
        }
        let jobs: Vec<(&Path, &sweep::PreparedRemoval)> = ready
            .iter()
            .map(|(position, prepared)| (repos[batch[*position].repo].path.as_path(), prepared))
            .collect();
        let outputs = sweep::run_removals(ctx, &jobs);
        drop(jobs);
        let mut was_removed = vec![false; batch.len()];
        for ((position, prepared), output) in ready.into_iter().zip(outputs) {
            let candidate = batch[position];
            let repo = &repos[candidate.repo];
            let attempt = output.and_then(|output| match self.held.get_mut(&candidate.repo) {
                Some(judged) => judged.finish(ctx, repo, &candidate.key, prepared, output),
                None => bail!("the lock on {} was lost", ctx.paths.tilde(&repo.path)),
            });
            was_removed[position] = self.settle(ctx, repo, candidate, attempt);
        }
        let freed: Vec<&&Candidate> = batch
            .iter()
            .zip(&was_removed)
            .filter_map(|(candidate, was_removed)| was_removed.then_some(candidate))
            .collect();
        let after = match freed.first() {
            None => available,
            Some(_) if ctx.dry_run => freed.iter().fold(available, |sum, candidate| {
                sum.saturating_add(candidate.bytes)
            }),
            Some(first) => {
                // The worktrees' folders are gone: read the parent of the first, and the mount
                // point if that is gone too.
                let parent = first.folder.parent().unwrap_or(&volume.mount);
                let reading = ctx
                    .volume_of(parent)
                    .or_else(|_| ctx.volume_of(&volume.mount));
                reading.map_or(available, |now| now.available)
            }
        };
        (was_removed, after)
    }

    /// Records what an attempt on `candidate` came to; whether it removed the worktree.
    fn settle(
        &mut self,
        ctx: &Ctx,
        repo: &Repo,
        candidate: &Candidate,
        attempt: Result<Attempt>,
    ) -> bool {
        let message = match attempt {
            Ok(Attempt::Removed(removed)) => {
                self.removed.push(Reclaimed {
                    repo: ctx.paths.tilde(&repo.path),
                    removed,
                    bytes: candidate.bytes,
                });
                return true;
            }
            // It stopped being removable between the survey and the lock.
            Ok(Attempt::NotRemovable(_)) => return false,
            Ok(Attempt::Busy(why)) => {
                self.busy.insert(candidate.repo);
                self.skipped.push(Refused {
                    path: ctx.paths.tilde(&repo.path),
                    message: why,
                });
                return false;
            }
            Ok(Attempt::Refused(message)) => message,
            Err(e) => format!("{e:#}"),
        };
        self.refused.push(Refused {
            path: candidate.key.clone(),
            message,
        });
        false
    }
}

/// Removes what a low volume needs removed and works out what to tell the owner.
fn relieve(
    ctx: &Ctx,
    repos: &[Repo],
    volume: &Volume,
    set: &BTreeSet<usize>,
    found: Survey,
    episode: &mut VolumeMemory,
    now: DateTime<Utc>,
) -> Pressure {
    let Survey {
        mut eligible,
        asked,
        holders,
    } = found;
    largest_first(&mut eligible);
    let mut attempts = Attempts::default();
    let (_, available) = reclaim(
        &eligible,
        volume.available,
        |repo| repos[repo].low_disk.healthy_at(volume.total).unwrap_or(0),
        |batch, available| attempts.remove_batch(ctx, repos, volume, batch, available, now),
    );
    let Attempts {
        busy,
        removed,
        refused,
        skipped,
        ..
    } = attempts;
    let still_removable = eligible
        .iter()
        .filter(|candidate| busy.contains(&candidate.repo))
        .count();
    let is_healthy = available >= healthy_at(repos, volume, set);
    let asked = if is_healthy { Vec::new() } else { asked };
    let announce_asked = asked
        .iter()
        .any(|holder| !episode.asked.contains(&holder.path));
    if announce_asked {
        episode
            .asked
            .extend(asked.iter().map(|holder| holder.path.clone()));
    }
    let status = status_of(
        repos,
        volume,
        available,
        set,
        Some((still_removable, holders)),
    );
    let is_stuck = status.is_low && removed.is_empty() && still_removable == 0 && asked.is_empty();
    let announce_stuck = is_stuck && !episode.has_announced_stuck;
    if announce_stuck {
        episode.has_announced_stuck = true;
    }
    Pressure {
        volume: status,
        removed,
        freed_bytes: available.saturating_sub(volume.available),
        refused,
        skipped,
        asked,
        announce_asked,
        announce_stuck,
        log: ctx.take_log(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(repo: usize, key: &str, bytes: u64) -> Candidate {
        Candidate {
            repo,
            key: key.into(),
            folder: PathBuf::from(format!("/w/{key}")),
            bytes,
        }
    }

    #[test]
    fn the_largest_worktree_goes_first_and_ties_go_by_path() {
        let mut candidates = vec![
            candidate(0, "~/b", 2),
            candidate(0, "~/a", 2),
            candidate(0, "~/big", 9),
            candidate(0, "~/none", 0),
        ];

        largest_first(&mut candidates);

        let keys: Vec<_> = candidates.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["~/big", "~/a", "~/b", "~/none"]);
    }

    /// Adapts a per-candidate remover to the batch one `reclaim` takes: each answers with the
    /// free space after it, or `None` when it removed nothing.
    fn one_by_one(
        mut remove: impl FnMut(&Candidate, u64) -> Option<u64>,
    ) -> impl FnMut(&[&Candidate], u64) -> (Vec<bool>, u64) {
        move |batch, mut available| {
            let removed = batch
                .iter()
                .map(|candidate| {
                    remove(candidate, available).is_some_and(|after| {
                        available = after;
                        true
                    })
                })
                .collect();
            (removed, available)
        }
    }

    #[test]
    fn removal_stops_once_the_volume_is_healthy_and_the_rest_keep_their_clocks() {
        let candidates = [
            candidate(0, "~/a", 60),
            candidate(0, "~/b", 40),
            candidate(0, "~/c", 10),
        ];
        let mut tried = Vec::new();

        let (removed, available) = reclaim(
            &candidates,
            80,
            |_| 150,
            one_by_one(|candidate, available| {
                tried.push(candidate.key.clone());
                Some(available + candidate.bytes)
            }),
        );

        // 80 + 60 = 140 is still short of 150; 140 + 40 = 180 is not.
        assert_eq!(removed, [0, 1]);
        assert_eq!(available, 180);
        assert_eq!(tried, ["~/a", "~/b"]);
    }

    #[test]
    fn a_batch_stops_before_the_estimated_healthy_point() {
        let candidates = [
            candidate(0, "~/a", 60),
            candidate(0, "~/b", 50),
            candidate(0, "~/c", 40),
            candidate(0, "~/d", 30),
        ];
        let mut batches = Vec::new();

        let (removed, available) = reclaim(
            &candidates,
            0,
            |_| 100,
            |batch, available| {
                batches.push(batch.iter().map(|c| c.key.clone()).collect::<Vec<_>>());
                let freed: u64 = batch.iter().map(|c| c.bytes).sum();
                (vec![true; batch.len()], available + freed)
            },
        );

        // 0 + 60 is short of 100, so `b` joins; 0 + 110 is not, so `c` stays out.
        assert_eq!(batches, [["~/a", "~/b"]]);
        assert_eq!(removed, [0, 1]);
        assert_eq!(available, 110);
    }

    #[test]
    fn a_batch_holds_at_most_the_parallel_limit_and_the_rest_follow() {
        let candidates: Vec<_> = (0..9)
            .map(|index| candidate(0, &format!("~/w{index}"), 1))
            .collect();
        let mut sizes = Vec::new();

        let (removed, available) = reclaim(
            &candidates,
            0,
            |_| 100,
            |batch, available| {
                sizes.push(batch.len());
                (vec![true; batch.len()], available + batch.len() as u64)
            },
        );

        assert_eq!(sizes, [4, 4, 1]);
        assert_eq!(removed.len(), 9);
        assert_eq!(available, 9);
    }

    #[test]
    fn a_candidate_the_estimate_left_out_is_judged_again_on_the_real_free_space() {
        let candidates = [candidate(0, "~/a", 60), candidate(0, "~/b", 60)];

        // `a` is estimated to be enough, but frees nothing: `b` must still go.
        let (removed, _) = reclaim(
            &candidates,
            0,
            |_| 50,
            one_by_one(|candidate, available| {
                Some(
                    available
                        + if candidate.key == "~/a" {
                            0
                        } else {
                            candidate.bytes
                        },
                )
            }),
        );

        assert_eq!(removed, [0, 1]);
    }

    #[test]
    fn a_refused_or_skipped_worktree_does_not_stop_the_pass() {
        let candidates = [
            candidate(0, "~/a", 60),
            candidate(1, "~/b", 40),
            candidate(0, "~/c", 30),
        ];

        let (removed, available) = reclaim(
            &candidates,
            0,
            |_| 100,
            one_by_one(|candidate, available| {
                (candidate.repo == 0).then(|| available + candidate.bytes)
            }),
        );

        assert_eq!(removed, [0, 2]);
        assert_eq!(available, 90);
    }

    #[test]
    fn a_repo_with_a_lower_threshold_is_spared_before_one_with_a_higher() {
        let candidates = [
            candidate(0, "~/a", 50),
            candidate(1, "~/b", 50),
            candidate(0, "~/c", 5),
        ];
        let healthy = |repo: usize| if repo == 0 { 120 } else { 60 };

        let (removed, _) = reclaim(
            &candidates,
            50,
            healthy,
            one_by_one(|candidate, available| Some(available + candidate.bytes)),
        );

        // 50 -> 100 after `a`; repo 1 is healthy at 60, repo 0 is not at 120.
        assert_eq!(removed, [0, 2]);
    }

    #[test]
    fn nothing_is_removed_from_a_healthy_volume() {
        let candidates = [candidate(0, "~/a", 60)];

        let (removed, available) = reclaim(
            &candidates,
            200,
            |_| 150,
            |_, _| panic!("removed from a healthy volume"),
        );

        assert_eq!(removed, Vec::<usize>::new());
        assert_eq!(available, 200);
    }
}
