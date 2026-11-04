<img src="docs/brand/logo.svg" alt="" width="96" height="96">

# git-tether

Run as `git tether`.

Keeps the local clones you list on one branch each. The spec is
[ADR 0001](docs/adr/0001-tether-keeps-listed-repos-on-their-branch.md);
rule numbers below refer to it. It works alongside concord, a companion
menu-bar app that keeps one instructions repository and `~/.claude` in sync,
and never writes to concord's clone.

Each repo either **follows** its branch (fast-forward only, never destroys
anything; the default) or **mirrors** it (shallow, forced to the branch tip,
local changes discarded). One binary: with a subcommand it is the CLI, without
one it is the menu-bar app.

| Path | What |
|---|---|
| `core/` | `tether-core`: config, poll, follow, mirror, status. No UI deps. Also builds `git-tether`, the same CLI without Tauri. |
| `app/` | Tauri v2 menu-bar shell (`tether` binary, also the CLI): tray icon, popover, per-repo timers. |
| `app/ui/` | The popover: React, Base UI and Tailwind, built with Vite into `app/dist/` ([ADR 0002](docs/adr/0002-the-tray-ui-is-a-react-popover.md), [styleguide](docs/styleguide.md)). |
| `install.sh` | Build, install to `~/Applications`, link the CLI as `git-tether`, login item; `--uninstall` removes them. The steps are the repo's [`scripts/install-app.sh`](../../scripts/install-app.sh). |
| `docs/brand/` | The logo and how the app and tray icons derive from it ([brand](docs/brand/README.md)). |

## Install

Needs a Rust toolchain, git, and Node.js with npm (to build the popover; the CLI alone does not need it).

```bash
./install.sh --dry-run
```

```bash
./install.sh
```

It builds, shows `git tether status` and the first `sync --dry-run` plan, and asks
before installing anything; answering no leaves the machine as it was, and a
failing dry run stops the install. On yes it copies the app to
`~/Applications/tether.app`, links the CLI at `~/.local/bin/git-tether`, writes the
login item `~/Library/LaunchAgents/io.github.rejifald.tether.plist` and starts
it. It runs as you, with your git credentials. Then list a repo:

```bash
git tether add ~/Development/notes --branch main
```

```bash
./install.sh --uninstall --dry-run
```

`--uninstall` removes the login item, the CLI link and the app. `~/.config/tether`
and every listed clone stay as they are.

## CLI

```
git tether status [--json]
git tether sync [<repo>] [--dry-run] [--json]
git tether sync --due [--dry-run] [--json]
git tether add <path> --branch <b> [--mode follow|mirror] [--remote <r>] [--interval 5m] [--url <clone-url>]
                [--gate <command>] [--after <command>]
git tether set <repo> --interval <15m>
git tether remove <path>
git tether pause
git tether resume
```

`<repo>` is a listed repo's directory name or its path. Usage errors (a missing
flag, an unknown mode, an interval under 30 s, a repo that is not listed) exit 2.

- `sync` runs every listed repo now, or the one named, and ignores Pause and
  each repo's `gate`. It waits up to 30 s for another run's lock.
- `sync --due` is what the tray's timers do: only repos whose `interval` has
  passed since their last sync and whose `gate` says go, never waiting for a
  lock, and nothing while paused.
- `--dry-run` runs only read-only git (`ls-remote`, `rev-parse`, `status`, …)
  and prints every command that would change a clone, prefixed `would run:`.
  It runs neither `gate` nor `after`, printing `would run gate: …` and
  `would run after: …` instead. It does not fetch, so a plan that needs new
  objects says what it would do once fetched. Nothing is recorded.
- `add` refuses a linked worktree,
  a path that is not a clone's top level, and a branch the remote lacks. A
  `mirror` at a new or empty path is cloned there (`--url` required); an
  existing clone is adopted only with no linked worktree, no stash, no other
  local branch and no commit missing from the remote (rule 8). While concord is
  installed, it refuses concord's clone without a `--gate`, so that clone is
  never pulled under a running session (ADR 0004 rule 6).
- `set --interval` changes how often a listed repo is polled; the new interval
  applies from its next due check.
- `remove` unlists a repo and forgets its last result; the clone is untouched.
- `pause` / `resume` write or delete `~/.config/tether/paused`, which the tray
  and `sync --due` honour.

### What a sync does

A background sync (the tray's timers, `sync --due`) first runs the repo's
`gate`, if it has one, before taking any lock or running any git command; see
[Gate and after](#gate-and-after). Every sync then takes
`<git common dir>/tether.lock` (rule 4), skips while `index.lock` exists or a
merge, rebase, cherry-pick, revert or bisect is under way, then runs
`git ls-remote <remote> refs/heads/<branch>` and fetches only when that tip
differs from `refs/remotes/<remote>/<branch>` (rule 3). When the sync moved the
tip, it releases the lock and runs the repo's `after`.

- **follow:** `git merge --ff-only` when HEAD is on the branch and no tracked
  file is modified. Untracked files are fine. Skips, with the reason shown as
  *needs you*: another branch, detached HEAD, tracked changes while behind,
  diverged, operation in progress, fetch failed, the remote lacks the branch,
  or `--ff-only` refused (an untracked file in the way). Ahead with nothing to
  pull is *in sync*; so is a clone with local edits and nothing to pull. A
  clone the next sync will fast-forward is *behind*, with the number of
  commits to pull, in `status`, the tray row and a dry run.
- **mirror:** fetch `--depth 1`, then `reset --hard` (or a forced checkout when
  HEAD is elsewhere) and `clean -ffdx`. Tracked changes are first saved to
  `~/.config/tether/backups/<repo>/<timestamp>.diff`. When the tip moved, tags
  and other remote-tracking refs are deleted, the reflog expired and
  `git gc --prune=now` run, so the clone stays at one commit (rule 9).

## Config

`~/.config/tether/repos.toml` (rule 1); `add`, `set` and `remove` edit it, and so can you.

```toml
[[repo]]
path = "~/Development/notes"
branch = "main"
# mode = "follow" is the default

[[repo]]
path = "~/Mirrors/docs"
branch = "main"
mode = "mirror"
remote = "origin"          # optional, default origin
interval = "15m"           # optional: <n>s, <n>m or <n>h; default 5m, minimum 30s
url = "https://github.com/example/docs.git"  # mirror only: where to clone a missing path from
```

A bad entry (relative path, unknown key, duplicate path, interval under 30 s,
an empty `gate` or `after`) fails the whole file rather than silently dropping a repo.

### Gate and after

Two optional keys let another tool work with a clone tether keeps current,
without tether knowing anything about that tool ([ADR 0004](docs/adr/0004-repos-can-gate-and-follow-up-a-sync.md)):

```toml
[[repo]]
path = "~/Development/handbook"
branch = "main"
gate = "my-tool idle"        # asked before a background sync
after = "my-tool reconcile"  # run after a sync moved the tip
```

Both run with `sh -c` in the clone's directory, with your own rights, like a
git hook; they come only from `repos.toml`, never from a clone. Their
environment has `TETHER_REPO` (the path as listed, with `~`) and
`TETHER_BRANCH`; `after` also gets `TETHER_FROM` and `TETHER_TO`, the tips
before and after (`TETHER_FROM` is empty for a fresh mirror clone).

- **`gate`** runs before a background sync, before the lock and before any git
  command. Exit 0 means go. Any other exit, or no exit within 30 s (the command
  and everything it started are then killed), skips the repo this round as
  *skipped*, with the command's first line of output as the reason; the next
  round asks again. An explicit sync (*Sync now*, *Sync all*,
  `git tether sync [<repo>]`) does not ask it.
- **`after`** runs when a sync moved the tip, once the lock is released, so the
  command may take the lock itself. It never runs for a sync that skipped,
  failed or found nothing to pull. A non-zero exit marks the repo *needs you*
  with its first line of output. It stays so until a later sync that moves the
  tip runs `after` successfully (from the last tip it succeeded at, so nothing
  is missed), or you fix the cause and run an explicit sync, which runs it again
  for the failed move.

**The shared lock.** Every tether sync holds an exclusive OS file lock
(`flock`) on `<git common dir>/tether.lock` for its whole length. Any other
tool that writes to a listed clone must take the same lock first, so it and a
pull never overlap. The lock is released when its holder exits, however it
exits, so there is no stale lock to clean up.

Also in the config dir: `state.json` (last sync, state and reason per repo),
`paused`, and `backups/`.

## Environment overrides (rule 13)

| Variable | Default |
|---|---|
| `TETHER_CONFIG_DIR` | `~/.config/tether` |
| `TETHER_CONCORD_REPO_DIR` | concord's `CONCORD_REPO_DIR`, else `~/Development/handbook`: the clone `add` refuses while concord is installed (`~/Applications/concord.app` or `~/Library/LaunchAgents/io.github.rejifald.concord.plist` exists) |
| `TETHER_GIT` | `git` on `PATH` (tests use a logging wrapper) |

## Menu (rule 11, ADR 0002)

The icon is the cube (*in sync*), the cube as a wireframe (*syncing*) or the
cube in orange (*needs you*, with a count); a *behind* repo draws as in sync,
since the next tick catches it up.

A left click opens the popover under the icon. Its header shows the overall
state and *Paused*; each repo has a row with its name, branch, mode, state and
detail (commits to pull, the skip reason, or the last sync), with *Sync now* and
*Open in Finder* on hover. *Needs you* rows come first. The footer has *Sync
all*, *Pause*/*Resume* and *Add repository*, a form in the popover: a folder
(*Choose…* opens the macOS folder picker), the branch, defaulting to the one
checked out, the mode, defaulting to `follow`, and a clone URL for a mirror at
an empty folder. The popover hides on Escape and when it loses focus; Cmd+Q
quits. A right click shows a native menu with *Sync all now* and *Quit tether*,
so a broken web view never leaves tether without a way out. A notification
fires only when a repo flips to *needs you*, and when a mirror discarded
tracked edits or untracked files.

All git runs on one worker thread; clicks and the popover's commands only queue
a request, and requests within 300 ms are coalesced, so the UI never blocks on
the network. The popover receives the same report as `git tether status
--json`. The folder picker and notifications use `osascript` (`choose
folder`), like concord, so the app needs no Tauri dialog or notification plugin.

## Architecture

The spec, with numbered rules, is [ADR 0001](docs/adr/0001-tether-keeps-listed-repos-on-their-branch.md).

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

The popover (`app/ui/`):

```bash
npm --prefix app/ui ci
npm --prefix app/ui test -- --run
npm --prefix app/ui run typecheck
npm --prefix app/ui run dev
```

`run dev` serves the popover in a plain browser against an in-memory fake of the
app's commands (`app/ui/src/api.fake.ts`). `cargo tauri build` builds the UI into
`app/dist/` itself; a plain `cargo build -p tether-app` without that step embeds
a placeholder page that says how to build it.

The integration tests (`core/tests/`) build a bare remote, a seed clone and
clones under a fake `$HOME` in a temp dir, with git's global config replaced
and a `git` wrapper that logs every command, so they can assert that a dry run
runs nothing mutating and that an unchanged remote is not fetched.

### CI

`.github/workflows/ci.yml` runs on a self-hosted runner (labels `faber`,
`docker`), a throwaway Linux container per job: the sensitive-information scan,
installer syntax, formatting, clippy and tests for `tether-core` (the core and
the CLI), and the UI's typecheck, tests and build. The Tauri app crate is
macOS-only, so CI does not build it; the lefthook hooks check the whole
workspace on every commit and push, so install them (`lefthook install`).

That runner serves only private repositories. **When this repository goes
public, CI moves to GitHub-hosted runners** (free for public repositories, and
no code from outside runs on a self-hosted machine): `runs-on` becomes
`macos-latest`, which also lets CI build and lint the app crate. Do that in the
same change that makes the repository public.

## Known gaps

- **Other local branches in a mirror.** Adoption refuses a clone that has any,
  but one created later is left alone by the sync (and keeps its history), so
  that clone is no longer one commit's size.
- **Untracked files are not in the backup patch.** Only tracked changes are
  saved before a mirror sync; untracked and ignored files are discarded, as
  rule 7 says.
- **Network failures in `status`.** `status` is offline: it judges against the
  remote-tracking ref and shows a fetch failure only as recorded by the last sync.
- **`add` needs the remote.** It checks the branch with `ls-remote`, so it fails
  offline.
- **Interval timing** is checked at least once a minute; a repo is synced up
  to a minute after it falls due, and the tray must be running (or a
  `git tether sync --due` cron/launchd job) for timers to fire.
- **`after` has no timeout.** A command that never exits holds that sync, and
  the tray's next round, until it does.
- Linux and inbound webhooks are out of scope (ADR).
