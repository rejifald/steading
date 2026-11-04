# 0003. The popovers share one npm workspace and a tray-popover package

- Status: Accepted
- Date: 2026-11-03

## Context

reap's popover started as a copy of tether's (ADR 0001). Tether's rounds 3 to 6 of the popover
design then added a raised callout, a progress hairline, a notice strip, state glyphs, the
`shadow-raised` and `font-mono` tokens and Storybook, and reap is about to take the same look. A
third copy of those pieces would drift the way the first two did: by the time of this ADR, reap's
`controls/`, `theme.css`, `Header`, `Footer` and `ReadError` already differed from tether's in
small ways nobody chose (a tooltip shadow, a glyph, a font).

ADR 0001 rule 2 says shared code moves into a package named for what it does once both tools
use it. Each popover was its own npm project with its own lockfile, so a package needs a way to
be installed into both.

## Decision

1. **The repository root is one npm workspace** (`package.json`): `packages/tray-popover`,
   `tools/tether/app/ui` and `tools/reap/app/ui`. One `package-lock.json` at the root replaces
   the two per-app lockfiles. `npm ci` runs at the root, in CI and in both apps'
   `beforeBuildCommand`; each app's scripts still run in its own folder.
2. **`packages/tray-popover/` (`@steading/tray-popover`) holds what both popovers draw the same
   way:** the shared tokens (`theme.css`), the controls (`TextButton`, `IconButton`), the icon
   set, `Callout`, `ProgressLine`, `NoticeStrip`, `ReadError`, and the frame (`PopoverFrame`,
   `PopoverHeader`, `PopoverFooter`). It is source only: no build, imported by file
   (`@steading/tray-popover/Callout`), bundled by each app's Vite.
3. **What a tool says stays in the tool.** The package knows no tool's states, words or data.
   Icons are named by shape (`WarningIcon`, `CheckCircleIcon`); each tool maps its own states to
   them and its own copy into `ReadError` and `NoticeStrip`. Colours a state is drawn in, the
   accent and `--color-progress` are each app's tokens, defined in its `theme.css` after the
   package's.
4. **A piece moves into the package when the second tool needs it,** not before, and leaves the
   first tool in the same change, so there is never a copy.
5. **The no-Russian list is unchanged in content.** ADR 0002's decision covers the root
   lockfile as it covered the two it replaces. At this ADR the root lockfile holds exactly the
   packages, at exactly the versions, of the two audited lockfiles together (checked by comparing
   every `name@version` entry; the workspace links are the only additions), so no new audit was
   needed. A later change that adds a package lists it as ADR 0002 says, naming the root lockfile.

## Considered options

- **A `file:` dependency, two lockfiles kept.** CI and the Tauri scripts would not change, but a
  symlinked package resolves React from its own real path, so each app needs Vite `dedupe` and
  matching TypeScript paths, and two installs must stay in step by hand.
- **A Vite and TypeScript path alias, no package.** The least wiring, but every tool config
  (Vite, Vitest, Storybook, tsc) repeats the alias, and nothing declares the package's peers.
- **Keep copying.** Rejected: that is how the drift above happened.

## Consequences

- A fix to a shared piece reaches both popovers in one commit; its tests and stories sit beside
  it in the package, and tether's Storybook shows them.
- Installing anything for one popover installs at the root; never run `npm install` inside
  `app/ui`.
- Both popovers move to a new version of a shared dependency (React, Base UI, Vite) together.
- Tether's built CSS is unchanged except that the progress hairline's utility is named for
  `--color-progress` (the same colour as before) and one `font-normal` rule from the compact button.
