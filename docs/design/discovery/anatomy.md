# Clone discovery — anatomy

Reference: [`reference/discovery.dc.html`](reference/discovery.dc.html) (frozen 2026-11-04; the
artboard's own source, drawn in the tray-popover tokens at 360 × 480, panels 1–11) · Spec:
[ADR 0004](../../adr/0004-each-tool-suggests-the-clones-it-finds.md) · Decided: 2026-11-04

The reference is the design canvas's `.dc.html` file, not a screenshot. Every colour in it is a
token value from `packages/tray-popover/src/theme.css` or a tool's `theme.css`, written out in
`renderVals()` (`L` light, `D` dark), and the layout is plain flex markup with the token sizes.
It renders only inside the canvas runtime (`support.js`), so read it as markup. Panel numbers
below refer to the captions in the file (`1 · Live: …` to `11 · reap, Found: …`).

## 1. The screen

- **Purpose.** Let the operator find the clones on this Mac and add or skip each one, instead
  of picking folders one at a time. Discovery only proposes; nothing is listed until *Add*.
- **Who.** A developer setting up tether or reap, or coming back to pick up new clones.
- **Data.** A scan of the roots in `repos.toml` `scan` (default: whichever of `~/Development`,
  `~/Code`, `~/Projects`, `~/src`, `~/dev`, `~/work` exist). Each suggestion has a path, a name
  (folder name), the root it was found under, and a tool fact. For tether that fact is the
  branch it would follow. For reap it is the number of linked worktrees. Each scan also
  records what every root yielded (clone count, stopped early, or an error) and when it ran.
- **Entry points.**
  - The empty state's *Find clones* (no repo listed, no scan yet).
  - *Look again* on the collapsed Found row, and in *Where to look*.
  - *Where to look…* from the empty state, the Found heading and the nothing-found state.
  - `git <tool> discover [--json]`, `skip <path>`, `unskip <path>`.
- **Viewport.** The popover, fixed 360 px wide, at most 480 px tall, dark and light.
- **Sparsest real data.** Six cases:
  - no root exists (`scan` resolves to nothing);
  - zero clones found;
  - one clone found;
  - every suggestion skipped;
  - a root unreadable while the others succeed;
  - a root stopped early at the folder cap.

## 2. Decisions

- **Direction B, an inline *Found* section, chosen 2026-11-04.**
  - Suggestions are rows at the end of the list, with *Add* and *✕ Skip*.
  - While nothing is listed, the Found section is the whole body.
  - The section stays open while the operator works through it. On the next popover open after
    something is listed, it is one collapsed row: count, time of the last scan, *Look again*.
- **A scan runs only when the operator starts one** (owner, 2026-11-04; ADR rule 10). The first
  run shows the invitation (panel 1 idle state); nothing scans on start, on a timer or on open.
- **Scan defaults stay as the ADR says** (owner, 2026-11-04): depth 4, 20 000 folders or 2 s
  per root, and the six default roots.
- **"Where to look…" in the invitation is an underlined text button** (owner, 2026-11-04). It
  is built as a new `link` variant of `TextButton`, as drawn.
- **A failed *Add* reports in the existing `NoticeStrip`, and the row stays** (owner,
  2026-11-04). The text is `add`'s own error, verbatim. No inline row error.
- **One PR, stacked on the ADR PR (#7)** (owner, 2026-11-04).
- **Results stream per root.** The scan walks one root at a time and publishes after each, so
  panel 2 shows partial results with "looking in <root>…". The operator can *Add* or *Skip* a
  row before the scan ends.
- **The scan is local.** Facts come from `.git` and remote-tracking refs only; no fetch, no
  `ls-remote`. tether's branch is the current branch if `refs/remotes/<remote>/<branch>`
  exists, otherwise the target of `refs/remotes/<remote>/HEAD`. A clone with neither is not
  suggested. `add` still runs `ls-remote` on *Add*, as today (ADR rule 8).
- **Scan data lives in `discovery.json`** beside `state.json`, never inside it (see §3).
- **Look again replaces the suggestion list.** The new list is the new scan's result minus
  listed and skipped paths, so a clone that disappeared also leaves the list.
- **Unskip** takes the path out of `skip`. If the last scan found it, it shows in Found again
  at once. Otherwise it shows after the next scan.
- **Removing a root** in *Where to look* edits `scan` and drops that root's suggestions from
  `discovery.json` at once. Adding a root runs nothing until *Look again*.
- **`scan` absent vs written.** While `scan` is absent the defaults apply. The first edit in
  *Where to look* writes the resolved list explicitly.
- Rejected, with reasons in ADR 0004:
  - Direction A, a checklist view;
  - Direction C, one prompt per clone;
  - the A+B mix;
  - background or hourly scans;
  - notifications;
  - pre-selected suggestions.
- **Diverges from the reference:** none. Everything below is `as-drawn`.
- **Open:** none.

## 3. Code archaeology

Paths are from the repository root. Line numbers are as of commit `64cb9d6`; re-find a line by
its symbol if it has moved.

- **State file shape is load-bearing.**
  - tether `tools/tether/core/src/record.rs:50` (`Records = BTreeMap<String, Record>`, `load`
    `:53`, `save` `:68` under `state.lock`).
  - reap `tools/reap/core/src/state_file.rs:65` (`StateFile = BTreeMap<String, RepoMemory>`,
    `load` `:70`, `update` `:83` under `state.lock`).
  - `load` returns `default()` on a parse error. A new top-level key silently wipes records,
    clocks and keeps. Discovery writes its own `discovery.json` (atomic write and rename, the
    `write_atomic` pattern at `record.rs:98`), under its own lock or `state.lock`.
- **Config rejects unknown fields.**
  - tether `tools/tether/core/src/config.rs:70` (`Config`, `deny_unknown_fields`);
    reap `tools/reap/core/src/config.rs:87`.
  - Add `scan: Option<Vec<String>>` and `skip: Vec<String>` with `serde(default)` and
    `skip_serializing_if`, so a file with neither key round-trips byte-for-byte.
  - In reap, put them with the global keys before `repo` (`:104`): TOML writes tables last.
  - load/save: tether `:181`/`:195`, reap `:213`/`:229`. Both write tmp + rename and keep a
    header comment.
  - `repos()` (tether `:214`, reap `:260`) fails the whole file on one bad entry. Validate
    `scan`/`skip` entries the same way (expand `~`, reject relative paths).
- **`repos.toml` has no lock.** tether's app serialises writers through `state.work`
  (`tools/tether/app/src/state.rs:20`). reap has no such mutex.
  - New writers (skip, unskip, add root, remove root, remove-adds-skip) follow each tool's
    existing pattern: in tether, take `state.work` as `add`/`remove` do.
  - Do not add a new locking scheme in this PR.
- **Paths.** `Paths::tilde`/`expand` (tether `paths.rs:124`/`:134`, reap `:110`/`:120`).
  `absolute`/`same_path` canonicalise, since macOS has `/private/var`: tether `config.rs:163`,
  `:173`; reap `paths.rs:132`, `:142`. Compare scan paths to listed and skipped paths with
  `same_path`, never string equality. reap's `add` (`tools/reap/core/src/add.rs:17`) does not
  expand `~`; its callers do.
- **add.**
  - tether `tools/tether/core/src/add.rs:32` runs these checks, in order: listed, gate/after,
    concord's clone needs a gate (`:42`), interval, then main clone (`inspect::git_dir`), then
    `ls_remote` for the branch (`:85`). Accepting a suggestion calls it with `mode = follow`,
    the suggestion's branch, and every optional field unset.
  - reap `add.rs:17`: folder, top level, not a linked worktree, not listed.
  - tether `current_branch` is at `add.rs:207` (`git.rs:113`, `symbolic-ref --short HEAD`).
    tether has no default-branch logic. The `<remote>/HEAD` read mirrors reap's
    `report.rs:378` `base_ref` (`symbolic-ref refs/remotes/<remote>/HEAD`).
- **remove.** tether `add.rs:176` also calls `record::forget`. reap `add.rs:66` does not touch
  state. Both now also append the path to `skip` (ADR rule 3). reap has no Tauri `remove`
  command, so only its CLI path changes there.
- **CLI.** clap derive.
  - tether: `cli.rs:50` `enum Cmd`, dispatch `:151`.
  - reap: `cli.rs:36` `enum Command`, dispatch `:123`.
  - `--json` uses `serde_json::to_string_pretty` (tether `:156`, reap `:128`).
  - Unknown targets exit 2 (`UsageError`). Parse tests are at tether `cli.rs:288+` and reap
    `cli.rs:270+`.
  - `discover`, `skip` and `unskip` join both enums.
- **Tauri.**
  - Commands live in `app/src/commands.rs`, with git work inside `blocking()` (`:23`).
  - They are registered in `tray.rs` `invoke_handler`: tether `:40-52`, reap `:38-50`.
  - Each command ends by sending `Event::Refresh`. Worker events coalesce for 300 ms
    (`worker.rs:30` tether, `:26` reap), and each round ends in one `tray::publish` of
    `"report"`.
  - A scan publishes partial results by sending `Refresh` after each root, never by emitting
    its own event.
  - `Snapshot`: tether `state.rs:36`, reap `state.rs:31`. It gains a `discovery` field.
  - The folder picker is `osascript.rs:30` `choose_folder`, used through `pick_folder`
    (tether commands `:55`, reap `:49`). It sets `picking` so focus loss does not hide the
    popover. *Add folder…* reuses it.
- **A scan takes no per-clone lock** (`tether.lock`/`reap.lock`/`index.lock`). It only reads.
- **UI.**
  - API and test layers: `api.ts` (tether `TetherApi` `:67`, reap `ReapApi` `:103`, one
    snake_case `invoke` per method), `api.fake.ts` (`createFakeApi`), `api.scenarios.ts`
    (`SCENARIOS`; reap's `api.scenarios.test.tsx` renders every key).
  - Views: tether `Popover.tsx:21` `View` union, plus `show()` at `:39`. reap
    `Popover.tsx:30` uses a bare `"list" | "add"` string; turn it into a union for
    `{ kind: "roots" }`.
  - Lists: `RepoList.tsx:41` and `WorktreeList.tsx:83` return `EmptyState` when there are no
    rows. Both now go through the discovery states below.
  - Errors: tether's `run()` sets `actionError`, and `noticeOf.ts` puts it first. reap's
    `run()` (`:70`) returns a boolean and has its notice inline at `:101`. A failed *Add* uses
    exactly these.
  - Footers hide everything but *Pause* when no repo is listed.
- **Shared package.**
  - `TextButton`: variants `primary | quiet | inset`, sizes `default | compact`.
  - `IconButton {label, onClick, busy?}`: the label is the tooltip and the aria-label.
  - Icons: `DashedCircleIcon`, `CrossIcon`, `ChevronRightIcon`, `SpinnerIcon`, `WarningIcon`,
    `FolderIcon` all exist.
  - ADR 0003 rule 3: the package knows no tool's words, so every string comes in as a prop.
- **Not reusable as-is.**
  - tether `RepoLine` `preview` (`RepoLine.tsx:21`, `:45`) is `aria-hidden` and says "not
    listed yet".
  - reap's `OutcomeSection` (`OutcomeSection.tsx:9`) is typed to reap's `Outcome` glyphs and
    returns null at count 0.
  - Both are references for markup and tokens, not dependencies. `OutcomeSection` shows how
    the collapsible heading is built (`@base-ui/react/collapsible`, `:80`); `FoundSection`
    copies its classes.
- **Docs that quote the UI.**
  - tether's styleguide quotes the empty-state copy verbatim
    (`tools/tether/docs/styleguide.md:125`). Update it with the new copy in the same PR.
  - reap's styleguide § Outcomes (`tools/reap/docs/styleguide.md:36`) governs outcome
    sections. *Found* is not an outcome section, so add one line there saying so.
- **Workspace and CI.**
  - New crate `crates/find-clones/`: add it to root `Cargo.toml` `members` and
    `default-members` (it is UI-free). Inherit `[workspace.package]`, set
    `[lints] workspace = true`.
  - `.github/workflows/ci.yml` names crates in `clippy`/`test -p …`; add `-p find-clones`.
    `ci.yml` is an owner path (`.github/owner-paths`), so the PR needs the owner's approval.
- **Tests.**
  - Rust integration tests are `tools/<tool>/core/tests/*.rs` with `mod sandbox;`.
    - tether `tests/sandbox/mod.rs:88`: bare remote, clones, fake `$HOME` at `root/home`;
      `cli()` (`:152`) sets `HOME`, `TETHER_CONFIG_DIR`, `TETHER_GIT`.
    - reap `sandbox/mod.rs:52`: home is the temp root, gh is faked.
  - UI tests: Vitest + jsdom + Testing Library.
  - Storybook titles: tether `Popover/List`, `Popover/Add form`, `Popover/Settings`,
    `Popover/Edge cases`; reap `Popover/States`. `popoverSteps.ts` finds buttons by exact
    name.

## 4. Component inventory

`pkg` = `@steading/tray-popover/<File>` (`packages/tray-popover/src/`). Fidelity is `as-drawn`
throughout.

| Element on artboard | Component | Status | Import | Fidelity | Panels |
|---|---|---|---|---|---|
| Frame, header band, footer band | `PopoverFrame`, `PopoverHeader`, `PopoverFooter` | reuse | pkg | as-drawn | all |
| Header aside "N found" while nothing is listed | tool `Header` (`summaryOf`) | extend | app `Header.tsx` | as-drawn | 1, 2, 3, 8, 11 |
| Header progress hairline while scanning | `ProgressLine` via `isPending` | reuse | pkg | as-drawn | 2 |
| Invitation empty state (ghost rows, copy, *Add repository* + *Find clones*, "Looks in …" + *Where to look…*) | tool `EmptyState` | extend | app `EmptyState.tsx` | as-drawn | 1 idle, 10 |
| Nothing-found empty state | tool `EmptyState` (`found-nothing` variant) | extend | app `EmptyState.tsx` | as-drawn | 7 |
| Underlined text button "Where to look…" | `TextButton` `variant="link"` | extend | pkg `controls/TextButton` | as-drawn | 1, 10 |
| Found section: heading + count + right slot (action or scanning status) + lead + rows | `FoundSection` | new | pkg `FoundSection` | as-drawn | 1, 2, 3, 4, 8, 11 |
| Found row: dashed glyph, name, detail, *Add*, ✕ | `FoundRow` | new | pkg `FoundSection` (same file) | as-drawn | 1, 2, 3, 4, 8, 11 |
| Ghost rows while scanning | `FoundRow` `ghost` | new | pkg | as-drawn | 1 looking, 2 |
| Root error line (warning glyph + text) | `FoundSection` `errors` | new | pkg | as-drawn | 8 |
| Collapsed Found row (chevron, Found, count, "· looked …", *Look again*) | `FoundSection` `collapsed` | new | pkg | as-drawn | 5 |
| Settled row (dashed glyph, "No suggestions · looked …" / "Nothing new in … · looked …", *Look again*) | `FoundSection` `settled` | new | pkg | as-drawn | 1 settled, 6 |
| Where to look view: Folders (folder glyph, path, "N clones", ✕), *Add folder…*, protection note, Skipped (name, root, *Unskip*), footer *Done* + *Look again* | `WhereToLook` | new | pkg `WhereToLook` | as-drawn | 9 |
| Compact *Add* / *Look again* / *Where to look…* / *Add folder…* / *Unskip* | `TextButton` `size="compact"` (`inset` for row *Add*/*Unskip*, `quiet` elsewhere) | reuse | pkg | as-drawn | all |
| ✕ Skip / ✕ Stop looking | `IconButton` + `CrossIcon` | reuse | pkg | as-drawn | 1–4, 8, 9, 11 |
| Listed rows above Found | tether `RepoRow` / reap worktree list | reuse | app | as-drawn | 4, 5, 6 |
| Add error | `NoticeStrip` via each tool's existing error flow | reuse | pkg | as-drawn (decided §2) | — |

## 5. Components

### `TextButton` — `link` variant (extend)

- **For:** a low-key action inside a sentence ("Looks in ~/Development and ~/Code. *Where to
  look…*").
- **API:** `variant="link"` joins `primary | quiet | inset`. Use it only with
  `size="compact"`.
- **Token map:** no border; `bg-transparent`; `text-text`; `text-xs`; `underline`; `px-2`;
  `h-6`; `rounded-control`; focus `outline-focus`, as the other variants. Hover: no
  background, keep the underline.
- **States:** default, hover, focus-visible, disabled (`opacity-50`).
- **A11y:** a real button; its name is its text.
- **Tests:** `packages/tray-popover/src/controls/TextButton.test.tsx` checks that the variant
  renders a button with the link classes, and that the existing variants are unchanged.

### `FoundSection` + `FoundRow` (new, `packages/tray-popover/src/FoundSection.tsx`)

- **For:** the suggestions a scan found, in either tool. It is tool-agnostic: every string
  and the row detail come from the caller (ADR 0003 rule 3).
- **API (sketch; the names are the implementer's, the slots are not):**
  - `mode`: `"open" | "collapsed" | "settled"`.
  - `title` ("Found"), `count`, `lead?` (string).
  - `aside?`: right slot of the heading. Either an action node (*Where to look…* compact
    quiet) or a scanning status (`SpinnerIcon` + "looking in ~/Code…", `text-xs`,
    `text-progress`).
  - `errors?: string[]` (each one a warning line).
  - `rows`: children `FoundRow`s.
  - `ghostRows?: number` (while scanning).
  - `collapsedSummary` ("· looked 3 days ago"), `settledText` ("No suggestions · looked just
    now").
  - `onLookAgain`, `lookAgainLabel`.
  - `hasRuleAbove` (top border when listed rows sit above it).
- **`FoundRow` API:** `name`, `detail` (tether "main · ~/Development", reap "7 worktrees ·
  ~/Development"), `title` (the full path, a tooltip on name and detail), `onAdd`,
  `addLabel`, `onSkip`, `skipLabel` (aria-label and tooltip "Skip <name>"), `busy?` (an *Add*
  in flight: *Add* disabled, row stays).
- **States:**
  - open with rows (3);
  - open while scanning, partial rows plus ghosts (2, 1 looking);
  - open with an error line (8);
  - collapsed (5);
  - settled, none left (1 settled, 6);
  - row add in flight;
  - a failed add (row stays; the notice is the tool's).
- **Token map:**
  - Heading: `flex items-center gap-2 px-4 pt-3 pb-1`. `h2` is `text-sm font-semibold
    text-text`; the count is `font-normal text-faint`.
  - Lead: `px-4 pb-1 text-xs text-muted`.
  - Rows list: `px-2 pb-1`. Row: `flex h-8 items-center gap-2 px-2 rounded-control`.
  - Row content:
    - glyph: `DashedCircleIcon text-faint`;
    - name: `font-medium truncate`;
    - detail: `flex-1 truncate text-faint`;
    - *Add*: `TextButton size=compact variant=inset`;
    - ✕: `IconButton` + `CrossIcon`.
  - Ghost row: `opacity-50 aria-hidden`, glyph plus a `h-2.5 rounded-full bg-border` bar of
    varying width (`w-24`, `w-16`, `w-30`).
  - Error line: `flex gap-2 px-4 pb-1 text-xs text-text`, `WarningIcon text-needs-you`.
  - Collapsed and settled rows: `flex min-h-8 items-center gap-2 px-4 py-1.5 border-t
    border-border`.
    - Collapsed: a chevron toggle button (`ChevronRightIcon text-muted`, rotated 90° when
      open), "Found" `font-semibold`, count `text-faint`, summary `text-xs text-faint`.
    - Settled: `DashedCircleIcon text-faint`, text `flex-1 text-xs text-faint`.
    - *Look again*: `TextButton size=compact variant=quiet`.
  - Top rule when listed rows sit above: `border-t border-border`.
- **A11y:**
  - The section is a `section` labelled by its `h2`. Rows are a `ul`/`li`.
  - The collapsed toggle is a button with `aria-expanded` (Base UI Collapsible, as
    `OutcomeSection`). *Look again* is a sibling button, never nested in the toggle.
  - The scanning status sits in a polite live region.
  - Focus order: *Add*, then ✕, per row, top to bottom.
  - Ghost rows are `aria-hidden`.
- **Content:** name and detail truncate with the full path as `title`. Counts are plain
  numbers. Times come from each tool's `ago.ts` ("looked 3 days ago", "looked just now").
- **Motion:** `SpinnerIcon animate-spin` and `ProgressLine`, as existing. Reduced motion
  follows what those already do.
- **Tests (Vitest + Testing Library, package):**
  - each mode renders its parts;
  - *Add*/✕ call back with the row;
  - the collapsed toggle shows and hides rows;
  - *Look again* fires without toggling;
  - ghost rows are hidden from the a11y tree;
  - long names truncate and carry `title`.

### `WhereToLook` (new, `packages/tray-popover/src/WhereToLook.tsx`)

- **For:** the folders a scan walks, and the clones the operator skipped.
- **API:**
  - `roots: {path, found?: string, error?: string}[]`, with `onRemoveRoot`, `onAddRoot`.
  - `skipped: {path, name, root}[]`, with `onUnskip`.
  - `note` (the protected-folders sentence).
  - `onDone`, `onLookAgain`.
  - All labels as props.
  - The tool renders it as the body of a `{kind: "roots"}` view, with the header title
    "Where to look" and `showState: false` (as the add view does).
- **Layout:**
  1. "Folders" `h2` with *Add folder…* (compact quiet) right.
  2. Root rows: `FolderIcon text-muted`, path `flex-1 font-medium`, "N clones"
     `text-xs text-faint`, ✕ `IconButton` "Stop looking in <path>".
  3. The note (`px-4 pb-2 text-xs text-muted`).
  4. A `border-t` and "Skipped" `h2` with its count.
  5. Skipped rows: name `font-medium`, root `flex-1 text-faint`, *Unskip* (compact inset).
  6. Footer: *Done* (quiet) and *Look again* (primary, `ml-auto`).
- **States:** roots and skipped both present (9); no skipped (the section is hidden); a root
  with an error (its `found` slot shows the error in `text-needs-you`); the last root removed
  (Folders shows only *Add folder…*, and *Look again* is disabled).
- **A11y:** two labelled lists; focus starts on the first control.
- **Tests:** add, remove, unskip and done callbacks; hiding of an empty Skipped; disabled
  *Look again* with no roots.

### tool `EmptyState` (extend, both apps)

- **Variants:**
  - `invite`: before any scan.
    - Ghost box: two `FoundRow ghost`-style lines that show a name, a detail and an *Add*
      pill. These are not the scanning ghosts.
    - Copy:
      - tether: "tether keeps each clone you list on its branch. Find the clones on this Mac,
        or add one by hand."
      - reap: "reap removes the worktrees whose work has merged. Find the clones on this Mac
        that have worktrees, or add one by hand."
    - Buttons: *Add repository* (quiet) and *Find clones* (primary).
    - Then `text-xs text-faint` "Looks in ~/Development and ~/Code." with *Where to look…*
      (`link`).
  - `found-nothing`: after a scan with zero suggestions and nothing listed.
    - Copy: "No clones in <roots>. Look somewhere else, or add one by hand."
    - Buttons: *Where to look…* (quiet) and *Add repository* (primary).
    - Then `text-xs text-faint` "Looked just now · stopped early in ~/Code after 20 000
      folders." (the stopped-early part only when it applies).
- **Roots in copy:**
  - one root: "~/Development";
  - two: "~/Development and ~/Code";
  - three or more: "~/Development, ~/Code and 2 more";
  - none resolve: "Looks nowhere yet." with *Where to look…*.
- **Tests:** both variants and both tools' copy; the buttons call the right handlers.

## 6. Composition

- **The body, in order.**
  - No repo listed and no scan yet: `EmptyState invite`.
  - No repo listed, scanning or with suggestions: `FoundSection open` fills the scroll area.
    The header aside reads "N found". The footer shows *Pause* and *Add repository* (quiet,
    `ml-auto`), as panel 3 draws it.
  - No repo listed, a scan done and nothing found: `EmptyState found-nothing`.
  - Repos listed: the existing list. Under it:
    - `FoundSection open` (with the top rule) while this popover session has acted on
      suggestions or a scan is running;
    - otherwise `FoundSection collapsed` when suggestions remain;
    - `settled` when a scan has run and none remain;
    - nothing at all before the first scan.
  - The footer is unchanged when repos are listed.
- **Live flow (panel 1):** *Find clones* → the header hairline and a Found section filling in
  per root → rows with *Add*/✕. Each *Add* moves the clone into the list as the tool's normal
  row (tether "syncing…"). When the last suggestion is answered the section becomes the
  settled row.
- **Scroll:** listed rows and Found share one scroll area between header and footer. The
  header and footer never scroll.
- **In reap,** Found goes after the outcome sections and after `SalvagedSection`/`RepoNotes`,
  as the last thing in the scroll area.
- **Where to look** replaces the body like the add form. Leaving it returns to the list.
- **No breakpoints:** the width is fixed at 360. Long names and paths truncate; nothing wraps
  except the lead and note text.

## 7. Out of scope

- What *Add repository* (the add form) does now that discovery exists. It is unchanged.
- Keyboard shortcuts beyond tab order.
- Auto-expanding Found when the popover opens with new suggestions.
- Discovering remote repositories to clone; bare repositories; sharing roots or skips between
  the two tools.
- tether `mirror` from a suggestion. A mirror still goes through the add form.
- Visual-regression baselines. The project has no visual harness yet (ADR 0005 § 5); stories
  are the reference.
