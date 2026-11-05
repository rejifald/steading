# 0005. Agents use the tools through the CLI

- Status: Proposed
- Date: 2026-11-05

## Context

Agents are why both tools exist. Tether's ADR 0001 starts from agents reading instructions out
of clones that nothing pulls. Reap's ADR 0001 starts from agents working in one linked worktree
per task. Yet agents meet the tools only by accident: a clone moves under them, or a worktree
is gone after its pull request merged. Nothing tells an agent that the tools exist, what they
will do to its checkout, or how to ask them anything.

Both CLIs are already the whole interface. They read git and the config directly, and they have
no IPC with the tray app (tether's ADR 0001 rule 10, reap's rule 14). An agent with a shell can
run them today. Four gaps stop it from relying on them:

- **Only some commands print JSON.** `status`, `sync`/`sweep` and reap's `salvaged` take
  `--json`. `add`, `remove`, `set`, `keep`, `unkeep`, `pause` and `resume` print only sentences
  such as "unlisted … the clone is untouched".
- **Exit codes say only whether the command ran:** 0, 1 for an error, 2 for a usage error.
  `git tether sync` exits 0 when a repo needs the operator. `git reap sweep` exits 0 when git
  refused a removal or a repo could not be read. The answer is in the output, so an agent has
  to parse it to learn whether anything went wrong.
- **The JSON has no contract.** It is the shape the popovers read, it is documented nowhere,
  and any release can change it.
- **The rules an agent needs are in the wrong place.** Never commit on a tethered branch. Keep a
  worktree you still need past its merge. Know where salvaged changes went. These rules are in
  the ADRs and READMEs, which an agent reads only if pointed at them. Operators copy them into
  their own agent instructions by hand.

Constraint: tether's ADR 0004 keeps tether a general git tool that carries no Claude-specific
code, and the same holds for reap. Whatever this ADR adds must serve any agent, and nothing
agent-specific enters a core.

## Decision

### The CLI is the interface

1. **Agents use `git tether` and `git reap`, the same commands people run.** There is no
   agent-only command and no second interface in the cores. An operation an agent needs is a
   CLI command first.
2. **Every command takes `--json`.** A command that changes config prints one object saying
   what it changed: `add`, `remove`, `set`, `keep`, `unkeep`, `pause`, `resume`, and the
   commands that ADR each-tool-suggests-the-clones-it-finds adds. An error still goes to stderr
   as text, and the exit code says which kind it was (rule 5).
3. **The JSON is a contract.** Every `--json` output is one object whose first field is
   `"schema": 1`. `sync --json` and `sweep --json` change from a bare array to
   `{"schema": 1, "repos": [...]}`. That is the one break, made before anything depends on the
   shape. Adding a field or an enum value keeps the number. Removing or renaming a field, or
   changing what one means, raises it. A reader ignores fields and enum values it does not know.
4. **The contract is written down and tested.** Each tool's `docs/json.md` lists every output
   and its fields. A snapshot test per output fails when the shape changes, so a change shows
   up in the diff of the PR that makes it.
5. **Exit codes mean something, and the README lists them:**
   - `0`: the command did what was asked. This includes the skips its rules call for, which the
     next round retries without the operator: paused, not due, the lock held by another run,
     the gate said wait.
   - `1`: the command could not run (unreadable config, no git).
   - `2`: a usage error, as today.
   - `3`: it ran, and at least one repo or worktree needs the operator. For tether, that is a
     repo whose health is *needs you*. For reap, it is a removal git refused or a repo it could
     not read.

   Only commands that act (`sync`, `sweep`) exit 3. `status` and `salvaged` exit 0 whenever
   they could read, and the state is in their output.
6. **No command prompts.** No CLI command opens a dialog, asks a question or reads stdin. This
   is true today, and this rule keeps it true. The folder picker belongs to the app.

### A skill teaches when to use it

7. **Each tool ships an agent skill** in the open `SKILL.md` format (frontmatter with `name`
   and `description`, then Markdown). It says what the tool does to the agent's checkout, how to
   find out (`status --json`), and what to run when:
   - **tether:** a listed clone's branch moves by itself, so work on another branch or in a
     worktree and never commit on it.
   - **reap:** a merged worktree is removed after its grace period, `git reap keep` holds one,
     and `git reap salvaged` lists the uncommitted changes it saved and how to restore them.
8. **The skill splits commands by who decides.** An agent runs read-only commands on its own:
   `status`, `salvaged`, and `sync`/`sweep --dry-run`. It may also `keep` and `unkeep` the
   worktree it is working in. It asks the operator before anything else that changes a repo or
   the config: `sync`, `sweep`, `add`, `remove`, `set`, `pause`, `resume`, and `keep` of any
   other worktree.
9. **The skill is part of the tool.** It lives at `tools/<tool>/agent/skills/git-<tool>/SKILL.md`
   and ships with the tool's version, changelog and release tag. A change to the CLI that makes
   the skill wrong changes the skill in the same commit, and each tool's `CLAUDE.md` says so.
10. **Distribution stays outside the cores and the installer.** `tools/<tool>/agent/` is a
    Claude Code plugin (`.claude-plugin/plugin.json` plus `skills/`). The repository root lists
    both plugins in `.claude-plugin/marketplace.json`. An operator adds the tools to their agent
    from the repository's URL. An agent that reads `SKILL.md` some other way uses the same file.
    These two manifests are the only agent-specific files in the repository.

### No MCP server yet

11. **There is no MCP server for now.** Every agent that uses the tools today has a shell. The
    question comes back when an agent without a shell needs the tools. The answer then is an
    `mcp` subcommand of each CLI binary that maps one-to-one onto the CLI commands and their JSON
    (rule 1), not a separate service.

This amends tether's ADR 0001 rule 10 and reap's ADR 0001 rule 14. Their status lines name this
ADR once it is accepted.

## Considered options

- **An MCP server now.** It gives typed tools that a shell-less agent can call. But each tool
  would have a second interface to keep in step with the first, and a protocol dependency in
  each binary, for agents that already have a shell. Deferred (rule 11).
- **A skill without changing the CLI.** The skill would have to teach agents to parse sentences
  and to ignore exit codes, and the next wording change would break them.
- **The installer writes the skill into the agent's config folder** (such as `~/.claude/skills`).
  That puts agent-specific code in the installer. It also writes into a folder that another
  tool may keep in sync, which is the situation of tether's ADR 0004. The operator adds the
  plugin through their agent instead.
- **One skill for both tools.** They are separate products with separate versions (repo ADR
  0001 rule 1), and an operator may run only one.
- **Detecting an agent and changing the output** (by an environment variable the agent sets).
  That puts agent-specific code in the cores, and every command would have two output modes to
  test. `--json` is explicit.
- **`status` exits nonzero when something needs the operator.** The most-run read command would
  then fail under `set -e`, and the state is already in its JSON.
- **No schema number, and treat any change as additive.** Then a rename would break readers
  without a signal. One integer costs nothing and settles it.

## Consequences

- An agent can find out what the tools did to its checkout, and act on it, without parsing
  prose. The rules it needs come from the tool, at the tool's version, not from instructions an
  operator copied by hand.
- `sync --json` and `sweep --json` change shape from an array to an object. That is a breaking
  change for any script that reads them, and both changelogs call it out.
- Exit 3 is new. A scheduler that runs `sync --due` or `sweep --due` now sees *needs you* as a
  failure, which is how it should log it.
- The popovers get one more field, `schema`, in the report they already read, and ignore it.
- A CLI change now has three places to keep true: the code, `docs/json.md` and the skill. The
  snapshot tests cover the first two. Review covers the skill.
- Out of scope: an MCP server (rule 11), any sign of agent activity inside the cores (an idle
  check stays a `gate` command, as in tether's ADR 0004), and agents that discover or list clones
  on their own. Listing stays the operator's decision under ADR
  each-tool-suggests-the-clones-it-finds.
