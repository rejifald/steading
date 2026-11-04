# ADR 0006: an abandoned worktree is saved to a ref, then offered for removal

- **Status:** Proposed
- **Date:** 2026-11-05
- **Amends:** [ADR 0001](0001-reap-sweeps-merged-worktrees.md), rules 3, 13 and 15;
  [ADR 0003](0003-pushed-stale-worktrees-are-removed.md), rule 3; and
  [ADR 0004](0004-dirty-merged-worktrees-are-salvaged.md), rule 4 and its closing clause

## Context

reap exists to get rid of the worktrees nobody needs any more: merged, stale, unused and
abandoned. After ADRs 0002 to 0004 it removes the merged ones well. On one operator's Mac it had
24 merged worktrees queued for removal, and every one was right. But two kinds still never leave:

1. **Merged work reap cannot see.** Rule 3 matches a pull request by the worktree's branch name.
   Two common worktrees have no pull request under that name:
   - **Detached worktrees.** A review fixer or a sub-agent checks out a commit detached, pushes
     to a pull request's branch and stops. The worktree has no branch, so reap finds no pull
     request.
   - **Branches merged through another branch.** Several issue branches are merged into one batch
     branch, and the batch's pull request merges. Each issue branch's commits are in a merged pull
     request, but not one whose head is that branch.

   On the same Mac, 20 worktrees taking 19.5 GB were in this state: 11 detached and 9 on such
   branches. For each one, GitHub's compare API showed HEAD to be an ancestor of a merged pull
   request's head, and none had uncommitted changes. reap showed all 20 as *open, not merged*. In
   two weeks they turn stale. ADR 0003 then removes the 9 on branches, as abandoned rather than
   merged, but the 11 detached ones stay forever, since ADR 0003 leaves out a detached HEAD.
2. **Abandoned work with something local in it.** A stale worktree (rule 13) with uncommitted
   changes, commits on no remote, a detached HEAD or an open pull request is never removed. Its
   row says "reap won't remove it" without saying why, and the operator has no way out but a
   terminal. The two such worktrees on that Mac held 2.9 GB, and each held work that existed
   nowhere else: 22 commits never pushed and two docs pages missing from the base branch in one;
   two new components missing from its open pull request in the other. Deleting them by hand
   would have lost that work. Keeping them forever is the pile-up reap exists to clear.

[ADR 0004](0004-dirty-merged-worktrees-are-salvaged.md) already has the tool for the second case:
it saves a worktree's whole state as one commit under `refs/reap/salvage/` and checks it before
removing anything. That commit's parent is HEAD, so the ref also keeps every commit under HEAD,
pushed or not, and a detached HEAD. ADR 0004 leaves stale worktrees out on purpose: their work is
not merged anywhere, and its 30-day expiry would then delete the only copy.

## Decision

### Merged, found by commit

1. **Rule 3 gains (d): a merged pull request contains HEAD, whatever its branch.** When (a) to (c)
   do not hold, reap asks GitHub for the pull requests associated with HEAD. It uses one batched
   GraphQL call per pass for every such worktree in a repo (`object(oid:)` with
   `associatedPullRequests`, up to 100 commits per call, as ADR 0002 rule 3 batches branches). A
   merged pull request in the answer counts only when ADR 0002 rule 1's check passes: HEAD is an
   ancestor of, or equal to, its merged head, shown by `git merge-base --is-ancestor` or by the
   compare API. A commit GitHub does not have comes back `null`, and a failed call counts as
   "not merged".
2. **Branches follow ADR 0002 rule 2.** A branch whose tip equals HEAD, where (d) holds, is
   deleted with `git branch -D`, since the merged pull request then holds every commit on it. A
   detached worktree has no branch to delete.
3. **The popover shows the pull request it found** (rule 15): `in #212, merged`, with its link,
   in place of the branch's own pull request. Rule 15's "only pull requests whose head is a
   worktree's branch" becomes "only pull requests that contain a worktree's HEAD".

A worktree merged under (d) is then like any merged worktree: removed after the grace period if
clean, or saved and removed under ADR 0004 if dirty. reap checks (d) before staleness, so merged
work is never offered as abandoned.

### Abandoned, saved, then offered

4. **Abandoned:** a stale worktree (rule 13) that ADR 0003 does not remove because something in
   it is local: uncommitted changes, commits on no remote, a detached HEAD, or an open pull
   request.
5. **Saveable unless a guard holds it.** The guards are those of ADR 0004 rule 1: locked, in use
   or kept (rules 5 and 10); a merge, rebase, cherry-pick, revert or bisect under way; unmerged
   paths; an untracked folder holding a repository; or untracked files above
   `salvage_max_untracked`. A held worktree stays stale.
6. **Offered, never removed on its own.** Under every `policy`, reap never removes an abandoned
   worktree by itself: there is no grace clock and no `sweep` removes it. It notifies once, when
   the worktree first becomes abandoned and saveable, and the row offers **Save and remove**
   beside *Keep*. The CLI's equivalent is `git reap salvage <worktree>`, and `--dry-run` prints
   its steps as `would run:`. The operator confirms each one.
7. **Saving is ADR 0004 rule 3, always.** Under the clone's lock, reap checks the guards of rule
   5 and the facts on the row again, then follows ADR 0004 rule 3 steps 1 to 4. A clean worktree
   gets a commit with HEAD's tree, so every abandoned worktree has a ref. The commit adds a
   trailer `Reap-Reason: abandoned (<stale reason>)`. Then reap runs
   `git worktree remove --force <path>`. **The branch is kept**, as in ADR 0003 rule 2. A step
   that fails stops the removal and is recorded as a refusal, as in ADR 0004.
8. **An abandoned save does not expire.** ADR 0004 rule 4's `salvage_max_age` prunes only saves
   of merged worktrees. A ref whose commit carries `Reap-Reason: abandoned` stays until the
   operator deletes it: by *Delete* on its row in the saved list, or with
   `git reap salvaged --delete <ref>`. `git reap salvaged` and the popover list abandoned saves
   apart from merged ones, with their age, size and how to restore them.
9. **A held stale row says what holds it.** Every stale row names what is local and which guard,
   if any, holds it, for example: `no commit or checkout for 119 days · 13 uncommitted changes ·
   22 commits on no remote`, or `… · pull request #15 open · a merge is in progress`. "reap
   won't remove it" appears only when a guard holds it, and then names the guard.

Unchanged: reap never pushes, never deletes a remote branch, and never touches a main clone. An
abandoned worktree is never removed without the operator's confirmation, a worktree in use or
locked is never saved, and ignored files are dropped, as in every removal.

## Considered options

- **Keep rule 13 and ADR 0003 as they are.** It is the safest option, but the abandoned
  worktrees stay forever, and the operator either deletes them by hand, losing what is local, or
  keeps them. Both are what reap exists to prevent.
- **Remove abandoned worktrees on a grace clock, like merged ones.** The save loses nothing, but
  the work is not merged anywhere and someone may still mean to come back to it. Staleness is a
  guess about intent, and a merge is a fact. A guess gets an offer, not a removal.
- **Save abandoned work under ADR 0004's 30-day expiry.** Then the only copy of unmerged commits
  disappears a month after a click. A save is a commit and its objects, with no `node_modules`,
  so keeping it costs little, and the listing keeps it visible.
- **Push the save to the remote.** It would survive the disk, but reap never pushes (rule 11),
  and uncommitted changes may hold things that should never leave the machine.
- **Delete the branch of an abandoned worktree.** The salvage ref holds its commits, but a branch
  is how the operator finds and resumes work, and deleting it frees nothing measurable.
- **Find merged work by GitHub's search** (`gh pr list --search <sha>`). It also matches a SHA
  quoted in a pull request's text: in the same audit it reported a worktree as merged whose HEAD
  GitHub did not have. `associatedPullRequests` is the commit's own record, and ADR 0002's
  ancestry check then confirms it.
- **One REST call per commit** (`repos/<owner>/<name>/commits/<sha>/pulls`). It gives the same
  answer as the GraphQL field, but costs a call per worktree per pass, which ADR 0002 rejected
  for branches for the same reason.
- **Patch-id or tree comparison for branches merged through another branch.** ADR 0001 rejected
  it as a guess, and (d) finds the same worktrees from GitHub's record.

## Consequences

- Detached worktrees and branches merged through a batch leave with the rest of the merged work,
  a grace period after reap first sees them merged. On the Mac above that was 20 worktrees and
  19.5 GB.
- An abandoned worktree no longer stays forever. It waits for one confirmation, and its work
  stays restorable from a ref with `git worktree add <path> <ref>`, then `git reset HEAD~1`.
- Abandoned saves pile up in `refs/reap/salvage/` until the operator deletes them. They are
  small, local and listed, so the pile-up stays visible.
- A pass costs one more GraphQL call per repo while some worktree is unmerged under (a) to (c),
  and one compare call for each merged pull request whose head is not in the clone.
- reap's safety rules gain a second `--force`: the confirmed removal of a saved abandoned
  worktree. When this is accepted, the safety rules in `tools/reap/CLAUDE.md` and ADR 0001's
  status line say so.
- A commit whose pull request came from a fork, or that GitHub does not have, is still not found
  by (d). Its worktree turns stale and is offered as abandoned, with its commits saved.
