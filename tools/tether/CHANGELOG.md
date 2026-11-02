# Changelog

All notable changes to this project are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Storybook for the popover (ADR 0005): every panel of the approved design boards is a story,
  dark and light from a toolbar switch, with `play` steps for forms, focus and menus. Dev-only;
  the app bundle is unchanged.
- A repo can set `gate` and `after` commands in `repos.toml` (ADR 0004), so another tool can
  work with a clone tether keeps current. A background sync asks `gate` first and skips the
  repo this round unless it exits 0 within 30 s; an explicit sync does not ask it. `after` runs
  once a sync moved the tip and the lock is free, with `TETHER_FROM` and `TETHER_TO`; a failure
  makes the repo *needs you* until a later run succeeds. `git tether add` takes `--gate` and
  `--after`, the popover's add form has them under *Add commands…*, and `status --json`
  reports both. `<git common dir>/tether.lock` is now the documented lock any tool writing to
  a listed clone takes.
- concord's clone can be listed while concord is installed, but only with a `gate`.
- Each repo in the popover has *Settings* (an icon on its row or callout): change how
  often it is pulled, or *Remove from tether* after a confirm (the clone stays). The
  add form also sets *Pull every*. The CLI gains `git tether set <repo> --interval`.

### Changed

- The tray's dropdown is now a popover (ADR 0002): a left click on the icon opens
  a window with the overall state, one row per repo (state, detail, *Sync now*,
  *Open in Finder*; *needs you* first) and *Sync all*, *Pause*/*Resume* and *Add
  repository*. Adding a repo is one form (folder, branch, mode, clone URL for a
  mirror) instead of three prompts. A right click shows a native menu with *Sync
  all now* and *Quit tether*. Building the app now needs Node.js and npm; the CLI
  does not.
- Opening the popover re-reads every clone (local git only), so a change made a
  moment ago, such as switching branches, shows without waiting for the next tick.
- A repo that needs you now says what to do about it. `git tether status`, the
  notification and the popover show a next step under the reason ("Switch back to
  release and tether resumes."), and `status --json` carries it as `hint`. The
  popover leads with *Open in Finder* unless a sync is what clears the reason.

### Fixed

- A sync could be skipped as busy just after the previous one released its lock, while
  another thread was starting a process. A lock that looks held is now retried for 50 ms,
  even with no wait.
- A clone short of its remote tip showed as *in sync* in `status`, the tray and
  `sync --dry-run`. It is now *behind*, with the commits the next sync will pull
  (`behind` in `status --json`). Both modes.

## [0.1.0] - 2026-10-28

### Added

- Moved into its own repository. The CLI is `git-tether`, run by
  git as `git tether`; the tray app, `~/Applications/tether.app`, the login item and
  `~/.config/tether` keep the name `tether`.
