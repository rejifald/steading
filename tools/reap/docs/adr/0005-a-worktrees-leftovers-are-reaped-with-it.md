# ADR 0005: a worktree's build caches, Compose stack and dev servers go with it

- **Status:** Proposed
- **Date:** 2026-11-04
- **Amends:** [ADR 0001](0001-reap-sweeps-merged-worktrees.md), rules 5, 6, 11 and 15

## Context

A worktree is more than its folder. Work in it starts things that live elsewhere on the Mac, and
reap does not know about any of them:

- **Xcode build folders.** Building an Xcode project in a worktree creates a folder under
  `~/Library/Developer/Xcode/DerivedData/`, named for the project and a hash of its path, whose
  `info.plist` records the workspace path in `WorkspacePath`. Removing the worktree leaves that
  folder behind. On one operator's Mac, 4 such folders taking 8.4 GB pointed at worktrees that no
  longer existed, each about the size of the worktree itself.
- **Docker Compose stacks.** `docker compose up` in a worktree starts containers that the Docker
  daemon runs, not a process inside the worktree, so rule 5's `lsof -d cwd` does not see them.
  reap removes the worktree while its containers keep running, with their compose file and any
  bind mounts gone from under them. The same operator had a 4-container stack running for half a
  day from a worktree. Compose labels every container with
  `com.docker.compose.project.working_dir` and `com.docker.compose.project.config_files`, so
  the worktree a stack came from is a fact, not a guess. A project name fixed in the compose file
  is shared by every worktree of the repo: whichever worktree ran `up` last owns the labels, and
  the others may be using the same stack.
- **Dev servers.** A `npm run dev` left running in a worktree has its current directory inside
  it, so rule 5 holds the worktree as *in use* for as long as the server runs, which is until
  someone stops it or the Mac restarts. reap cannot tell it apart from a shell or an agent
  session, which rule 5 exists to protect.

iOS simulators were also considered. Simulator devices are shared by every checkout on the Mac,
and nothing on a device records the worktree that installed an app onto it. A device can only be
matched to a worktree by its name, which is a guess.

## Decision

1. **Xcode build folders are deleted with their worktree.** reap reads Xcode's
   `IDECustomDerivedDataLocation` default, or uses `~/Library/Developer/Xcode/DerivedData`. A
   folder there is a *build folder* when its name is `<name>-<hash>` and its `info.plist` has a
   `WorkspacePath`; nothing else in the directory (`ModuleCache.noindex` and the like) is ever
   touched.
   - **At removal:** after `git worktree remove` succeeds, reap deletes every build folder whose
     `WorkspacePath` is inside the removed worktree.
   - **Each pass:** reap deletes every build folder whose `WorkspacePath` no longer exists and lies
     inside a listed clone's folder (covering `<clone>/.claude/worktrees/<name>`, removed by reap
     or anyone else). A worktree outside its clone's folder is covered only at removal.
   - Deletion is `rm -rf` of the folder, with no grace: Xcode recreates it on the next build.
     `clean_derived_data` (default `true`) turns it off, globally and per repo.
2. **A running Compose stack holds its worktree.** Each pass runs one
   `docker ps --all --format json` and reads the Compose labels. A worktree holds a project when
   its `working_dir` or any of its `config_files` is inside the worktree. A project with a
   running container holds the worktree as *in use*, like a process (rule 5), and its row names
   the project. A project with only stopped containers does not hold it. If `docker` is not
   installed, or its daemon does not answer, no container holds a worktree and `status` says why,
   as rule 9 does for `gh`.
3. **Dev servers can hold a worktree only alone.** A process inside a worktree (rule 5) is a
   *server* when it, or a process in its process group, listens on a TCP port
   (`lsof -iTCP -sTCP:LISTEN`) and it has no controlling terminal. A worktree is *held by
   servers* when every process inside it is a server. One shell, editor or agent process inside
   it keeps it plain *in use*, and nothing in it is ever stopped. reap and tether's own processes
   are never servers.
4. **Stopping is opt-in, before removal.** `stop_compose` and `stop_servers` (both default
   `false`, globally and per repo) let reap stop these holders. With one set, a merged, clean,
   not-kept worktree whose only holders are of that kind is *removable* (rule 6). The grace clock
   and the notification of rule 10 run as usual, and the notification names what will be stopped:
   the Compose project, or each server's command and port. Then, under the clone's lock, before
   `git worktree remove`:
   - **Compose:** `docker compose --project-name <project> down` for each project the worktree
     holds, running or stopped. Never `--volumes`, so named volumes and the data in them survive,
     and a `compose up` from another worktree brings the stack back.
   - **Servers:** `SIGTERM` to each server's process group, then up to 10 s for it to exit. reap
     never sends `SIGKILL`. A server still running then is recorded as a refusal on the row, as
     a git refusal is (rule 11), and the worktree stays.
   - reap runs `lsof` and `docker ps` again, and removes the worktree only if nothing holds it
     now. Anything that started inside it in the meantime keeps it.
5. **The operator can always stop them by hand.** The popover offers *Stop and remove* on a row
   held only by servers or Compose projects, whatever the two settings say, with the same steps
   and checks, and no grace. `--dry-run` prints the `docker compose` and `kill` commands as
   `would run:` and runs none.
6. **The popover names every holder.** Rule 15's worktree view gains two things, and
   `git reap status` prints the same facts:
   - **Holders on a held row.** In *Merged, but held*, the collapsed summary counts holders by
     kind (`1 server · 1 stack · 1 in use`, only kinds present). A row's second line says what
     holds it (`Merged in #212 · held by a dev server`), and under it one line per holder,
     indented on a hairline:
     - a server: its command, port, pid and uptime (`vite :5173 · pid 4121 · up 2 d`), with
       *Stop*;
     - a Compose project: its name, running count and uptime, then its services and
       `volumes kept` (`webapp-dev 4 running · up 12 h`), with *Stop*;
     - anything else: the process names and `reap won’t stop a shell or a session`, with no
       button.

     A row held only by servers or Compose projects offers *Stop and remove* (rule 5) beside
     *Keep*.
   - **A *Still running* section.** It lists every server and running Compose project inside a
     listed clone's worktrees, merged or not, so the operator can see what is still up. Each row
     is the holder (`vite :5173 · up 2 d`) and, below it, its worktree and that worktree's state
     (`in feat/status-page · merged #212`, `in feat/checkout · in progress`), with *Stop*. Those
     in merged worktrees come first. It sits after *Merged, but held*, collapsed, with the
     summary `<n> in merged worktrees`, and is not shown when nothing runs. The header's right
     slot leads with `<n> running` when there is any.

   *Stop* here stops that one holder, with the steps and checks of rule 4, and removes nothing.
   The tray icon's states are unchanged: a running server alone never makes reap *need you*.

Unchanged: a worktree is never removed while a process it cannot stop is inside it, reap stops
nothing on its own in a worktree that is not otherwise removable (only the operator's *Stop*
does), and reap never touches a container,
process or build folder that it cannot tie to a listed clone's worktree.

## Considered options

- **Leave it all to the operator.** Nothing is stopped by surprise, but the 8.4 GB of build
  folders and the orphaned stack are the pile-up reap exists to clear.
- **Delete build folders whose workspace is gone, anywhere on the Mac.** It would clear every
  orphan, including those of clones nobody listed, but reap acts only on listed clones (rule 1).
  A clone with orphans can be listed.
- **Treat a running stack as nothing, as today.** The worktree is removed while its stack keeps
  running against files that are no longer there. Holding the worktree shows the stack and keeps
  removal and stopping one decision.
- **Stop Compose stacks by default.** `down` without `--volumes` loses no data, but a project
  name shared by every worktree of a repo means stopping one worktree's stack can take down the
  stack another worktree is using. That needs the operator's say-so per repo.
- **Stop every process inside a merged worktree.** It frees every held worktree, but the holder
  is usually a shell or an agent session that someone will come back to, which is what rule 5
  protects.
- **`SIGKILL` after the timeout.** It always frees the worktree, but a server that ignores
  `SIGTERM` may be writing state. Leaving it running and saying so loses nothing.
- **Show holders only on held rows, with no *Still running* section.** It keeps the popover
  about removal, but a server in an unmerged worktree, or one the operator forgot, is then
  visible only by `lsof`. Listing them costs one collapsed line.
- **Stop only servers in merged worktrees.** It keeps reap out of work in progress, but the
  operator who opens *Still running* to stop a server would then need a terminal for half of
  them. *Stop* acts on one holder the operator picked, so it removes nothing and guesses nothing.
- **Delete simulators whose name matches the worktree, or apps a worktree's build installed.**
  Nothing on a device records the worktree, so any match is a guess. A guess is not enough for
  deleting a device someone else may use.

## Consequences

- Removing a merged iOS worktree frees its Xcode build folder too, often as much disk again.
- A worktree with a running Compose stack shows as *in use* where it used to be removed, so with
  `stop_compose` off, a stack nobody stopped keeps its worktree, visibly, until someone does.
- With the settings on, a forgotten dev server or stack no longer keeps a merged worktree
  forever. A stack whose project name is shared stops for every worktree of the repo, and comes
  back with the next `compose up`.
- The popover is also where an operator sees and stops what runs in their worktrees. The
  [styleguide](../styleguide.md) and the popover anatomy gain these rows when this is built.
- reap now depends on `docker` and on Xcode's defaults in addition to git and `gh`, both
  optional: without them it behaves as before.
- A process that holds files in the worktree without having its current directory there (a
  server started from elsewhere with an absolute path) is still not seen, as before.
- Out of scope: simulators and the apps installed on them, containers started without Compose
  (`docker run`), and caches other than Xcode's, such as `~/.cache` folders keyed by path.
