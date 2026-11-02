# ADR 0004: a repo can gate its sync and run a command after it

- **Status:** Accepted
- **Date:** 2026-10-28
- **Amends:** [ADR 0001](0001-tether-keeps-listed-repos-on-their-branch.md) rule 2 (concord's
  clone) and its out-of-scope line on running a command after an update.

## Context

concord keeps one instructions repository in sync with `~/.claude`. Until now it also pulled
that clone itself, so ADR 0001 rule 2 made tether refuse it: one writer per clone. The owner
decided (2026-10-28) that tether pulls every clone, concord's included, and that concord keeps
only its `~/.claude` work. concord's work does not disappear around a pull, though:

- **Before** a pull it waits until no Claude session is active, so instructions never change
  under a running session, and it parks edits to shared files on `pending/*` branches so the
  clone is clean.
- **After** a pull it rebases those branches onto the new tip and relinks `~/.claude`.

tether must make room for that without learning anything about Claude: it is a general tool.

## Decision

1. **Two optional per-repo commands.** A `[[repo]]` entry may set `gate` and `after`, each a
   shell command run with `sh -c` in the clone's directory, with `TETHER_REPO` (the path as
   listed), `TETHER_BRANCH`, and for `after` also `TETHER_FROM` and `TETHER_TO` (the old and
   new tip) in its environment.

   ```toml
   [[repo]]
   path = "~/Development/handbook"
   branch = "main"
   gate = "concord idle"
   after = "concord reconcile"
   ```

2. **`gate` decides whether a background sync runs now.** It runs before the lock and before
   any git command. Exit 0 means go. Any other exit, or no exit within 30 seconds, skips the
   repo this round as *skipped*, with the gate's first line of output as the reason, and the
   next round asks again. The gate applies to the tray's timers and `sync --due`. An explicit
   sync (*Sync now*, *Sync all*, `git tether sync [<repo>]`) does not ask it, the way concord's
   own *Sync now* skips its idle gate.
3. **`after` runs when a sync moved the tip, once the lock is released,** so the command can
   take the lock itself. A non-zero exit marks the repo *needs you* with the command's first
   line of output, until the next sync that moves the tip runs `after` successfully, or the
   owner fixes it and runs *Sync now*. It never runs for a sync that skipped, failed, or found
   nothing to pull.
4. **A dry run runs neither command;** it prints `would run gate: …` and `would run after: …`.
5. **One lock, shared by name.** Everything that changes a clone holds an exclusive OS file
   lock on `<git common dir>/tether.lock` for its whole length. A companion tool that writes to
   the clone (concord's park and rebase) takes the same lock, so it and a pull never overlap.
6. **Rule 2 changes.** concord's clone may be listed, but only with a `gate`: `git tether add`
   refuses it otherwise while concord is installed, so the clone is never pulled under a
   running session by accident. A linked worktree is still refused.
7. **Rollout order.** tether ships `gate` and `after` first, still refusing concord's clone
   without a gate. concord then ships `concord idle`, `concord reconcile` and the shared lock,
   and stops fetching and fast-forwarding a clone that tether lists. Only then is the clone
   listed. A machine with the old concord keeps the old arrangement.

## Considered options

- **The Claude idle signal built into tether** (`idle = "10m"`, reading `~/.claude/sessions`).
  Fewer moving parts, but Claude Code's undocumented session format would be parsed by two
  tools, and a general git tool would carry Claude-specific code.
- **concord drives tether**, calling `git tether sync` for its clone when idle. No new tether
  config, but the clone tether's popover shows would be the one it cannot keep current by
  itself, and concord would stay the scheduler.
- **A file watcher instead of `after`.** concord already watches its clone, but a watcher
  sees ref changes from any source and can miss or merge bursts; an explicit command after a
  known move is simpler to reason about and to test.

## Consequences

- tether gains a general extension point any operator can use (a build step after a pull, a
  gate on a VPN or on battery power), which ADR 0001 had left out of scope.
- A gate or `after` command runs with the operator's own rights, from their own config file,
  like a git hook. The commands come only from `repos.toml`, never from a clone.
- concord's side needs its own decision record in its repository, amending its rule 5 (timing)
  and the parts of its sync that fetch and fast-forward.
