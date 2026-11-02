# ADR 0006: Russian-origin build tools in the UI lockfile are listed exceptions

- **Status:** Accepted
- **Date:** 2026-11-02

## Context

The project follows a no-Russian rule: no Russian brands, packages or assets at any layer. One
exception is allowed: a transitive package used only at build or test time, which never ships,
may stay if it is listed by name in the ADR that brings it in. A direct dependency of Russian
origin is never allowed, and neither is anything that reaches the app bundle.

ADR 0005 listed the two such packages that Storybook added (`browserslist`,
`update-browserslist-db`). It also noted that `postcss`, `nanoid`, `picocolors` and `css-tree`
were already in `app/ui/package-lock.json`, through Vite, Testing Library and jsdom, and that no
ADR listed them. ADR 0002, which brought in Vite and the test stack, cannot be edited. This ADR
is the list for everything in the lockfile up to and including ADR 0005.

## Audit

Run on 2026-11-02 against `app/ui/package-lock.json` on `main` after ADR 0005: 355 entries,
324 distinct packages.

**What was checked.**

- **Every package's npm registry record** (`npm view <name>@<version>`): author, maintainers,
  repository and contributors.
- **The GitHub profile** (name, location, company) of each author or owning account whose name,
  email domain or history pointed at Russia, and of each co-maintainer with a Russian email
  domain.
- **Public talk and conference listings** for the authors where a profile alone was not enough.
- **Whether each package is direct or transitive, and dev-only.** This comes from the lockfile's
  `dev` flag and its dependents.
- **Whether each package ships.** A source-mapped `vite build` lists every module in the app
  bundle. The shipped bundle holds React, React DOM, `scheduler`, `use-sync-external-store`,
  Base UI, Floating UI, `@tauri-apps/api` and our own `src/`. Nothing below appears there. A
  text search of `app/dist/` after `npm --prefix app/ui run build` finds none of their names
  either.

**What was not checked.**

- **The Ukrainian register, the Russian company register (EGRUL) and sanctions lists.** These
  packages are written by individuals or community organisations, not by companies, so those
  registers have nothing to search.
- **Nationality.** "Origin" below is an inference from name, career and the places someone
  worked and spoke. It is not a confirmed citizenship or residence.
- **Every co-maintainer of every package.** Only those with a Russian email domain or name were
  looked at.
- **Packages a name or email would not flag.** An author with no Russian name, email or profile
  location would pass unseen.

**Criterion.** A package counts as Russian-origin when its author, or the account or
organisation that owns its repository and publishes it, is of Russian origin. A Russian-origin
co-maintainer of a project owned and led elsewhere does not make it Russian-origin; those are
recorded below but not listed.

## Decision

1. **These packages are known exceptions.** Each is transitive, `dev: true` in the lockfile,
   and absent from the app bundle. Each runs only while Vite builds, Vitest and jsdom run the
   tests, or Storybook compiles stories.

   The "Author / owner" column names the owning npm account or GitHub organisation or repository,
   not private individuals. The "Origin" column is a judgement about that owning account, not a
   confirmed fact.

   | Package | Locked | Author / owner | Comes in through | Origin |
   |---|---|---|---|---|
   | `postcss` | 8.5.32 | `postcss` organisation | `vite` | Russian |
   | `nanoid` | 3.3.20 | `nanoid` repository owner | `postcss` | Russian |
   | `browserslist` | 4.29.5 | `browserslist` organisation | `@babel/helper-compilation-targets` (Storybook, ADR 0005) | Russian |
   | `update-browserslist-db` | 1.3.4 | `browserslist` organisation | `browserslist` (ADR 0005) | Russian |
   | `caniuse-lite` | 1.0.30001820 | `browserslist` organisation (owned and published only by it) | `browserslist` | Russian (owner) |
   | `node-releases` | 2.0.63 | `node-releases` repository owner | `browserslist` | Russian |
   | `css-tree` | 3.2.1 | `css-tree` repository owner | `jsdom`, `@asamuzakjp/dom-selector` | Russian |
   | `source-map-js` | 1.2.2 | `source-map-js` repository owner | `postcss`, `css-tree`, `@tailwindcss/node` | Russian |
   | `require-from-string` | 2.0.2 | `require-from-string` repository owner | `bidi-js` (`jsdom`) | Russian |
   | `parse5` | 8.0.1 | `parse5` repository owner | `jsdom` | Probably Russian, not confirmed |
   | `picocolors` | 1.1.1 | `picocolors` repository owner | `postcss`, `@babel/code-frame`, Testing Library | Unresolved |

2. **`picocolors` is listed because its origin is unresolved, not because it was found Russian.**
   ADR 0005 grouped it with the others. The npm maintainer record does not settle the question
   either way. It stays listed until someone settles it; if it turns out not to be Russian, a
   later ADR removes it.

3. **Checked and not listed.** In each of these, a Russian-origin person co-maintains a project
   that is owned and led elsewhere:
   - `picomatch` (micromatch)
   - `@storybook/global` (Storybook)
   - `enhanced-resolve` and `tapable` (webpack)
   - `react-is` and `reselect` (Meta, Redux)

   `reselect` is a runtime dependency of Base UI, but tree-shaking leaves it out of the bundle.
   If the owner wants co-maintainers counted too, these move into the table in a new ADR.

4. **The next one is listed where it comes in.** A lockfile change that adds a Russian-origin
   build-time package lists it in the ADR or PR that adds it, with the same checks. A
   Russian-origin package that would be direct or shipped is refused, not listed.

## Considered options

- **Replace them.** `postcss` sits under Vite and Tailwind, and `css-tree` and `parse5` sit
  under jsdom. Removing them means swapping the build tool or the test DOM. That cost is far
  above the policy's bar for build-time packages that never ship.
- **List only the four that ADR 0005 named.** That would be quicker, but the full audit found
  seven more. A partial list is the blind spot the rule exists to prevent.
- **Count co-maintainers as origin.** That would be stricter, but it would pull in React,
  webpack and Storybook packages for a single maintainer among many. The criterion above keeps
  the list to projects that are Russian-led, and records the rest so the choice stays visible.

## Consequences

- Every Russian-origin package in `app/ui/package-lock.json` as of ADR 0005 is now listed or
  recorded. None is direct, and none ships.
- No tool enforces this. A lockfile diff is reviewed against this table, and against later
  ADRs, by hand.
- The Rust workspace (`Cargo.lock`) was not part of this audit.
