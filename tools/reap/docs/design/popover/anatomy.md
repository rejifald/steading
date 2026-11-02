# Popover: outcome inbox, anatomy

Reference: `./reference/A-outcome.png`, `A-dark.png`, `A-tidy.png` (frozen 2026-10-29; repository
names made up). Decided by the owner on 2026-10-29.

## 1. The screen

The menu-bar popover (360px wide, at most 480px tall, light and dark) answers two questions
first: what reap will remove, and what needs the owner. It renders the `Report` from
`app/ui/src/api.ts` (made by `core/src/report.rs`); the contract does not change.

Real data it must survive: 37 worktrees over 3 repos, one repo with none; one repo elsewhere
has 164. Every repo runs `policy = "ask"`, so `removes_at` is usually absent. Sparsest: one repo,
no worktrees (the existing empty state), or a report with nothing to remove (`A-tidy`).

## 2. Decisions

- **Group by outcome, across repos**, not by repo. Sections, in order: *Needs you*, *Ready to
  remove*, *Merged, but held*, *In progress*, *Kept*. The repo becomes a quiet tag on each row.
  Why: the old list buried the 1 row that needed the owner and the 22 reap would remove among
  ~14 rows that need nothing.
- **Row title is the branch**; the folder name when there is no branch. Claude's generated
  folder names (`happy-villani-365e18`) say nothing; the branch does. The folder name moves to
  the row's accessible description and Open in Finder.
- **A plain sentence replaces the state word.** Line 2 says what happens (rules in §5,
  `sentenceOf`), so the nine CLI state words and the hover-only reason go.
- **Bulk "Remove all N"** in the Ready heading, with an inline confirm. It calls
  `api.removeNow(path)` once per worktree, in order; git's guards still apply to each, and a
  refusal shows up under Needs you as `refused`.
- **Row actions stay icon buttons on hover or focus** (Remove now, Keep/Unkeep, Open in Finder),
  at the right of line 1. Needs-you rows keep visible text buttons, as drawn.
- **Repo errors go to Needs you** as a row (repo name, the error); `gh` notes and repos with no
  linked worktrees go to a faint *Repositories* list at the end of the scroll.
- Rejected: per-repo groups with sentences (B) — what reap removes stays spread across repos;
  summary first with drill-in (C) — every look at a worktree costs a click and a second view.
- Content edge (settled by `docs/styleguide.md`, "the reason in full"): a Needs-you row's
  sentence wraps; every other row's line 2 truncates with an ellipsis.
- Open: none.

## 3. Code archaeology

- `app/ui/src/api.ts` — `State`, `Worktree`, `Repo.policy`, `Worktree.removes_at` (set only under
  `auto` once a grace clock runs). Do not change it.
- `app/ui/src/StateLabel.tsx` — `STATE_TONES` maps a state to a colour token; keep using it for
  dots. `STATE_LABELS` stays for the header's overall state only (`OVERALL_LABELS`).
- `app/ui/src/Popover.tsx` — `ACTIONS` maps a `WorktreeAction` to an api call; `run()` reports a
  failed action in the alert line. The bulk remove goes through the same `onAction`.
- `app/ui/src/worktree-list/ago.ts` — `ago()` returns "3 d ago", "never"; `inTime()` returns
  "in 3 h", "now". Rows show the age without "ago" ("3 d") and nothing when there is no
  `last_activity`.
- `app/ui/src/worktree-list/EmptyState.tsx` uses `WorktreeLine` for its ghost preview; keep it
  working (redraw the ghost with the new row if `WorktreeLine` goes).
- `app/ui/src/add-form/AddForm.tsx` also renders a ghost line; check what it imports before
  deleting anything.
- The fake (`api.fake.ts`) removes a worktree from the list on `removeNow`, so the bulk remove
  is testable end to end.

## 4. Component inventory

| Element | Component | Verdict | Path | Fidelity | Artboards |
|---|---|---|---|---|---|
| Header | `Header` | extend (right slot: `N worktrees · M repos`) | `src/Header.tsx` | as-drawn | all |
| Footer | `Footer` | reuse | `src/Footer.tsx` | as-drawn | all |
| Section heading (dot, title, count, optional action) | `OutcomeSection` | new | `src/worktree-list/OutcomeSection.tsx` | as-drawn | A, dark, tidy |
| Collapsed section line (chevron, dot, title, count, summary) | `OutcomeSection` (`collapsible`) | new | same | as-drawn | A, tidy |
| Section lead sentence | `OutcomeSection` (`lead`) | new | same | as-drawn | A |
| Two-line row | `WorktreeRow` | extend (rewrite) | `src/worktree-list/WorktreeRow.tsx` | as-drawn; actions per §2 | A, dark |
| Needs-you row with text buttons | `WorktreeRow` (`emphasis`) | extend | same | as-drawn; sentence wraps | A |
| Repo error row | `RepoErrorRow` | new | `src/worktree-list/RepoErrorRow.tsx` | §2 decision | — |
| "Show N more" | inside `OutcomeSection` | new | same | as-drawn | A |
| Remove all + confirm | `RemoveAll` | new | `src/worktree-list/RemoveAll.tsx` | as-drawn; confirm per §2 | A |
| Nothing to remove block | `NothingToRemove` | new | `src/worktree-list/NothingToRemove.tsx` | as-drawn | tidy |
| Repositories footnote | `RepoNotes` | new | `src/worktree-list/RepoNotes.tsx` | §2 decision | — |
| Sentence for a row | `sentenceOf()` | new (pure) | `src/worktree-list/sentenceOf.ts` | §5 | all |
| Outcome of a state | `outcomeOf()` | new (pure) | `src/worktree-list/outcomeOf.ts` | §5 | all |
| Dots | `STATE_TONES` / dot classes | reuse | `src/StateLabel.tsx` | as-drawn | all |
| Buttons | `TextButton`, `IconButton`, icons | reuse | `src/controls/` | as-drawn | all |
| Old pieces | `RepoGroup`, `NeedsYouCallout`, `WorktreeLine` | delete when unused | `src/worktree-list/` | — | — |

## 5. Components

### `outcomeOf(state)`

`stale`, `refused` → `needs-you`; `removable`, `missing` → `ready`; `dirty`, `in-use`, `locked` →
`held`; `open` → `in-progress`; `kept` → `kept`. Section order: needs-you, ready, held,
in-progress, kept. Within a section: repo order as reported, then worktree order as reported.

### `sentenceOf(worktree, repo, now)`

Line 2 of a row. Sentence case (first letter upper-cased).

- `removable` with `pr.state === "merged"`: `Merged in #<n>`; otherwise the reason
  (`Merged into origin/main`). Then, when `removes_at` is set, ` · removes <inTime>`
  (`· removes in 3 h`); when `repo.policy === "off"`, ` · auto-removal is off`.
- `missing`: `Folder is already gone. A sweep clears git's record.`
- `stale`: the reason, then ` reap won't remove it.` (`Pull request #35 closed without merging.
  reap won't remove it.`)
- every other state: the reason, sentence-cased (`Merged into origin/main, but a process is
  running inside it`).

Use a typographic apostrophe (’) in copy, as the artboard does.

### `OutcomeSection`

Props: `outcome`, `title`, `count`, `lead?`, `action?` (a node at the right of the heading),
`collapsible` (default closed), `summary?` (right side of a collapsed line), `limit?` (rows shown
before "Show N more"), `children` rows.

- Open section heading: `px-4 pt-3 pb-1`, dot (`size-2 rounded-full`, the outcome's tone), title
  `text-sm font-semibold text-text`, count `font-normal text-faint`, `action` at `ml-auto`.
- Lead: `px-4 pb-1 text-xs text-muted`.
- `limit`: shows the first `limit` rows (Ready: 4), then a 32px line `Show <N> more` in
  `text-muted` that reveals the rest (a Base UI `Button`). Once shown, no "show fewer".
- Collapsible: a 32px line with `border-t border-border px-4`: chevron (rotates 90° open),
  dot, title `font-semibold`, count `text-faint`, summary `ml-auto text-xs text-faint`. Built on
  Base UI `Collapsible`; the trigger is the whole line; expanded, the rows follow it.
- A section with no rows renders nothing.
- a11y: `<section aria-label={title}>`, rows in `<ul aria-label={title}>`; the collapsible
  trigger exposes `aria-expanded`.
- Sections and their props:
  - Needs you: open, no limit.
  - Ready to remove: open, `limit` 4, `action` = `RemoveAll`, `lead` by policy: all repos with
    ready rows `ask` → `Merged and clean. You remove them here; reap waits (policy: ask).`; all
    `auto` → `Merged and clean. reap removes each one when its grace period ends.`; all `off` →
    `Merged and clean. Auto-removal is off; remove them here.`; mixed → `Merged and clean.`
  - Merged, but held: collapsible; summary counts by kind: `8 in use · 4 uncommitted · 1 locked`
    (only kinds present; `dirty` → "uncommitted").
  - In progress: collapsible; summary `not merged yet`.
  - Kept: collapsible; summary `until new commits`.

### `WorktreeRow`

Two lines, `px-4 py-1.5`, `gap-0.5`, `<li>`, hover/focus-within `bg-surface-hover`.

- Line 1: title (`branch ?? name`) `truncate font-medium text-text`, `flex-1 min-w-0`; then the
  actions (icon buttons, hidden until hover or focus-within, as today); then the meta
  `<repo> · <age>` in `shrink-0 text-xs text-faint` (age omitted when unknown: `docs-site`).
- Line 2: `sentenceOf()` in `text-xs text-muted`, `truncate`. With `emphasis` (Needs you): in
  `text-needs-you`, wrapping, and the text buttons below it (`Try again` and `Keep` for refused, `Open in
  Finder`), `pt-1`.
- Actions by state: removable/missing → Remove now, Keep (not for missing), Open in Finder;
  held states → Keep, Open; kept → Unkeep, Open; open → Open. Same `IconButton`s and labels as
  today.
- a11y: `aria-label` = the title; `aria-description` = `<folder name> · <reason> · last
  activity <ago>` (plus ` · removes <inTime>` when set). Icon buttons keep `aria-label`.

### `RemoveAll`

Two parts. `RemoveAllButton` sits in the Ready heading: `TextButton variant="primary"
size="compact"` `Remove all <N>`. Clicked, it hides and `RemoveAllConfirm` takes the lead line's
place under the heading, full width: `Remove <N> worktrees?` (`text-sm text-text`, singular for
1), `Cancel` (quiet) and `Remove` (primary). Remove calls `onAction("remove-now", path)` for each
ready worktree, one at a time, and stops at the first failure so its error stays in the alert
line; then the confirm closes. Focus moves to Cancel when the confirm opens; Escape inside the
confirm cancels it (and must not hide the popover: stop the event).

Built differently from the first sketch (confirm inside the heading line): at 360px the heading,
the question and two buttons do not fit on one line, so the confirm takes the lead line. No
artboard drew the confirm state.

### `RepoErrorRow`

In Needs you: title = repo name, meta empty, line 2 = the error, wrapping in `text-needs-you`,
one text button `Open in Finder` (`repo.path`). Counts toward the section count.

### `NothingToRemove`

Shown above the collapsed sections when there is nothing in Needs you or Ready: `px-4 py-6`,
`Nothing to remove` (`text-base font-semibold`), then `No worktree has merged since the last
sweep. reap checks again on the next sweep.` (`text-sm text-muted`).

### `RepoNotes`

At the end of the scroll, only when there is something to say: heading `Repositories` (`px-4
pt-4 pb-1 text-xs font-semibold text-muted`), then one `text-xs text-faint` line per note:
`<repo> · no linked worktrees`, `<repo> · <gh_note>`.

### `Header`

Right slot: `<N> worktrees · <M> repos` (singular forms for 1); `Paused` when paused, as today.

## 6. Composition

Header, then the scroll area (`ScrollArea`, as today) holding, in order: Needs you, Ready to
remove (or `NothingToRemove` when both are empty), Merged but held, In progress, Kept,
Repositories; then the footer. No repo grouping anywhere. One viewport, so nothing between
breakpoints; long titles truncate, meta never shrinks. The empty-repos state, loading rows, read
error and add form are unchanged.

## 7. Out of scope

Tray icon and its menu; the add form; the empty (no repos), loading and read-error states beyond
keeping them working; search or filtering for very large repos (164 worktrees renders as long
sections); visual-regression baselines (no harness exists; adding Playwright is its own change).
