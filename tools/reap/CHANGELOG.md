# Changelog

All notable changes to this project are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- `git reap discover [--json]`, `skip` and `unskip` ([repo ADR 0004](../../docs/adr/0004-each-tool-suggests-the-clones-it-finds.md)):
  suggest the unlisted clones that have linked worktrees, found by a bounded, read-only walk of
  `scan` in `repos.toml`; `skip` hides one, and `remove` now skips what it unlists. The last scan
  is kept in `discovery.json`.
- `install.sh` (ADR 0001 rule 16): builds reap, shows `status` and the first `sweep --dry-run`,
  asks, then installs the app to `~/Applications`, links `git-reap` and registers the login item;
  `--dry-run`, `--yes` and `--uninstall`.
- The spec, [ADR 0001](docs/adr/0001-reap-sweeps-merged-worktrees.md): which worktrees
  reap sees, when one counts as merged, the guards before removal, the grace period and
  *Keep*, and the app's shape.
- `reap-core` with its first module, a reader for `git worktree list --porcelain -z`.
- `git reap status [--json]`: every linked worktree of the listed clones with its pull
  request and state (*open*, *removable*, *dirty*, *in use*, *locked*, *stale*, *missing*),
  judged by ancestry against the base, by a pull request's target, or by `gh`'s merged head
  SHA (rules 2 to 6, 9, 13). Read-only.
- `git reap sweep [<repo>] [--dry-run] [--due] [--json]`: under the clone's locks (and
  git-tether's, when held), fetch the refs that moved, run the grace clocks in `state.json`,
  and remove what is due: `git worktree remove` without `--force`, then `git branch -d`, or
  `-D` only on `gh`'s proof; prune missing entries after the same grace (rules 7, 8, 10 to 12).
- `git reap keep` / `unkeep`, and the *kept* and *refused* states.
- `git reap add`, `remove`, `pause`, `resume`, and `repos.toml` with its global and per-repo
  settings.
- `gh` is found in Homebrew's folders when the app is started from Finder or at login.
- The menu-bar app: a tray icon with the three states and a React popover listing the
  worktrees by repo, in git-tether's design, with *Keep*, *Unkeep* and *Remove now*. It sweeps
  every interval and when it opens, and notifies when a clock starts, a worktree goes, or git
  refuses.
- Quality gates: format, clippy, tests and the sensitive-info check, in lefthook and CI; CI
  also typechecks, tests and builds the popover.
- Every worktree carries the disk its folder takes (`size_bytes` in `status --json`), counted
  as what removing it would free, so files hard-linked from a package store do not count.
  `git reap status` prints each size and a total line; the popover shows the total in the
  header, the space *Ready to remove* frees, the space held by uncommitted changes, and each
  row's size.

- A merged worktree with uncommitted changes is removed once nothing in it has changed for
  `dirty_grace` (default 7 days) and the usual grace has passed. Its changes are saved first,
  as one commit under `refs/reap/salvage/<folder>/<YYYYMMDD-HHMMSS>` in the main clone, and
  checked; only then is it removed with `--force`. A merge, rebase, cherry-pick or revert under
  way, unmerged paths, an untracked repository inside, or untracked files over
  `salvage_max_untracked` (default 100 MB) hold it, and its row says which
  ([ADR dirty-merged-worktrees-are-salvaged](docs/adr/0004-dirty-merged-worktrees-are-salvaged.md)).
- `git reap salvaged [--json]` lists the saves with the commands that restore each one;
  `status` and the popover's *Salvaged* section show them too. A sweep deletes a save after
  `salvage_max_age` (default 30 days).

- On a low disk a removable worktree goes without waiting out its grace
  ([ADR low-disk-skips-the-grace](docs/adr/0005-low-disk-skips-the-grace.md)): when a volume's
  free space falls below `low_disk` (default `10%`, a size, or `off`), every tick removes
  removable worktrees of `policy = "auto"` repos, largest first, until it has 1.5 times that
  free, then notifies per volume; once per episode it says when nothing is left to remove and
  what holds the space. `git reap status` shows each volume's free space and marks a low one,
  and the tray shows *needs you* while one stays low with nothing to remove.

### Changed

- `git reap sweep --json` prints `{"repos": [...], "volumes": [...]}` instead of the repo
  outcomes alone.
- The popover groups worktrees by outcome across repos (*Needs you*, *Ready to remove*,
  *Merged, but held*, *In progress*, *Kept*), titles each row by its branch, says in a plain
  sentence what happens to it, and offers *Remove all* with a confirm.

### Fixed

- A pass could find a clone busy just after the previous one released its lock, while
  another thread was starting a process. A lock that looks held is now retried for 50 ms,
  even with no wait.
