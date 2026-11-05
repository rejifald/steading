# Styleguide

How tether's UI looks and is built ([ADR 0002](adr/0002-the-tray-ui-is-a-react-popover.md)).
It is a small macOS utility: it should feel like part of the menu bar, quiet until something
needs the owner.

## Stack

- **React** with function components and hooks; TypeScript strict.
- **Base UI** (`@base-ui/react`) for every interactive primitive: `Button`, `Field`, `Input`,
  `Select`, `RadioGroup`/`Radio`, `Tooltip`, `ScrollArea`, `Separator`. Never a hand-rolled
  `div` with a click handler where Base UI has the primitive; its focus, keyboard and ARIA
  behaviour are why it is here.
- **Tailwind CSS v4**, configured CSS-first. No other CSS library, no CSS-in-JS, no `style=`
  props except for values computed at runtime.

## Tokens

Shared tokens live in `packages/tray-popover/src/theme.css`; `app/ui/src/theme.css` imports it and adds only tether's own. Both use `@theme`, and nothing else defines a raw colour,
length, radius, shadow or duration. Components use the token utilities (`bg-surface`,
`text-muted`, `rounded-panel`); an arbitrary value (`bg-[#fff]`, `w-[361px]`) is a bug.

| Group | Tokens | Notes |
|---|---|---|
| Surfaces | `surface`, `surface-raised`, `surface-hover`, `border` | the panel, a row, a hovered row, hairlines |
| Text | `text`, `text-muted`, `text-faint` | primary, secondary (branch, mode, times), disabled |
| Accent | `accent`, `accent-text`, `focus` | primary button, its label, the focus ring |
| States | `in-sync`, `behind`, `skipped`, `needs-you` | a repo's state; the same four as `git tether status` |
| Radius | `radius-panel` (12px), `radius-control` (6px) | the window; buttons, inputs, rows, callouts |
| Shadow | `shadow-raised` | anything that sits above the list: callouts, the chosen mode option, menus, tooltips |
| Motion | `animate-progress` | the progress hairline's sweep; static under reduced motion |
| Type | `font-sans` = the system UI font, `font-mono` = the system monospace (shell commands, the CLI's error text); sizes `text-xs` 11px, `text-sm` 13px, `text-base` 14px | 13px is the body size, as in macOS menus |

State colours also tint, always through an opacity step of the state token: a needs-you strip
`bg-needs-you/14` with `border-needs-you/30`, the notice strip `bg-needs-you/12`, a syncing row
`bg-behind/10`, the hairline track `bg-behind/25`.

Light and dark are both first-class: every colour token and `shadow-raised` has a dark value
under `@media (prefers-color-scheme: dark)`, and the popover follows the system setting. Check
both.

## Layout

- The popover is 360px wide and at most 480px tall; the body scrolls inside a `ScrollArea`,
  the header and footer stay put. Column: header → notice strip (if any) → body → footer.
- A 4px spacing grid: padding and gaps are multiples of `1` (4px) in Tailwind's scale.
- **Header:** "tether" and the overall state (glyph and word); on the right, in `text-faint`,
  `<busy> of <total>` while a sync runs, `last sync …` when all are in sync, otherwise
  `<n> repos`; nothing while loading or on a read error. While a sync, an add, a save or a
  removal runs, a progress hairline sweeps along its bottom edge.
- **Notice strip:** a failed action, the app's note, or a read error with a stale report shows
  under the header as a needs-you-tinted strip: the triangle glyph and the text verbatim. A
  stale read error reads `Cannot read repos.toml: <error>. Showing the last report.` It clears
  on the next refresh.
- **Callout:** a raised box (`shadow-raised`, `rounded-control`) with a top strip and a body.
  The needs-you tone tints the strip; the plain tone (a confirm) does not, because removal is
  not a state.
- **Needs-you callout:** a repo that needs you sits above the list in a callout. Strip: the
  triangle, name, branch and mode, its age, and a × *Dismiss until it changes*. Body: the
  reason in full in the needs-you colour; under it, in `text-text`, the hint that says what to
  do (`Reason::hint`); *Sync now* and *Open in Finder* as visible text buttons, *Sync now* first
  only when a sync is what clears the reason (fetch failed, no remote branch, fast-forward
  refused) and *Open in Finder* first otherwise; *Settings* as an icon button at the right.
  While its sync runs, *Sync now* reads `Syncing…` and is disabled.
- **Callout stack:** with two or more callouts, only the first is open; one sliver per hidden
  callout (at most two) peeks under it, and a line under the stack reads `<n> more need you`
  with *Show all*. *Show all* opens every callout in a column, ended by *Show less*. The stack
  folds again when the popover reopens.
- **Dismiss:** a dismissed callout becomes a needs-you row at the top of the list; the header
  still counts it and the tray icon is unchanged. The dismissal is kept (in `localStorage`,
  against the reason text) until the reason changes or the repo stops needing you.
- **Repo row:** every other repo is one 32px line: a 16px state glyph, the name, the branch (and
  `· mirror` for a mirror) in `text-faint`, and the state word on the right. On hover and
  keyboard focus the row shows its actions (*Sync now*, *Open in Finder*, *Settings*) as icon
  buttons with tooltips, and its detail (commits behind, the skip reason, the last sync time) as
  a tooltip. Names and branches truncate on one line, with the path as the `title`.
- **Footer:** *Sync all* (disabled while a sync runs), *Pause/Resume*, *Add repository*; while
  paused, *Resume* is the primary button and *Add repository* goes quiet. With no repos, only
  *Pause*.
- **Add form:** replaces the list in place; the header reads *Add repository* and the footer
  holds *Cancel* and *Add*. A dashed box previews the repo as a row with a dashed-circle glyph
  and `not listed yet` where the state goes. The chosen mode option is raised. *Pull every* is
  a menu of presets (1 min, 5 min, 15 min, 1 hour; 5 min unless changed) with a check on the
  selected one. *Add commands…* reveals two optional fields, *Gate* and *After*, in monospace,
  each with a one-line description; they stay hidden until asked for. A rejection shows in a
  needs-you callout above the footer titled *Not added*, with the CLI's text verbatim.
- **Repo settings:** replaces the list in place, like the add form; the header reads the repo's
  name (truncating) with its state on the right, and the footer holds *Cancel* and *Save*
  (enabled once something changed). A needs-you repo opens with its reason in a callout. The
  facts are one-line pairs: *Folder*, *Branch* (`main · follow`), *Last sync*, and *Gate* and
  *After* in monospace only when set. *Pull every* is the add form's menu, plus the repo's own
  interval marked `(custom)` when `repos.toml` sets one that is not a preset. Below a hairline,
  *Remove from tether…* opens a plain confirm callout (`Stop keeping <name> on <branch>?`, *The
  clone stays as it is.*, *Remove* and *Keep*); nothing is unlisted on one click. Rejections show
  as *Not saved* / *Not removed* callouts.
- **Pending** (adding, saving, removing): the header hairline runs, the controls are disabled
  and the primary button says what is happening (`Adding…`, `Saving…`, `Removing…`). Nothing
  fades.
- **Loading** shows five skeleton rows with a glyph circle; a `repos.toml` that cannot be read
  fills the popover with the triangle, the error in monospace and *Try again*.

## States

A state is always a word, never colour alone. Rows lead with a 16px glyph in the state colour:
check (in sync), down arrow into a line (behind), minus in a circle (skipped), triangle (needs
you), a spinning open arc (syncing), two bars in a circle (paused). Colour goes only where
something differs: `in sync` is `text-faint`; `behind` (with ` · <n>` in `text-faint`),
`skipped` and `needs you` take their state colour. *Needs you* repos become callouts above the
list, in the order of `repos.toml`, with their reason in full.

- **Syncing:** a busy row swaps its glyph for the spinning arc and its word for `syncing…`,
  takes a `behind` wash and a progress hairline.
- **Paused:** every row's glyph becomes the pause glyph; the words stay, and needs-you callouts
  stay (an explicit *Sync now* still runs).
- **Header word**, first match wins: a read error (`needs you`), paused (`paused`), something
  needs you (`needs you (<n>)`, dismissed ones included), a sync running (`syncing…`), else the
  overall health. While a sync runs and something needs you, the word stays and only the
  hairline shows the sync.

## Copy

- English, sentence case, short. Buttons are verbs: *Sync now*, *Add repository*, *Pause*.
- Use the CLI's own words for states and reasons, so the UI and `git tether status` read alike.
- An empty list before any scan shows what a suggestion looks like (a faded box of two example
  rows with an *Add* pill), says what tether does — "tether keeps each clone you list on its
  branch. Find the clones on this Mac, or add one by hand." — and offers *Add repository* and
  *Find clones*, then "Looks in ~/Development and ~/Code." with the underlined *Where to look…*
  (one root: as is; three or more: "~/Development, ~/Code and 2 more"; none: "Looks nowhere
  yet."). After a scan with nothing left to suggest it says "No clones in ~/Development or
  ~/Code. Look somewhere else, or add one by hand." with *Where to look…* and *Add repository*,
  and a quiet line "Looked just now" (plus " · stopped early in <root> after 20 000 folders"
  when a root hit its cap). See [clone discovery](../../../docs/design/discovery/anatomy.md).
- Paths show with `~`, as the CLI prints them.

## Accessibility

- Every control is reachable by keyboard; Escape closes the popover; focus is always visible
  (the `focus` token ring).
- Text contrast is at least 4.5:1 against its surface in both themes; state colours are paired
  with their label, never used alone.
- Icon buttons carry an `aria-label` equal to their tooltip.
