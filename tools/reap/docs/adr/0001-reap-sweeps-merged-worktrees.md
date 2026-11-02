# ADR 0001: reap removes the worktrees whose work has merged

- **Status:** Accepted; rules 3(c), 9 and 11 amended by
  [ADR 0002](0002-merged-pull-request-contains-head.md), rule 13 by
  [ADR 0003](0003-pushed-stale-worktrees-are-removed.md)
- **Date:** 2026-10-28

`reap` is a menu-bar app on a developer's Mac that finds the linked worktrees of the clones
the operator lists, shows each one with its pull request, and removes the ones whose work has
merged once a grace period has passed. It never touches a main clone or a remote, and it never
removes a worktree that is dirty, in use, or merely abandoned. It is named `git-reap`, so git
runs it as `git reap`; `gws` was rejected because a Google Workspace CLI in Homebrew already has
that name.

## Context

Agents work in linked worktrees, one per task, and so do people running several branches at
once. Creating a worktree is one command; nothing removes it once its pull request merges. Each
one keeps a checkout plus its build output (`target/`, `node_modules/`), and its branch stays
checked out, so `git switch` to that branch elsewhere fails. git itself only prunes the entry
of a worktree whose folder is already gone, and `git gc` does even that only after three
months.

[git-tether](https://github.com/rejifald/git-tether) is the companion tool that keeps listed
clones on their branch tip. The owner chose a separate app over widening it (2026-10-28): the
repos to keep current and the repos to keep slim are different lists, and keeping worktrees
slim is a different responsibility. reap borrows tether's shape, and coordinates with it
through tether's lock (rule 7).

## Decision

### Repos and worktrees

1. **The operator lists the repos.** `~/.config/reap/repos.toml` holds one `[[repo]]` entry
   per main clone: `path`, and optional `remote` (default `origin`) and `base` (default the
   remote's default branch, `<remote>/HEAD`). `git reap add` / `git reap remove` and the tray's
   *Add repository…* edit the file. `add` refuses a path that is not a clone's top level or
   that is itself a linked worktree. Nothing in a clone that is not listed is ever touched.
2. **Every linked worktree of a listed clone is in scope.** reap reads them with
   `git worktree list --porcelain`. The main worktree is never removed. Worktrees another tool
   created, such as the Claude desktop app's `<repo>/.claude/worktrees/<name>`, follow the
   same rules as the rest; rule 5 spares one a live session is using.

### When a worktree is merged

3. **Merged means one of three things,** checked against its HEAD:
   - **(a)** HEAD is an ancestor of the base ref, `<remote>/<base>`;
   - **(b)** HEAD is an ancestor of the remote-tracking ref of the branch its pull request
     targets, as `gh` reports it. This covers a cascade: a worktree merged into another
     feature branch that has not merged yet;
   - **(c)** `gh` reports its pull request merged, with a head SHA equal to HEAD. This covers
     squash and rebase merges, whose commits are never ancestors of the target.
     [ADR 0002](0002-merged-pull-request-contains-head.md) widens this to a merged head that
     HEAD is an ancestor of.

   A worktree whose branch merely has another branch stacked on top of it is not merged:
   ancestry counts only against the base and a pull request's own target, not against any
   branch that contains it. There are no patch-id or tree-comparison heuristics.
4. **Clean, with nothing under way.** `git status --porcelain` in the worktree is empty
   (untracked files count; ignored files do not, and `git worktree remove` deletes them), and
   no merge, rebase, cherry-pick, revert or bisect is in progress.
5. **Not in use.** The worktree is not `git worktree lock`ed, and no process has its current
   directory inside it. reap runs `lsof -d cwd` once per pass and matches paths by prefix.
6. **Removable** is merged (3), clean (4), not in use (5), and not kept (rule 10). Only a
   removable worktree is ever removed.

### Polling

7. **One pass per repo, under a lock.** Every `interval` (default 5 min, minimum 1 min) and
   when the popover opens, reap takes `<git common dir>/reap.lock`. A repo whose
   `tether.lock` or `index.lock` exists is skipped and retried next pass, so reap and tether
   never write the same clone at once.
8. **Fetch only what moved.** A pass runs `git ls-remote` for the refs rule 3 needs (the base
   and every open or merged pull request's target), and fetches only those whose remote tip
   differs from the local remote-tracking ref. It writes remote-tracking refs only. A stale
   ref can only delay a removal, never cause one.
9. **`gh` is optional.** One batched `gh pr list --json` per repo per pass supplies pull
   request state, targets, head SHAs and CI. Without `gh`, without authentication, or for a
   remote that is not on GitHub, reap uses rule 3(a) alone and its status says so and why.

### Removal

10. **The operator picks the policy; the default removes after a grace period.** `policy` is
    `auto` (the default), `ask` (notify, remove only on confirm) or `off` (show, never
    remove), with `grace` (default 24 h), globally and per repo. The grace clock starts when
    reap first sees a worktree removable, and the notification goes out then, so a worktree
    always gets the full warning even after reap was not running. The clock lives in
    `~/.config/reap/state.json`, keyed by worktree path and HEAD SHA, and restarts if the
    worktree stops being removable. *Keep* holds a worktree until its HEAD moves; new commits
    end the Keep and start a fresh cycle. *Unkeep* ends it by hand.
11. **Remove with git's own guards.** `git worktree remove <path>`, never `--force`, then
    `git branch -d <branch>`. `git branch -D` is used only when rule 3(c) holds with a head
    SHA equal to the branch tip, since the merged pull request then holds every commit on the
    branch; otherwise a branch `-d` refuses stays, shown as *merged, branch kept*. reap never
    pushes, deletes a remote branch, or changes a main clone's checkout. A git refusal is
    shown on the worktree's row and in a notification, and is not retried with more force.
12. **Missing folders are pruned after the same grace.** An entry whose folder no longer
    exists gets the grace clock of rule 10; when every such entry in a repo has passed it
    and none is kept, reap runs `git worktree prune`. A locked entry is never pruned, so a
    worktree on an external drive should be locked.
13. **Stale is flagged, never removed.** A worktree is *stale* when its pull request closed
    without merging, its remote branch was deleted while unmerged, or it has had no commit
    and no file change for `stale_after` (default 14 days). The operator decides what to do
    with it.
    [ADR 0003](0003-pushed-stale-worktrees-are-removed.md) removes a clean stale worktree
    whose commits are all on the remote, keeping its branch.

### App

14. **One binary, tether's shape.** A Rust core crate (`reap-core`, no UI dependencies) with
    a Tauri v2 tray shell: no Dock icon, a login item
    (`~/Library/LaunchAgents/io.github.rejifald.reap.plist`). The same binary is the CLI,
    linked as `git-reap`: `git reap status [--json]`, `sweep [<repo>] [--dry-run] [--json]`,
    `sweep --due` (the tray's tick), `add`, `remove`, `keep <worktree>`, `unkeep <worktree>`,
    `pause`, `resume`. `--dry-run` prints every command that would change a repo, prefixed
    `would run:`, and runs none. The CLI reads git and the config directly; there is no IPC.
15. **The popover is the worktree view.** A React popover, as in tether's ADR 0002, lists
    every worktree of every listed repo: branch, pull request number and state, CI, age,
    dirty, and its reap state (*removable at <time>*, *kept*, *in use*, *stale*, *merged,
    branch kept*). Each row offers *Keep* / *Unkeep*, *Remove now* (removable rows only, same
    guards, no grace), and *Open in Finder*. It is not a general pull request dashboard:
    only pull requests whose head is a worktree's branch appear. The tray icon has three
    states: *tidy*, *removal pending*, and *needs you* (stale, or a git refusal).
16. **Install.** `install.sh` builds with `cargo tauri build`, copies the app to
    `~/Applications`, links the CLI at `~/.local/bin/git-reap` and registers the login item,
    with `--dry-run` and `--uninstall`. It runs as the user, with the user's git and `gh`
    credentials.
17. **Testability.** `REAP_CONFIG_DIR` redirects the config and state, the clock and `gh` are
    injected into the core, and integration tests build a bare remote, clones and worktrees
    in a temp dir with a fake `gh` that answers from fixtures.
18. **Code is copied from tether, not shared yet.** reap starts from copies of tether's git,
    lock and path modules and its tray and popover skeleton. Once reap ships, the overlap
    that stayed identical moves into a shared crate in its own repository, consumed by both
    as a git dependency.

## Considered options

- **Part of tether, or a "repo toolbox" holding both.** One tray instead of two, but the two
  tools keep different repo lists with different jobs; the owner chose separate apps.
- **Ask before every removal as the default.** The safest option, but it leaves the operator
  doing the cleanup the tool exists for. It stays available as `policy = "ask"`.
- **Patch-id or tree comparison for squash merges.** It catches squash merges without `gh`,
  but it guesses: a cherry-picked subset or a rewritten pull request looks merged. `gh`'s
  head SHA is a fact.
- **Scan a root folder, or reuse tether's list.** A scan makes any new clone sweepable the
  moment it appears; tether's list is the wrong set of repos and couples the two configs.
- **Skip or only flag worktrees another tool created.** They are most of the pile-up; rule 5
  already spares those still in use.
- **`git branch -d` only.** It refuses every squash-merged branch, so those branches would
  pile up instead of the worktrees.
- **Merged into any pushed branch, or any local worktree's branch.** Both work without `gh`,
  but both sweep the bottom of a stack while it is still being worked on, and a local branch
  may hold the only copy of the commits.
- **Never fetch.** reap would write no refs, but merges into targets nothing fetches would go
  unseen.
- **Require `gh`, or call it only on demand.** Required rules out repos hosted elsewhere and
  stops the tool on a login problem; on demand starts the grace clock only after someone
  looks.
- **Start the grace clock at the pull request's `mergedAt`.** No state file for the clock, but
  after reap has been off for a day it removes worktrees with no warning, and an ancestry
  merge has no timestamp.
- **Keep forever.** Simpler, but kept worktrees pile up quietly.
- **Age as the only stale signal.** It misses a closed pull request or a deleted remote
  branch, the clearest signs of abandoned work.
- **A shared crate with tether first, or one Cargo workspace.** A crate extracted before reap
  exists guesses at what reap needs and changes tether first; one workspace undoes the
  separate-app decision and ties their releases together.

## Consequences

- Worktrees whose pull request merged disappear a day later without anyone cleaning up, with
  a notification first and a Keep for exceptions.
- A worktree with no commits of its own and a clean tree is an ancestor of its base, so it
  counts as merged under rule 3(a) and is removed after the grace period unless something
  runs inside it. Nothing is lost: it has no commits and no changes.
- An agent session whose merged worktree was removed cannot be resumed in place; its branch,
  or its merged pull request, is where the work lives.
- Without `gh`, squash-merged and cascade-merged worktrees are never removed; they only show
  as unmerged.
- A Mac with both tools runs two menu-bar apps that share one lock rule and never write the
  same clone at the same time.
- About 400 lines of tether's core and its tray skeleton are duplicated until the shared
  crate exists.
- Out of scope for v1: Linux, removing stale worktrees, deleting remote branches, pull
  requests whose head is not a worktree's branch, and inbound webhooks.
