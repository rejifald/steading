# ADR 0004: a merged worktree with uncommitted changes is saved to a ref, then removed

- **Status:** Accepted
- **Date:** 2026-11-04
- **Amends:** [ADR 0001](0001-reap-sweeps-merged-worktrees.md), rules 4, 6, 10 and 11, and the
  untracked-files clause of [ADR 0002](0002-merged-pull-request-contains-head.md)

## Context

ADR 0001 rule 4 never removes a merged worktree whose `git status` is not empty, and ADR 0002
kept that rule: an untracked file may be the only copy of something. In practice the dirty
worktrees became the pile-up. Agents leave scratch files, a half-written note or an edit nobody
committed, then their pull request merges and nobody comes back. One operator had 23 merged
worktrees held this way across their repos, taking 34 GB. None of the held changes were on the
base branch, and many were real work: draft ADRs, source edits, a new package. Deleting them was
never an option, and keeping the worktrees forever meant nothing got cleaned up.

Saving them by hand worked. A copy of the worktree's index, `git add -A` into that copy,
`write-tree` and `commit-tree -p HEAD`, stored under a ref in the main clone, saved each
worktree's whole state as one commit. Each save was checked and the worktree removed. This
saved 22 of the 23 and brought the held space down to 780 MB. The one left had a merge under way
with conflicted paths.

## Decision

1. **Saveable.** A merged worktree (rule 3) whose only dirt is changes, with no merge, rebase,
   cherry-pick, revert or bisect under way and no unmerged paths, is *saveable* unless one of
   these holds it:
   - it is locked, in use or kept (rules 5 and 10, unchanged);
   - an untracked folder holds a repository (`.git` inside), which `git add -A` would record as
     a bare gitlink and so lose;
   - its untracked files (symlinks counted by their own size, not their target's) take more than
     `salvage_max_untracked` (default `100MB`), so a stray build output or dataset is never
     copied into the clone's objects.

   A held worktree stays *dirty*, and its row says which guard holds it.
2. **Dirty grace.** A saveable worktree becomes *removable* only once nothing in it has changed
   for `dirty_grace` (default `7d`). This counts the newest modification time among its tracked
   and untracked files (not following symlinks), and its last commit or checkout. A session that
   is only paused is not saved from under itself. The grace clock and notification of rule 10
   then run as for any removable worktree, with *Keep* available, so the operator hears about
   it before anything is removed.
3. **Saving.** Under the clone's lock, right before removal:
   1. copy the worktree's index (`git rev-parse --path-format=absolute --git-path index`) to a
      file beside it, and with `GIT_INDEX_FILE` pointing at the copy run `git add -A` and
      `git write-tree`;
   2. `git commit-tree <tree> -p HEAD`, with the subject
      `git-reap salvage: <path> (<branch>)` and its date set to the pass's clock;
   3. check, against the copy, that `git diff --quiet --ignore-submodules=none` passes and
      `git ls-files -o --exclude-standard` prints nothing, so every change and untracked file is
      in the commit;
   4. `git update-ref refs/reap/salvage/<folder name>/<YYYYMMDD-HHMMSS> <commit> ""` in the
      main clone (create only, never overwrite), and check the ref resolves to that commit;
   5. only then `git worktree remove --force <path>`, then the branch as rule 11 says.

   A step that fails stops the removal. The worktree stays, and the failure is recorded as a
   refusal at its HEAD. `--force` is used only after these checks, and only because they have
   just shown that everything `remove` would refuse to delete is in the ref. git still refuses a
   locked worktree. Ignored files are dropped, as in every removal. `git stash` is never used: it
   is one stack shared by every worktree of the clone, and other sessions push and pop it.
4. **Visible, then pruned.** `git reap status`, `git reap salvaged` and the popover list every
   `refs/reap/salvage/*` ref with its worktree, branch and age, and how to restore it:
   `git worktree add <path> <ref>`, then `git reset HEAD~1` in it to get the changes back as
   uncommitted (the staged/unstaged split is not kept). A sweep under `policy = "auto"` deletes a
   salvage ref whose commit is older than `salvage_max_age` (default `30d`).
5. **Settings.** `dirty_grace`, `salvage_max_untracked` and `salvage_max_age` are in
   `repos.toml`, globally and per repo, like `grace`.

Unchanged: a stale worktree (rule 13, ADR 0003) with changes is still never removed, nothing is
pushed, and a main clone's checkout and index are never touched. The only ref reap writes outside
`refs/remotes/` is under `refs/reap/salvage/`.

## Considered options

- **Keep rule 4 as it was.** It is the safest option, but the dirty worktrees were most of what
  was left, and reap exists to clean those up.
- **`git stash push -u`.** One stack shared across worktrees and sessions, so a concurrent
  `stash pop` elsewhere can apply or drop the entry. Its entries are also hard to name and to list
  per worktree.
- **Archive to a tarball outside git.** It keeps ignored files too, but it is a second store with
  its own pruning. Its files restore without git's own checks, and it brings back the
  `node_modules` reap is meant to free.
- **Push the save to the remote.** Safer against losing the disk, but reap never pushes
  (rule 11), and the changes may hold things that should never leave the machine.
- **The same grace for clean and dirty worktrees.** A day is too short for an operator who comes
  back after a weekend to a session they paused.
- **Keep salvage refs forever.** Then they pile up the way the worktrees did, just out of sight,
  which is what ADR 0002 warned of. With a listing and a fixed expiry, the pile-up stays visible
  and has a limit.

## Consequences

- Merged worktrees with uncommitted changes leave a week after the last edit, plus the grace
  period, and their changes stay restorable for 30 days.
- The clone's object store grows by the saved changes (at most `salvage_max_untracked` of
  untracked data per worktree, plus edits to tracked files) until the ref is pruned and git
  collects the objects.
- Work an operator meant to keep but never committed is deleted 30 days after the save, unless
  they restore it first. `status` lists the refs for the whole of that time.
- `refs/reap/salvage/` is not fetched or pushed by default refspecs, so the saves stay local.
- Files a clean or smudge filter rewrites (Git LFS, `autocrlf`) are saved as git would commit
  them, and come back as git would check them out.
