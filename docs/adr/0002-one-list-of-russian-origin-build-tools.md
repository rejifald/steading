# 0002. One list of Russian-origin build tools covers both tools

- Status: Accepted
- Date: 2026-11-02

## Context

The no-Russian rule lets a transitive package that runs only at build or test time, and never
ships, stay when an ADR lists it by name. tether's
[ADR 0006](../../tools/tether/docs/adr/0006-russian-origin-build-tools-are-listed-exceptions.md)
is that list for `tools/tether/app/ui/package-lock.json`. It covers tether only: it was written in
git-tether before the move, and reap's lockfile (`tools/reap/app/ui/package-lock.json`) has never
been audited. ADR 0006 cannot be edited, so this ADR widens its scope.

## Audit

Run on 2026-11-02 against reap's lockfile on `main` at `8f050a5`: 172 entries, 87 distinct
packages.

- **Every package in reap's lockfile is in tether's, at the same version.** Reap uses the same
  Vite, Tailwind, Vitest, Testing Library and jsdom versions, without Storybook. ADR 0006's checks
  (registry records, owner profiles, the criterion for origin) therefore already cover each one,
  and nothing new needed checking.
- **Present in reap, all `dev: true`:** `postcss`, `nanoid`, `css-tree`, `source-map-js`,
  `require-from-string`, `parse5`, `picocolors`. They come in through the same parents as in
  tether: `vite`, `postcss`, `jsdom`, `@asamuzakjp/dom-selector`, `@bramus/specificity`,
  `bidi-js`, `@tailwindcss/node`, `@babel/code-frame` and Testing Library.
- **Absent from reap:** `browserslist`, `update-browserslist-db`, `caniuse-lite` and
  `node-releases`, which arrive with Storybook (tether ADR 0005).
- **Recorded, not listed, present in reap:** `picomatch`, `react-is`, `tapable`,
  `enhanced-resolve` (dev), and `reselect` (a runtime dependency of Base UI).
- **Nothing flagged ships.** A source-mapped `vite build` of reap's popover holds only Base UI,
  Floating UI, React, React DOM, `scheduler`, `use-sync-external-store`, `@tauri-apps/api` and
  reap's own `src/`. Tree-shaking leaves `reselect` out. A text search of the built assets finds
  none of the listed names.

What ADR 0006 says was not checked (company registers, nationality, every co-maintainer, authors
no name or email would flag) holds here too. `Cargo.lock` was not audited.

## Decision

1. **ADR 0006's table is the list for every npm lockfile in this repository,** tether's and
   reap's, and its criterion and its "checked and not listed" record apply to both.
2. **A lockfile change in either tool** that adds a Russian-origin build-time package lists it
   in the ADR or PR that adds it, with ADR 0006's checks, and says which tool's lockfile it is in.
   A Russian-origin package that would be direct or would ship is refused, in either tool.
3. **A shared npm package extracted under ADR 0001** inherits this list; its own lockfile, if it
   has one, is audited before it merges.

## Considered options

- **A separate reap ADR repeating the table.** That gives two copies of one list, which drift
  the way the code did; a fact lives in one place.
- **A new steading-wide table superseding ADR 0006.** That would be cleaner to read, but it
  copies an accepted audit for no new information. Worth doing when the list itself changes.

## Consequences

- Every Russian-origin package in both tools' npm lockfiles is listed or recorded. None is
  direct, and none ships.
- Reviewing a lockfile diff still means reading ADR 0006's table by hand; no tool enforces it.
- Auditing `Cargo.lock` stays open for both tools.
