# ADR 0001: tether keeps the repos an operator lists on their branch tip

- **Status:** Accepted
- **Date:** 2026-10-28

`tether` is a menu-bar app on a developer's Mac that keeps a list of local
clones up to date with one branch each. The operator picks the repos. Each repo either
*follows* its branch (fast-forward only, never destroys anything) or *mirrors* it (shallow,
forced to the branch tip). It is a separate tool from concord, a companion menu-bar app
that keeps one instructions repository and `~/.claude` in sync.

## Context

The tool started inside another repository and moved to its own on 2026-10-28 (owner
decision). It is named `git-tether` so git runs it as `git tether`.

Agents read instructions from more than one repository. A repository of decisions, runbooks
and `AGENTS.md` that nothing pulls is read in whatever state it was last pulled by hand, and
the instructions repository concord keeps had already fallen behind the same way before
concord was installed. (This context was generalised for publication; see
[ADR 0003](0003-public-docs-name-no-private-setup.md).)

Existing tools were checked first (owner question, 2026-10-26):

- **`kubernetes/git-sync`** polls a ref, fetches shallow, checks each commit out into a new
  worktree and swaps a symlink. That moves the repo behind a symlink, discards local work
  and history, and has no UI or controls beyond a metrics endpoint. It only calls webhooks
  outbound.
- **GitOps controllers (Flux, Argo CD)** do the same job inside Kubernetes only.
- **`gitwatch` and similar** commit and push local changes: the opposite direction.
- **concord** already fast-forwards safely and has a tray, but rule 1 fixes it to one repo,
  and all of it is built around `~/.claude`. The owner chose a separate tool over widening
  concord (2026-10-28).

## Decision

### Repos

1. **The operator lists the repos.** `~/.config/tether/repos.toml` holds one `[[repo]]`
   entry per clone: `path`, `branch`, `mode` (`follow` or `mirror`), and optional `remote`
   (default `origin`) and `interval` (default 5 min). `git tether add` / `git tether remove` and the
   tray's *Add repository…* edit the file. Nothing is synced that is not listed.
2. **One writer per clone.** `git tether add` refuses concord's clone (`~/Development/handbook`,
   or `CONCORD_REPO_DIR`) while concord is installed, since ADR 0002 rule 2 gives that
   clone to concord. It also refuses a path that is a linked worktree instead of a main
   clone.

### Polling

3. **Poll, cheaply.** Every `interval`, run `git ls-remote <remote> refs/heads/<branch>`.
   Fetch only when the tip differs from the local remote-tracking ref. There is no inbound
   webhook: GitHub cannot reach a Mac behind NAT without a relay, and a one-request poll
   every few minutes is cheaper than running one.
4. **Locking.** Each repo is synced under a lock file in its git dir, so a tray tick, a
   CLI run and the watcher never overlap. A repo whose `index.lock` exists, or whose lock
   another tether run holds, is *skipped* and retried next round. A repo in the middle of a
   merge, rebase, cherry-pick or bisect is *needs you*: it will not clear by itself.

### `follow` mode (the default)

5. **Fast-forward or skip, never destroy.** After the fetch, fast-forward the branch with
   `git merge --ff-only` only when HEAD is on that branch and no tracked file is modified.
   Untracked files are allowed; `--ff-only` itself refuses to overwrite one. History is
   kept in full.
6. **Skip reasons are states, not errors.** Every skip records why, and the repo shows as
   *needs you*: on another branch, detached, tracked changes, diverged from the remote,
   operation in progress, fetch failed. Tracked changes count only when there is something
   to pull: an edited clone that is already current is ordinary work, not a skip. Commits
   ahead of the remote with nothing to pull are *in sync*. tether never stashes, resets, rebases or switches branches in a `follow`
   repo.

### `mirror` mode (opt-in)

7. **Mirror is for read-only copies.** Each sync fetches `--depth 1` and makes the clone
   exactly the branch tip: `reset --hard` to it and `clean -ffdx`. Local edits, untracked
   and ignored files, and other history are discarded every time.
8. **Adopting a mirror is guarded.** `git tether add --mode mirror` either clones fresh into an
   empty or new path, or adopts an existing clone only when it has no linked worktrees, no
   stash, no local branch other than `branch`, and no commit missing from the remote. Any
   of those makes the add fail with what it found.
9. **A discard leaves a trace.** Before a reset that discards tracked changes, tether saves
   them as a patch under `~/.config/tether/backups/<repo>/<timestamp>.diff` and notifies.
   It prunes the reflog and runs `git gc` after a sync that moved the tip, so the clone
   stays at one commit's size.

### App

10. **One binary, concord's shape.** A Rust core crate with a Tauri v2 tray shell: no Dock
    icon, registered as a login item (`~/Library/LaunchAgents/io.github.rejifald.tether.plist`).
    The same binary is the CLI, linked as `git-tether`: `git tether status [--json]`, `sync [<repo>]`, `sync --due` (the
    tray's tick: only repos whose `interval` elapsed, and nothing while paused), `add`,
    `remove`, `pause`, `resume`. The CLI reads git and the config directly; there is no IPC.
11. **Tray.** The icon has three states: *in sync*, *syncing*, *needs you*. The dropdown
    lists each repo with its branch, mode, last sync and skip reason, plus *Sync now* per
    repo and for all, *Pause*, *Open in Finder* and *Add repository…* (an osascript folder
    picker, so no dialog crate). A notification fires
    only when a repo flips to *needs you*, and when a mirror discarded changes.
12. **Install.** The source lives in this repo. `install.sh`
    builds with `cargo tauri build`, copies the app to `~/Applications`, links the CLI at
    `~/.local/bin/git-tether` and registers the login item, with `--dry-run` and `--uninstall`.
    It runs as the user, with the user's git credentials.
13. **Testability.** `TETHER_CONFIG_DIR` and `TETHER_CONCORD_REPO_DIR` redirect the config
    and the concord check, and `--dry-run` prints planned git commands without running
    them. Integration tests build a bare remote and clones in a temp dir.

## Considered options

- **Widen concord to more repos.** One tray instead of two and its fast-forward code is
  reusable, but it amends ADR 0002 rule 1 and couples plain repo sync to `~/.claude`
  management. The owner chose a separate tool.
- **Use `kubernetes/git-sync`, with or without a tray around it.** Its symlink layout,
  forced sync and missing controls are the problem, and a UI cannot fix them.
- **Mirror only, or follow only.** Mirror-only is wrong for any repo people work in (hq is
  authored in); follow-only cannot keep a disposable copy self-healing. Per-repo mode
  covers both, and `follow` is the default.
- **Inbound webhooks.** They need a relay (`gh webhook forward`, a tunnel) and buy seconds
  over a minutes-scale poll.
- **A shared crate with concord now.** The overlap is small (run git, read status), and
  concord's git code is built around parking. Hoist it when a third tool needs it.
- **Shell script plus launchd.** Fastest to ship, but untestable and no UI.

## Consequences

- hq and any other listed repo stay current without anyone pulling by hand, and a repo
  that cannot be updated says why in the menu instead of going stale silently.
- A Mac with both tools runs two menu-bar apps. They never write to the same clone.
- A `mirror` repo is disposable by definition: anything written into it is gone on the
  next sync, kept only as a backup patch.
- Out of scope for v1: Linux, inbound webhooks, running a command after an update, and
  any idle gate like concord's rule 5.
