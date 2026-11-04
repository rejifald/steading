# 0004. Each tool suggests the clones it finds; the operator lists them

- Status: Proposed
- Date: 2026-11-04

## Context

Both tools act only on the clones the operator lists (tether's ADR 0001 rule 1, reap's ADR 0001
rule 1). Today the only way to list one is `git <tool> add <path>` or *Add repository…*, a folder
picker. On a machine with dozens of clones that means one picker trip per clone. Clones made after
setup are never listed, because nothing tells the operator they exist. For reap that is where
worktrees pile up unnoticed: a clone nobody listed is a clone nobody sweeps.

Reap's ADR 0001 rejected "scan a root folder" because then any new clone would be sweepable as
soon as it appeared, and nobody would have chosen it. This ADR keeps that rule. A scan runs only
when the operator starts one, it only *proposes* clones, and a clone is listed only when the
operator accepts it.

Constraints:

- macOS shows a privacy (TCC) prompt the first time an app reads `~/Desktop`, `~/Documents`,
  `~/Downloads`, iCloud Drive or a removable volume. A menu-bar app that walks those in the
  background throws up a dialog the operator did not trigger.
- The two lists stay separate (repo ADR 0001 rule 1; reap's ADR 0001 rejected reusing tether's
  list). Each tool wants different clones: tether wants the ones to keep current, reap the ones
  that grow worktrees.
- Shared code moves into a crate named for what it does once both tools use it (repo ADR 0001
  rule 2). Discovery is used by both from the start.

## Decision

### What is suggested

1. **Discovery suggests; it never lists.** A found clone is a *suggestion*. It reaches
   `repos.toml` only through the tool's existing `add`, with every check that `add` already makes
   (top level of a main clone, not a linked worktree, not already listed, and tether's concord
   and remote-branch checks). Nothing is synced or swept because it was found.
2. **The operator answers each suggestion:** *Add* lists it, *Skip* hides it for good. A
   suggestion left unanswered stays a suggestion and costs nothing.
3. **Skips are kept in config.** `skip = ["~/Development/scratch", …]` sits in each tool's
   `repos.toml` next to its `[[repo]]` entries, so it can be edited by hand. *Unskip* (popover and
   CLI) removes an entry. `git <tool> remove` adds the path to `skip`, so a clone that was just
   unlisted is not suggested again a minute later.

### Where it looks

4. **Roots are a list the operator owns.** `scan = ["~/Development", …]` in `repos.toml`. When
   `scan` is absent, the default is those of `~/Development`, `~/Code`, `~/Projects`, `~/src`,
   `~/dev` and `~/work` that exist. `scan = []` turns discovery off.
5. **Protected folders are never a default root.** `~/Desktop`, `~/Documents`, `~/Downloads`,
   `~/Library` (iCloud Drive included) and `/Volumes` are walked only when the operator names one
   in `scan` or picks it in the popover, which is the moment the macOS prompt is expected.
6. **The walk is cheap and bounded.** It goes at most 4 levels below each root, does not follow
   symlinks, and does not enter hidden folders, `node_modules`, `target`, or a folder already
   found to be a clone. A folder is a candidate when its `.git` is a *directory*. A `.git` *file*
   is a linked worktree or a submodule, and is never suggested. The walk itself runs no git. Git
   runs only on the candidates, for the facts in rules 8 and 9. It stops at 20 000 folders or 2
   seconds per root, and the result says it stopped early.

### Which clones each tool suggests

7. **The walk is shared; the filter is per tool.** A `find-clones` crate (`crates/find-clones/`)
   returns main clones under the roots. Each tool drops the ones listed or skipped, then applies
   its own rule:
8. **tether** suggests a clone that has a remote. It is not suggested while concord is installed
   and it is concord's clone (that clone needs a `--gate`, which only `add` can take). The
   suggestion carries the current branch if the remote has it, otherwise the remote's default
   branch. *Add* lists it in `follow` mode with that branch. **A suggestion is never added as
   `mirror`**, because mirror throws away local work on every sync. A mirror still goes through
   *Add repository…*, with rule 8 of tether's ADR 0001.
9. **reap** suggests a clone that has at least one linked worktree (an entry under
   `<git dir>/worktrees/`), with the count, most first. A clone with no worktrees has nothing to
   reap yet. It is suggested once one appears.

### When it runs, and how it shows

10. **A scan runs only when the operator starts one.** That means *Find clones* in the popover, *Look
    again* later, or `git <tool> discover`. Nothing scans on start, on a timer or on popover
    open, and there is no file watcher. The suggestions and the time of the last scan go into the
    tool's `state.json`. Unanswered suggestions stay until they are answered, until the next scan
    no longer finds them, or until they are listed some other way.
11. **The popover leads with the invitation, then the triage.** With nothing listed and no
    scan yet, the empty state offers *Find clones* beside *Add repository* and names the folders
    it will look in. Suggestions show as a *Found* section at the end of the list, each row with
    *Add* and *Skip*. The section stays open while the operator works through it. Once
    something is listed, the next time the popover opens it is one collapsed row with the count
    and the time of the last scan, and that row carries *Look again*. Discovery never sends a macOS
    notification, never changes the tray icon, and never pre-selects a suggestion. The layout is
    settled in each tool's design notes.
12. **CLI.** `git <tool> discover [--json]` scans and prints the suggestions with their facts.
    `git <tool> skip <path>` and `git <tool> unskip <path>` edit `skip`. Accepting is `add`, as it
    is today.
13. **Testability.** The walk takes its roots and home directory as arguments, and the tools pass
    the ones that `TETHER_CONFIG_DIR` / `REAP_CONFIG_DIR` already redirect. Tests build folder
    trees in a temp dir and never walk the real home directory.

This amends tether's ADR 0001 rules 1, 10 and 11, and reap's ADR 0001 rules 1, 14 and 15. Their
status lines name this ADR once it is accepted.

## Considered options

- **List what is found automatically, or list it with an undo.** Rejected in reap's ADR 0001 and
  rejected again here. For reap it means sweeping a clone nobody chose. For tether it means
  fetching repos nobody asked for.
- **Spotlight (`mdfind`).** Spotlight does not reliably index hidden `.git` folders, can be off
  for a volume, and gives no control over depth. A bounded walk of a few roots is fast enough.
- **Scan the whole home directory.** It is slow, it walks `~/Library`, and it sets off every
  privacy prompt on the first run.
- **Scan in the background (on start, hourly on popover open).** New clones would show up without
  anyone asking, but a menu-bar app would walk the disk unprompted, and suggestions would pile up
  that nobody asked for. The owner chose an explicit scan (2026-11-04). The *Found* row shows when
  the last scan ran, so a stale result is visible.
- **An FSEvents watcher for new clones.** It is the background scan with a long-running watcher
  and its edge cases on top.
- **A separate checklist view for the first triage.** It is faster for many clones at once, but
  it is a second place that shows the same suggestions. The owner chose one inline *Found*
  section for the first run and for later scans (2026-11-04).
- **One list of suggestions or skips for both tools.** It couples the two configs that both
  tools' ADRs keep apart. Skipping a clone for tether says nothing about whether to sweep it.
- **Skips in `state.json`.** They would be invisible and lost when state is reset. A skip is the
  operator's decision, so it lives where their other decisions live.
- **A notification per new clone.** It nags. Finding a clone is not urgent, and the tray icon
  stays reserved for states that need the operator.

## Consequences

- Setting up either tool on a new Mac is one scan and a pass through the suggestions, instead of
  a folder picker per clone. A clone made later shows up the next time the operator looks again,
  not before.
- `repos.toml` gains `scan` and `skip`. Both configs reject unknown fields, so a binary older
  than this change cannot read a config that uses them. The installer updates the app and the CLI
  together, which keeps that window small.
- Clones under a protected folder are found only after the operator names that folder.
- A third crate joins the workspace, `find-clones`, as a dependency of both cores. It is
  UI-free and builds and tests on Linux.
- Out of scope: discovering remote repositories (GitHub, other hosts) to clone, bare
  repositories, and sharing roots between the two tools.
