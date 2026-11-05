# Styleguide

How reap's popover looks and is built (ADR 0001 rule 15). It shares git-tether's stack and
design, so the two menu-bar apps read as one family: a quiet macOS utility that speaks up only
when a worktree needs the owner.

## Stack

- **React** with function components and hooks; TypeScript strict.
- **Base UI** (`@base-ui/react`) for every interactive primitive (`Button`, `Tooltip`,
  `ScrollArea`, `Collapsible`); never a hand-rolled `div` with a click handler.
- **Tailwind CSS v4**, configured CSS-first. No other CSS library, no CSS-in-JS, no `style=`
  except for values computed at runtime.
- The same dependency versions as git-tether's popover; a new dependency is reviewed first, and
  no Russian-origin package at any layer.

## Tokens

Shared tokens live in `packages/tray-popover/src/theme.css`; `app/ui/src/theme.css` imports it and adds only reap's own, both under `@theme`. Tailwind's own palette, radii, type
scale and fonts are cleared, so an arbitrary value (`bg-[#fff]`, `w-[361px]`) is a bug.

| Group | Tokens | Notes |
|---|---|---|
| Surfaces | `surface`, `surface-raised`, `surface-hover`, `border` | the panel, a callout, a hovered row, hairlines |
| Text | `text`, `muted`, `faint` | primary, secondary (branch, base), quiet (counts, notes) |
| Accent | `accent`, `accent-text`, `focus` | the brand green `#2e7d4f` for the primary button; the focus ring |
| States | `open`, `removable`, `held`, `needs-you` | the colour of a worktree state and its glyph (below) |
| Progress | `progress` (= `removable`) | the hairline, the wash on a row being removed, `refreshing…` |
| Depth | `shadow-raised` | callouts, tooltips: anything above the list |
| Radius | `radius-panel` (12px), `radius-control` (6px) | the window; buttons and rows |
| Type | system UI font; `text-xs` 11px, `text-sm` 13px, `text-base` 14px; `font-mono` | 13px is the body size; monospace for git's own error text |

Light and dark are both first-class: every colour has a dark value under
`prefers-color-scheme: dark`. Spacing is a 4px grid.

## Outcomes

The popover groups worktrees by what happens to them, not by repo (the design and its reasons:
[`docs/design/popover/anatomy.md`](design/popover/anatomy.md)). A section is always a 16px
glyph, a title and a count, never colour alone; each row says in a plain sentence what happens.
Rows carry no glyph of their own: their section already says it.

| Section | States | Glyph, colour | Shown |
|---|---|---|---|
| *Needs you* | `stale`, `refused`, and a repo reap cannot read | triangle, `needs-you` | open; each a raised callout: triangle, branch and `repo · age` in a tinted strip, the reason in full and text buttons below |
| *Ready to remove* | `removable`, `missing` | bin, `removable` | open; the disk they free after the count, a lead line on the policy, 4 rows then *Show N more*, *Remove all N* with a confirm in a plain callout |
| *Merged, but held* | `dirty`, `in use`, `locked` | pause, `held` | collapsed, with the count of each kind and the disk held by uncommitted changes (`4 uncommitted (9.3 GB)`); a summary too long for the line wraps under the title |
| *In progress* | `open` | dashed circle, `open` | collapsed |
| *Kept* | `kept` | pin, `open` | collapsed |

An empty section is not shown. With nothing in *Needs you* or *Ready*, *Nothing to remove*
takes their place.

*Found* is not an outcome section: it holds the clones a scan found, not a worktree's fate, so
it has no outcome glyph and does not go through `OutcomeSection` or `outcomeOf`. It is the last
thing in the scroll, after *Repositories* ([clone discovery](../../../docs/design/discovery/anatomy.md)).

The overall state in the header is a glyph and a word, first match wins: `needs you` (the
config cannot be read), `paused`, `needs you (n)`, `refreshing…` (a spinning arc), then
`tidy` (check circle) or `removal pending` (bin, `held`), the tray icon's states. `tidy` and
`paused` stay quiet in `muted`.

## Layout

- 360px wide, at most 480px tall; the list scrolls inside a `ScrollArea`, header and footer
  stay put.
- **Header:** "reap", the overall state, and on the right `N worktrees · M repos · 27 GB` (the
  disk they take, left out when nothing was measured). A progress hairline runs along its
  bottom edge while a refresh or a removal runs.
- **Notice strip:** a failed action, the app's note, or a config error with an earlier report,
  verbatim in a needs-you-tinted strip with the triangle, under the header.
- **Sections:** in the order of the table above, across all repos.
- **Rows:** two lines. Line 1 is the branch (the folder name when HEAD is detached), then
  `repo · age · size` in `faint` (age and size left out when unknown); line 2 is the sentence in `muted`, built from the CLI's reason
  (`Merged in #403 · removes in 3 h`). Hover or focus shows the row's actions as icon buttons;
  the folder name, reason, last activity and size are its accessible description. A row being
  removed takes the `progress` wash and an inset hairline, and reads `removing…` in place of
  its meta, until the next report drops it.
- **Sizes:** what removing the folder would free (a file hard-linked from outside it, such as
  a package store's, does not count), in Finder's decimal units: `840 MB`, `3.2 GB`, `14 GB`.
- **Repositories:** at the end, `gh`'s notes and the repos with no linked worktrees, in
  `faint`.
- **Footer:** *Refresh* (disabled while refreshing), *Pause/Resume*, *Add repository*. While
  paused, *Resume* is the primary button and *Add repository* goes quiet.
- **Add form:** replaces the list in place: the folder, picked with the macOS picker, and a
  ghost preview. reap finds the remote and base itself.
- **Empty, loading, read error:** the invitation (ghost clones with their worktree counts, one
  sentence, *Add repository* and *Find clones*, and where it looks) or, after a scan that found
  nothing, where it looked; still skeleton rows led by a glyph circle; the triangle, the error
  in monospace, and *Try again*.

## Copy

English, sentence case, short; buttons are verbs. Sentences are built from the CLI's own
reasons, so the popover and `git reap status` read alike. Paths show with `~`.

## Accessibility

Every control is reachable by keyboard; Escape closes the popover; focus is always visible.
Text contrast is at least 4.5:1 in both themes. Icon buttons carry an `aria-label` equal to
their tooltip.
