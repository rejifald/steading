# ADR 0002: the tray UI is a React popover

- **Status:** Accepted
- **Date:** 2026-10-28
- **Supersedes:** the dropdown and dialog parts of [ADR 0001](0001-tether-keeps-listed-repos-on-their-branch.md)
  rule 11. Its three icon states, its notification rule and every other rule stand.

## Context

ADR 0001 built the tray as a native macOS menu (`app/src/tray.rs`) with `osascript` prompts for
*Add repository…*. A native menu cannot be styled: it shows plain text rows, it cannot hold a
form, and adding a repo takes three modal prompts in a row. The owner chose a designed UI, on
one stack and one styleguide (owner decision, 2026-10-28).

## Decision

1. **Stack.** The UI is React with [Base UI](https://base-ui.com) (`@base-ui/react`, unstyled,
   accessible primitives) and Tailwind CSS v4, built with Vite and TypeScript. The look is set
   by [the styleguide](../styleguide.md); no other component or CSS library.
2. **A popover, not a menu.** Left-clicking the tray icon toggles one borderless window under
   the icon: a header with the overall state, one row per repo (name, branch, mode, state,
   commits behind or the skip reason, last sync, *Sync now*, *Open in Finder*), and a footer
   with *Sync all*, *Pause/Resume* and *Add repository*. Adding a repo is a form in the popover
   (folder, branch defaulting to the one checked out, mode defaulting to `follow`). The window
   hides when it loses focus or on Escape. It has no Dock icon, no title bar, and is not in the
   app switcher (ADR 0001 rule 10 stands).
3. **A native fallback.** Right-clicking the icon shows a minimal native menu (*Sync all now*,
   *Quit tether*), so a broken web view never leaves tether without a way out.
4. **The core stays UI-free.** The frontend talks to the app only through Tauri commands and
   one event. The commands call `tether-core` exactly as the menu did; no sync logic moves into
   TypeScript. The worker thread, the coalescing of clicks and the rule that the event loop
   never runs git (ADR 0001, README § Menu) are unchanged. The window receives the same
   `Report` that `git tether status --json` prints, so the CLI and the UI never disagree.
5. **Native where macOS is native.** Notifications and the folder picker stay `osascript`
   (`choose folder`), so the app still needs no Tauri dialog or notification plugin.
6. **Built into the bundle.** The frontend lives in `app/ui/`, `cargo tauri build` builds it
   into `app/dist/`, and the app loads it from the bundle: no dev server, no network, a strict
   content security policy.
7. **Tested in two layers.** Components are tested with Vitest and Testing Library against a
   fake of the command layer; the Rust side keeps its unit and integration tests. The UI can be
   opened in a browser against that fake, for review without building the app.

## Considered options

- **Keep the native menu and open web windows only for forms.** Smallest change and the
  standard macOS feel, but the menu, which is most of the UI, stays outside the styleguide.
- **Record the stack and build nothing yet.** Cheapest, but leaves the three-prompt add flow.
- **A different component library** (Radix, shadcn/ui, a styled kit). The owner chose Base UI;
  it is unstyled, so the styleguide alone sets the look.
- **SwiftUI menu-bar extra.** Native and small, but a second language and a separate build
  next to the Rust core, and the owner chose React.

## Consequences

- The repo gains a Node toolchain (`app/ui/package.json`, a lockfile) next to Cargo. Building
  the app needs Node; the CLI (`tether-core`, `git-tether`) does not.
- The bundle grows by the frontend's size, and the app now renders a web view on click.
- Every new dependency in `app/ui/` is reviewed before it lands: pinned in the
  lockfile, and no Russian-origin package at any layer.
