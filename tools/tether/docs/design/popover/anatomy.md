# Tray popover — anatomy

Reference: [`./reference/`](reference/) (frozen 2026-11-01) · Decided: 2026-10-28 (layout),
2026-10-29 (add form, light theme), 2026-11-01 (rounds 3–6: depth and states, forms, edge
cases, stacked callouts) · Rules: [styleguide](../../styleguide.md),
[ADR 0002](../../adr/0002-the-tray-ui-is-a-react-popover.md)

This file replaces the 2026-10-29 anatomy. Everything that file built is in the code at
`3b0c7ee`; this one describes what changes from there. Where a section says *as today*, the
code at that commit is the spec.

| Image | Shows (panels left to right; the caption above each panel names the state and is not UI) |
|---|---|
| `popover-r3.png` | the list: regular (with a hovered row), syncing, all in sync, paused, loading, empty, error, light |
| `add-form-r4.png` | add form: start, ready (follow), mirror, commands, adding, error, light |
| `settings-r4.png` | repo settings: regular, changed, needs you with commands, confirm remove, saving, error, light |
| `edges-list-r5.png` | focus in a callout, focus in a row (tooltip), long names, scrolled, two need you while syncing, action failed, paused with one needing you, read error with a stale report, light |
| `edges-forms-r5.png` | add: long path, branch not read, keyboard on mode; settings: menu open, long name and never synced, removing |
| `callouts-r6.png` | three need you stacked, show all, dismiss (focus + tooltip), one dismissed, all dismissed, reason changed, light |
| `before-r3.jpg` | the popover at `3b0c7ee`, for comparison only (not a target) |

Where two images disagree, the later round wins: `callouts-r6.png` over the single-callout
panels elsewhere (every callout gains the × of round 6, even where an earlier image lacks it).

## 1. The screen

- **Purpose:** show at a glance whether every listed clone is on its branch; put what needs
  the owner in front of them with what to do; stay quiet otherwise.
- **Who:** the owner, clicking the tray icon, for a few seconds.
- **Data:** the `Snapshot` (`app/ui/src/api.ts`): `report.repos` (`Row`s as
  `git tether status --json`), `details` (the CLI's detail line per path), `busy`, `syncing`,
  `note`, `error`. Rows carry `behind`, `last_sync`, `gate`, `after`, `reason`, `hint`.
- **Viewport:** fixed 360 × at most 480, dark and light from the system. No breakpoints.
- **Sparsest:** zero repos; no report yet. **Densest:** ~15 repos with three needing you —
  the body scrolls, header and footer never do.

## 2. Decisions

Round 3 — depth and states (owner, 2026-11-01; directions A/B/C on the canvas Archive):

- **Rows lead with a 16px state glyph**, before the name, in the state colour: check (in
  sync), down arrow into a line (behind), minus in a circle (skipped), triangle (needs you),
  an open arc that spins (syncing), two bars in a circle (paused). The dot is gone from rows.
- **Colour only where something differs.** The state word on the right stays: `in sync` in
  `text-faint`; `behind`, `skipped`, `needs you` in their state colour. A behind row adds
  ` · <behind>` in `text-faint` when `row.behind` is set.
- **The needs-you callout is raised:** `shadow-raised` (new token), and a top strip tinted
  with the needs-you colour holding glyph, name, branch · mode, age.
- **Syncing:** a busy row (requested or in `snapshot.busy`) swaps its glyph for the spinning
  arc, its word for `syncing…` in `text-behind`, takes a `behind`-tinted wash, and shows a 2px
  progress hairline along its bottom. The header shows the same hairline along its bottom edge
  while `snapshot.syncing`. The callout's *Sync now* reads `Syncing…`, disabled, `aria-busy`.
- **Paused:** every row's glyph becomes the pause glyph (`text-faint`); words stay. The header
  word is `paused` with the pause glyph; its right slot keeps `<n> repos`. The footer's
  *Resume* becomes the primary button and *Add repository* goes quiet.
- **Sync all is disabled while `snapshot.syncing`.**
- Rejected: layered header/footer bands tinted by state (B) — the same orange three times;
  a two-line status sentence as the header (C) — a 17px type size outside the scale, and
  sentences the CLI never prints.

Round 4 — add form and settings (owner, 2026-11-01):

- **Add-form preview** is a row: a dashed-circle glyph, name, branch, and `not listed yet` in
  `text-faint` where the state goes.
- **Chosen mode option** gains `shadow-raised` on top of today's accent border.
- **Pending (adding, saving, removing) runs the header hairline** and disables the controls;
  the form no longer fades to 60% (it failed contrast).
- **Errors in a raised callout** above the footer: needs-you-tinted top strip with the
  triangle and a title — `Not added` (add), `Not saved` (save), `Not removed` (remove; not
  drawn, same shape) — and the rejection text verbatim in `text-needs-you` below.
- **Settings header** is the repo's name (truncating) with its state on the right, as a row
  shows it (`behind · 3`).
- **Settings facts** are one-line pairs: *Folder*, *Branch* (`main · follow`), *Last sync*
  (`ago`), and, only when set, *Gate* and *After* in monospace. Read-only.
- **A needs-you repo's settings** open with its reason in a callout (top strip: glyph,
  `needs you`, age; body: the reason). No buttons in it.
- **Remove confirm** is a raised callout without tint (removal is not a state): heading
  `Stop keeping <name> on <branch>?`, body `The clone stays as it is.`, *Remove* and *Keep* as
  inset buttons; removing reads `Removing…` with both disabled.
- **The interval menu** popup gains `shadow-raised` and a check on the selected item.

Round 5 — edge cases (owner, 2026-11-01):

- **Header precedence for the state word:** read error (`needs you`) → paused → needs you
  (`needs you (<n>)`, counting dismissed ones too) → syncing (`syncing…`) → the overall
  health. While a sync runs and something needs you, the word stays `needs you (n)` and only
  the hairline shows the sync.
- **Header right slot:** `<busy> of <total>` while `snapshot.syncing` (e.g. `2 of 7`);
  `last sync <ago>` when every repo is in sync; otherwise `<n> repos`; nothing while loading
  or on the read error.
- **Paused keeps callouts.** A needs-you repo stays a callout while paused; *Sync now* still
  works (an explicit sync runs when paused, `app/src/worker.rs` `run_due_and_requested`).
- **The notice strip** (action failed, `note`, or a read error with a stale report) is a
  needs-you-tinted strip under the header: triangle glyph and the text verbatim, `text-xs
  text-text`. A read error with a report reads `Cannot read repos.toml: <error>. Showing the
  last report.` It clears on the next refresh, as today.
- **Focus:** the focus-token ring on whatever has focus, as today; focus inside a row shows
  its actions and opens its detail tooltip (tooltip now carries `shadow-raised`).
- **Long text:** names and branches truncate on one line (`title` = path); a callout's reason
  always wraps; in settings the title truncates and the state never does; the folder input
  shows the start of the path.
- **Never synced:** ages read `never` (`ago.ts`, as today).

Round 6 — stacked, dismissible callouts (owner, 2026-11-01):

- **Stack.** With two or more callouts showing, only the first (in `sortRows` order) is
  open; one or two slivers peek under it (one per hidden callout, at most two), and under the
  stack a line reads `<n> more need you` (`needs you` for one) with a *Show all* button on the
  right. *Show all* opens every callout in a column; a *Show less* button after the last one
  folds them back. The choice resets to folded when the popover reopens.
- **Dismiss.** Every callout's top strip ends with an × icon button, tooltip and label
  `Dismiss until it changes`. A dismissed repo leaves the callouts and shows as a needs-you
  row at the top of the list (triangle, `needs you`, hover tooltip with the reason); the
  header still counts it. It comes back as a callout when its reason text changes, or when it
  stops needing you and later needs you again.
- **Persistence:** dismissals survive closing the popover and restarting the app
  (`localStorage`, per path, against the reason text). The tray icon is not affected.
- **Open:** none.
- **Added 2026-11-01:** `--font-mono` token (theme.css cleared `--font-*`, so `font-mono`
  rendered sans at main).
- Diverges from the reference: *Add* disabled under a refusal (add-form error board) → *Add*
  stays enabled — the owner can retry once the cause is fixed, confirmed by the owner 2026-11-01.
- Diverges from the reference: a two-line slider glyph for *Settings* → the existing three-slider
  `SettingsIcon` — the inventory reuses it, confirmed by the owner 2026-11-01.
- Diverges from the reference: the mirror board's Clone URL right after Mode, with no *Pull every*
  → today's order (Mode, *Pull every*, Clone URL, *Add commands…*, preview) — the other add boards
  draw it, confirmed by the owner 2026-11-01.
- Diverges from the reference: Gate/After facts drawn whole at a smaller size → `text-sm`,
  truncating, with the full command as `title` — the token map sets `text-sm`, confirmed by the
  owner 2026-11-01.
- Diverges from the reference: Tab landing on the callout's first body button (edges r5) → Tab
  lands on the × first — round 6 put the × in the strip before the body, confirmed by the owner
  2026-11-01.

## 3. Code archaeology

- `app/ui/src/Popover.tsx` `receive` — every report clears `requested`; busy shows from the
  click until the worker reports. Dismissals are reconciled in the same place (see
  `useDismissed` below), so they see every report.
- `Popover.tsx` `isBusy` — `requested ∪ snapshot.busy`; the row's syncing look keys off this.
- `Popover.tsx` `notice` — precedence `actionError ?? note ?? (readError ? null : error)`; the
  stale-report read error gets the `Cannot read repos.toml: … Showing the last report.` wording
  only in the `error` branch.
- `app/ui/src/Header.tsx` `OverallState` — today checks needs-you before syncing, and paused is
  only in the right slot; round 5 reorders this (§2).
- `app/ui/src/repo-list/sortRows.ts` — needs-you first, otherwise `repos.toml` order. The stack
  order and the dismissed rows' order both come from it.
- `app/ui/src/repo-list/NeedsYouCallout.tsx` `CLEARED_BY_SYNC` — decides whether *Sync now* or
  *Open in Finder* leads. Keep it; the boards happen to show the Open-first case.
- `app/src/tray.rs:127` — every refresh is emitted to the window, which is hidden, not closed
  (`app/src/window.rs` `hide`). So the popover sees a repo stop needing you even while closed;
  that is what clears a dismissal when the problem goes away.
- `app/src/state.rs` — `details[path]` is the reason line for a needs-you repo; it is the
  dismissal fingerprint. Never compose one.
- `core/src/inspect.rs` `Reason` Display and `hint` — the reason and hint texts on the boards
  are these strings (`after failed: …`, `tracked changes (2 file(s))`, `HEAD is detached`).
- `app/src/commands.rs` `edited` — a settings save on a repo another writer removed fails with
  `<path> is no longer listed` (the settings error panel).
- `app/ui/src/repo-list/ago.ts` — the only age formatter (`never` for no `last_sync`).
- `app/ui/src/controls/IconButton.tsx` — `aria-label` equals its tooltip; the × follows it.
- Tests that change with this work, not regressions: `RepoRow.test.tsx`, `NeedsYouCallout.test.tsx`,
  `RepoLine.test.tsx`, `EmptyState.test.tsx`, `LoadingRows.test.tsx`, `Popover.test.tsx`,
  `AddForm.test.tsx` (the 60% fade, preview without a state word).
- `docs/styleguide.md` § Tokens, Layout, States describe dots, the flat callout and the faded
  form. Docs-first: rewrite them in the same PR.

## 4. Component inventory

Fidelity is `as-drawn` unless §2 says otherwise.

| Element on artboard | Component | Status | File | Images |
|---|---|---|---|---|
| Shadow and progress tokens | `theme.css` | extend (`--shadow-raised`, `--animate-progress`) | `src/theme.css` | all |
| State glyphs, dismiss ×, check, pending circle | icons | extend (9 icons) | `src/controls/icons.tsx` | all (Settings glyph: confirmed divergence, §2) |
| Glyph + word | `StateLabel` | extend (glyph, quiet in-sync, behind count, syncing, paused) | `src/StateLabel.tsx` | all |
| Progress hairline | `ProgressLine` | new | `src/ProgressLine.tsx` | r3, r4, r5 |
| Raised callout shell | `Callout` | new (tones `needs-you`, `plain`) | `src/Callout.tsx` | r3–r6 |
| Needs-you callout | `NeedsYouCallout` | extend (on `Callout`, busy label, × dismiss) | `src/repo-list/NeedsYouCallout.tsx` | r3, r5, r6 (Tab on × first: confirmed divergence, §2) |
| Callout stack | `CalloutStack` | new | `src/repo-list/CalloutStack.tsx` | r6 |
| Dismissal memory | `useDismissed` | new (hook) | `src/repo-list/useDismissed.ts` | r6 |
| One-line repo | `RepoLine` | extend (glyph column, word rules, pending preview) | `src/RepoLine.tsx` | all |
| Repo row | `RepoRow` | extend (busy wash + hairline, paused glyph) | `src/repo-list/RepoRow.tsx` | r3, r5, r6 |
| Row / icon tooltips | Base UI `Tooltip` | extend (popup `shadow-raised`) | `RepoRow.tsx`, `IconButton.tsx` | r5 |
| List body | `RepoList` | extend (stack, dismissed rows, paused) | `src/repo-list/RepoList.tsx` | r3, r5, r6 |
| Header | `Header` | extend (precedence, right slot, hairline, settings state) | `src/Header.tsx` | all |
| Notice strip | `NoticeStrip` | new (moved out of `Popover`) | `src/NoticeStrip.tsx` | r5 |
| Footer | `Footer` | extend (Resume primary when paused, Sync all disabled while syncing) | `src/Footer.tsx` | r3 |
| Empty promo | `EmptyState` | extend (ghost rows in glyph shape) | `src/repo-list/EmptyState.tsx` | r3 |
| Skeleton | `LoadingRows` | extend (glyph circle, five rows) | `src/repo-list/LoadingRows.tsx` | r3 |
| Read-error panel | `ReadError` | extend (triangle glyph, monospace error) | `src/ReadError.tsx` | r3 |
| Add form | `AddForm` | extend (preview, pending, error callout) | `src/add-form/AddForm.tsx` | r4, r5 (Add after a refusal, mirror order: confirmed divergences, §2) |
| Mode option | `ModeOption` | extend (checked `shadow-raised`) | `src/add-form/ModeOption.tsx` | r4 |
| Interval menu | `IntervalSelect` | extend (popup shadow, selected check) | `src/interval/IntervalSelect.tsx` | r5 |
| Repo settings | `RepoSettings` | extend (facts, needs-you callout, confirm callout, pending, error) | `src/repo-settings/RepoSettings.tsx` | r4, r5 (Gate/After truncation: confirmed divergence, §2) |
| Window frame | `Popover` | extend (pending → header hairline, dismissals, notice) | `src/Popover.tsx` | all |
| Buttons | `TextButton`, `IconButton` | reuse | `src/controls/` | all |

## 5. Components

Token map shared by every entry (utilities only; no arbitrary values):

| Value on the boards | Token utility |
|---|---|
| glyph 16 / large glyph 28 | `size-4` / `size-7`, `stroke-[1.5]` as the existing icons |
| raised shadow | `shadow-raised` (new) |
| needs-you strip tint, its divider | `bg-needs-you/14`, `border-needs-you/30` |
| notice strip tint | `bg-needs-you/12` |
| busy row wash | `bg-behind/10` |
| hairline track / bar | `h-0.5 bg-behind/25` / `bg-behind` |
| callout radius | `rounded-control` (6px; the boards were corrected from 8) |
| facts label column | `w-18 text-xs font-medium text-muted` |
| state column | `w-22 text-xs` |
| everything else | as the 2026-10-29 anatomy's map (frame, header, footer, controls) |

New tokens in `theme.css` (`@theme`, light; dark under the media query):

- `--shadow-raised`: light `0 1px 2px rgb(0 0 0 / 0.08), 0 6px 16px rgb(0 0 0 / 0.08)`;
  dark `0 1px 2px rgb(0 0 0 / 0.5), 0 6px 16px rgb(0 0 0 / 0.28)`.
- `--animate-progress: progress 1.2s ease-in-out infinite` with
  `@keyframes progress { from { translate: -100% } to { translate: 250% } }` (a 40%-wide bar
  sweeping its track). Under `prefers-reduced-motion: reduce` the bar is static at 40%.

### icons (extend)

`InSyncIcon`, `BehindIcon`, `SkippedIcon`, `NeedsYouIcon`, `SyncingIcon`, `PausedIcon`,
`PendingIcon` (dashed circle), `DismissIcon` (×), `CheckIcon`, all on the existing `base`
(16, stroke 1.5, round). Paths, as drawn: check-circle
`M5.4 8.2l1.8 1.8 3.4-3.7` in `r 6.25`; behind `M8 2.75v7.5 M4.75 7.25L8 10.5l3.25-3.25
M3.5 13.25h9`; skipped `M5.25 8h5.5` in the circle; needs-you `M8 2.25l6.25 11H1.75z M8 6.5v3
M8 11.4v.1`; syncing arc `M13.25 8A5.25 5.25 0 1 1 8 2.75`; paused bars `M6.5 5.75v4.5
M9.5 5.75v4.5` in the circle; pending circle `stroke-dasharray 2 2.4`; × `M4.5 4.5l7 7
M11.5 4.5l-7 7`; check `M4 8.5l2.5 2.5 5.5-6`. Test: each renders `aria-hidden`.

### StateLabel (extend)

- Props: `health`, optional `behind?: number`, `busy?: boolean`, `paused?: boolean`,
  `label?` (header overrides), `tone?: "row" | "header"`.
- Row tone: glyph goes in `RepoLine`'s leading slot, not here (see RepoLine); the label is the
  word: `in sync` `text-faint`; `behind` / `skipped` / `needs you` `text-<state>`; behind adds
  ` · N` `text-faint`; busy → `syncing…` `text-behind`.
- Header tone: glyph + word, `gap-2`; word colour `text-needs-you` for needs you, `text-behind`
  for syncing, `text-muted` otherwise; paused → `PausedIcon` `text-faint` + `paused`.
- Export a `GLYPHS` map (`Health | "syncing" | "paused" → icon`) used by RepoLine and callouts.
- Tests: word and colour class per state; behind count only when given; busy and paused.

### ProgressLine (new)

- `span aria-hidden` absolutely placed along the bottom of its positioned parent: track
  `h-0.5 bg-behind/25 overflow-hidden rounded-full`, bar `w-2/5 h-full bg-behind rounded-full
  animate-progress motion-reduce:animate-none`. Props: `inset?: boolean` (`inset-x-2` for rows,
  full width for the header, which sits on its `border-b`).
- Tests: renders hidden from the a11y tree; the header and row use it.

### Callout (new)

- Shell: `section rounded-control border border-border bg-surface-raised shadow-raised
  overflow-hidden`. Slots: `heading` (top strip `flex items-center gap-2 px-3 py-2 border-b`)
  and `children` (body `flex flex-col gap-2 p-3`). Tone `needs-you`: strip `bg-needs-you/14
  border-needs-you/30`; tone `plain`: strip `bg-surface-raised border-border`.
- Pass-through `aria-label`, `role` (`alert` for error callouts).
- Tests: tone classes; role and label pass through.

### NeedsYouCallout (extend)

- On `Callout tone="needs-you"`. Strip: `NeedsYouIcon text-needs-you`, name `font-medium
  truncate` (`title` = path), `branch · mode` `text-muted truncate`, age `ml-auto text-xs
  text-muted shrink-0`, then `IconButton` *Dismiss until it changes* (`DismissIcon`).
  Body: reason `text-needs-you`, hint `text-text`, buttons as today (order by
  `CLEARED_BY_SYNC`), *Settings* icon `ml-auto`.
- Busy: *Sync now* reads `Syncing…`, disabled, `aria-busy`.
- New prop `onDismiss(path)`.
- Tests: × calls `onDismiss`; busy label; strip shows age; existing order tests kept.

### CalloutStack (new)

- Props: `rows` (the undismissed needs-you rows, in order), the callout callbacks.
- One row: just the callout. Two or more, folded: first callout (`relative z-2`), then
  `min(n-1, 2)` slivers `absolute h-3 rounded-b-control border border-t-0 border-border
  bg-surface-raised shadow-raised` at `inset-x-1.5 bottom-1.5` and `inset-x-3 bottom-0`, the
  stack wrapper `relative pb-3`; under it a `flex h-6 items-center px-1` line:
  `<n> more need you` (`needs you` when 1) `text-xs text-muted`, and a text button *Show all*
  `ml-auto h-6 px-2 text-xs font-medium rounded-control hover:bg-surface-hover`
  (`aria-expanded=false`). Unfolded: all callouts `flex flex-col gap-2`, then *Show less*
  right-aligned (`aria-expanded=true`).
- Folded state is local (`useState`), reset when the popover remounts the list.
- A11y: slivers `aria-hidden`; *Show all* moves focus to the second callout's first button.
- Tests: one / two / three rows; Show all / Show less; the count wording.

### useDismissed (new)

- `useDismissed(rows, details) → { isDismissed(path), dismiss(path) }`. Storage key
  `tether.dismissed`: `{ [path]: reasonText }` in `localStorage`, read once, written on change;
  read and write inside `try/catch` (a failure means nothing is dismissed, never a crash).
- On every report: drop entries whose repo is gone, no longer `needs-you`, or whose
  `details[path]` differs from the stored text. `isDismissed` is true only for a needs-you row
  whose stored text equals `details[path]`.
- Tests (fake storage): dismiss hides; changed reason shows again; clearing then recurring
  shows again; corrupt storage is ignored.

### RepoLine (extend)

- Leading glyph slot: `GLYPHS[health]` in `text-<state>`; `busy` → `SyncingIcon animate-spin
  text-behind`; `paused` → `PausedIcon text-faint`. Then name, branch, children, state column
  (`StateLabel` row tone).
- `preview` (replaces `ghost` without health in the add form): `PendingIcon text-faint` and
  `not listed yet` `text-faint` in the state column. Empty-state ghosts keep `ghost` with
  health (`your-app` in sync, `your-docs` behind · 3, as drawn).
- Tests: glyph per state; preview text; mirror suffix kept.

### RepoRow (extend)

- Busy: `bg-behind/10` on the row and `ProgressLine inset`; glyph and word as RepoLine says.
- Paused: passes `paused`. A dismissed needs-you repo is an ordinary row with `health="needs-you"`
  (its tooltip already carries the reason from `details`).
- Tooltip popup adds `shadow-raised` (and so does `IconButton`'s).
- Tests: busy wash and line; paused glyph; needs-you row tooltip text.

### RepoList (extend)

- Callout area: `CalloutStack` of needs-you rows not dismissed. List: dismissed needs-you rows
  first (sortRows order), then the rest. Both in the one `ScrollArea`, as today.
- Passes `paused` from the report; `onDismiss` from `useDismissed`.
- Tests: dismissed repo moves to the list; paused keeps callouts.

### Header (extend)

- State-word precedence and right-slot rules per §2 round 5. Prop `isPending` (or the
  `snapshot.syncing` flag) draws `ProgressLine` on its bottom edge (header `relative`).
- Settings view: title truncates (`min-w-0 truncate`), right side `StateLabel` header tone
  with the repo's health and behind count, `shrink-0`.
- Tests: each precedence case; `2 of 7`; paused right slot; settings title + state.

### NoticeStrip (new)

- `p role="alert" flex items-start gap-2 border-b border-border bg-needs-you/12 px-4 py-2
  text-xs text-text` with `NeedsYouIcon text-needs-you`. Text verbatim; the stale read error is
  wrapped as `Cannot read repos.toml: <error>. Showing the last report.`
- Tests: action error, note, stale read error wording.

### Footer (extend)

- `paused`: *Resume* `variant="primary"`, *Add repository* quiet (still `ml-auto`).
- `isSyncing`: *Sync all* disabled.
- Tests: both.

### EmptyState, LoadingRows, ReadError (extend)

- EmptyState: ghost rows through the new RepoLine (glyphs shown), as drawn.
- LoadingRows: five rows `flex items-center gap-2`, a `size-4 rounded-full bg-surface-raised`
  circle, a name bar `h-3` of `w-40`, `w-33`, `w-46`, `w-29`, `w-37`, a state bar `w-14
  ml-auto`; container `p-4 gap-5`. Still no motion.
- ReadError: `NeedsYouIcon size-7 text-needs-you` replaces the dot; the error line
  `font-mono text-xs text-muted`. Header shows triangle + `needs you`.

### AddForm, ModeOption, IntervalSelect (extend)

- Preview: `RepoLine preview` in the dashed box. Pending: disable controls, no opacity;
  report pending up (`onPendingChange`) so `Popover` draws the header hairline; primary reads
  `Adding…`. Error: `Callout tone="needs-you" role="alert"` with heading `NeedsYouIcon` +
  `Not added`, body the text `text-needs-you`, `mt-auto` above the footer.
- ModeOption checked: add `shadow-raised`.
- IntervalSelect: popup `shadow-raised`; `Select.ItemIndicator` with `CheckIcon text-muted`
  at the right of the selected item.
- Gate/After inputs stay `font-mono text-sm` (the boards were corrected from 12px).
- Tests: preview text; no fade while pending; error callout title and text.

### RepoSettings (extend)

- Facts: `dl` of `flex h-7 items-center gap-2` pairs (`dt` per token map, `dd text-sm
  truncate`, path `title`). *Gate*/*After* only when set, `font-mono`.
- Needs-you repo: a `Callout tone="needs-you"` first: strip `NeedsYouIcon`, `needs you`, age;
  body the reason. Needs `detail` and `now` props.
- Remove: *Remove from tether…* + note as today; confirm becomes `Callout tone="plain"` per §2;
  removing → `Removing…`, *Keep* disabled. Error callouts `Not saved` / `Not removed`.
- Pending: no opacity; `onPendingChange` for the header hairline.
- Tests: facts with and without commands; needs-you callout; confirm callout; error titles.

## 6. Composition

Column: Header → NoticeStrip (if any) → body → footer. Body is one of LoadingRows, ReadError,
EmptyState, RepoList (CalloutStack + rows in one ScrollArea), AddForm, RepoSettings. Only the
body scrolls; forms scroll their own body when they overflow (the long-path add board does).
Fixed width, so nothing between breakpoints.

## 7. Out of scope

The tray icon and its badge (unchanged by dismiss), the native right-click menu,
notifications, an undo for a dismissal, editing *Gate*/*After* in settings, and drag or
keyboard reordering of repos.

## Build order

One branch, one PR: `docs/styleguide.md` (tokens, glyph rules, callouts, stack and dismiss,
pending, notice) → `theme.css` tokens → icons → `StateLabel`, `ProgressLine`, `Callout` →
`RepoLine`, `NeedsYouCallout`, `useDismissed`, `CalloutStack`, `RepoRow` → `RepoList`,
`Header`, `NoticeStrip`, `Footer`, `EmptyState`, `LoadingRows`, `ReadError` → `ModeOption`,
`IntervalSelect`, `AddForm`, `RepoSettings` → `Popover` → tests. Acceptance: the dev server at
360 wide, dark and light, every panel of every image above, against `reference/`.
