# 0001. One repository builds tether and reap

- Status: Accepted
- Date: 2026-11-02

## Context

git-tether keeps the clones an operator lists on their branch; git-reap removes worktrees whose
work has merged. They were built in two repositories, reap starting as a copy of tether: the same
Tauri menu-bar shell, the same popover parts (header, footer, read-error and state views, add
form, controls, theme), the same way of running git, the same instance lock, the same
sensitive-info gate. The copies had drifted: a fix in one (running git correctly when a hook
exports `GIT_DIR`) did not reach the other, and each lint or CI change had to be made twice.

## Decision

One public repository, **steading**, builds both tools.

1. **Two products, one repository.** Each tool keeps its own name, binaries (`git-tether` and
   `tether`, `git-reap` and `reap`), version, changelog, release tags (`tether-v<version>`,
   `reap-v<version>`), ADRs and `CLAUDE.md`, under `tools/<tool>/`. Neither depends on the
   other's crates. Merging them into one app stays rejected: what to keep current and what to
   keep slim are different lists.
2. **Shared code moves out only when both tools use it,** into a crate or npm package named for
   what it does (running git, the menu-bar shell, the instance lock, the popover parts), one
   extraction per change. Where the two copies differ, the extraction starts from the one with
   the newer fixes and keeps both tools' tests passing.
3. **One Cargo workspace** at the root: edition 2024, clippy pedantic, one `Cargo.lock`. The
   cores are the default members, so `cargo test` stays UI-free and runs on Linux.
4. **Fresh history.** The repository starts from one commit of both tools' files as of
   2026-11-02. git-tether and git-reap are archived with a pointer here and stay the record of
   earlier history; reap's history, private until now, is not published.
5. **CI on GitHub-hosted runners:** Linux for the gate (sensitive info, format, lint and tests of
   the cores, each popover's typecheck, tests and build), macOS for building both apps, and later
   for signing and notarizing the releases.

## Consequences

- A fix to shared code reaches both tools at once, and so can a regression: shared crates are
  tested by both tools' suites.
- Each tool's `CHANGELOG.md` is kept by hand from commits scoped to that tool.
- `git blame` starts at this commit; the archived repositories answer older questions.
- Releases are signed and notarized and installed as Homebrew casks from the `rejifald/steading`
  tap; the launch presents both tools together under the steading name.
