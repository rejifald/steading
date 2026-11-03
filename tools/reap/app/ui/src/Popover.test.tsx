import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { Repo, Snapshot } from "./api";
import { createFakeApi } from "./api.fake";
import { Popover } from "./Popover";

function setup(options: Parameters<typeof createFakeApi>[0] = {}) {
  const api = createFakeApi(options);
  const user = userEvent.setup();
  render(<Popover api={api} />);
  return { api, user };
}

const rowNamesIn = (sectionName: string) =>
  within(screen.getByRole("list", { name: sectionName }))
    .getAllByRole("listitem")
    .map((item) => item.getAttribute("aria-label"));

const expand = async (user: ReturnType<typeof userEvent.setup>, sectionName: RegExp) =>
  user.click(await screen.findByRole("button", { name: sectionName }));

function readyRepo(count: number): Repo {
  return {
    name: "busy",
    path: "~/Development/busy",
    base: "origin/main",
    policy: "ask",
    worktrees: Array.from({ length: count }, (_, index) => ({
      name: `done-${index}`,
      path: `~/Development/worktrees/done-${index}`,
      branch: `feat/done-${index}`,
      state: "removable" as const,
      reason: "merged into origin/main",
    })),
  };
}

describe("Popover", () => {
  it("shows the header with the overall state, the worktree and repo counts and the disk they take", async () => {
    setup();
    const header = await screen.findByRole("banner");
    expect(within(header).getByText("reap")).toBeInTheDocument();
    expect(within(header).getByRole("status")).toHaveTextContent("needs you (2)");
    expect(within(header).getByText("10 worktrees · 2 repos · 4.1 GB")).toBeInTheDocument();
  });

  it("uses the singular in the header for one worktree in one repo", async () => {
    setup({ repos: [sampleRepoWithoutStale()] });
    expect(await screen.findByText("1 worktree · 1 repo")).toBeInTheDocument();
  });

  it("groups worktrees by outcome across repos, in order, with counts", async () => {
    setup();
    await screen.findByRole("banner");
    const regions = await screen.findAllByRole("region");
    expect(regions.map((region) => region.getAttribute("aria-label"))).toEqual([
      "Needs you",
      "Ready to remove",
      "Merged, but held",
      "In progress",
      "Kept",
      "Repositories",
    ]);
    const countOf = (name: string) => within(screen.getByRole("region", { name })).getAllByRole("heading")[0];
    expect(countOf("Needs you")).toHaveTextContent("Needs you2");
    expect(countOf("Ready to remove")).toHaveTextContent("Ready to remove2 · 840 MB");
    expect(screen.getByRole("button", { name: /Merged, but held/ })).toHaveTextContent("3");
    expect(screen.getByRole("button", { name: /In progress/ })).toHaveTextContent("2");
    expect(screen.getByRole("button", { name: /Kept/ })).toHaveTextContent("1");
  });

  it("lists rows within a section by repo order, then worktree order", async () => {
    setup();
    await screen.findByRole("banner");
    expect(rowNamesIn("Needs you")).toEqual(["build/old", "feat/redesign"]);
    expect(rowNamesIn("Ready to remove")).toEqual(["fix/login", "fix/typo"]);
  });

  it("titles a row with its branch and with the folder name when detached", async () => {
    const { user } = setup();
    await screen.findByRole("banner");
    expect(screen.getByRole("listitem", { name: "fix/login" })).toBeInTheDocument();
    await expand(user, /In progress/);
    expect(rowNamesIn("In progress")).toEqual(["feat/api", "detached-probe"]);
  });

  it("shows the sentence and the repo, age and size on a row", async () => {
    setup();
    const row = await screen.findByRole("listitem", { name: "fix/login" });
    // The fake's removal is 20 h after it was built; floored, that reads 19 or 20 h.
    expect(within(row).getByText(/^Merged in #41 · removes in (19|20) h$/)).toBeInTheDocument();
    expect(within(row).getByText("git-reap · 1 h · 840 MB")).toBeInTheDocument();
    const gone = screen.getByRole("listitem", { name: "fix/typo" });
    expect(within(gone).getByText("Folder is already gone. A sweep clears git’s record.")).toBeInTheDocument();
    expect(within(gone).getByText("example-site")).toBeInTheDocument();
  });

  it("describes a row with its folder name, reason, last activity and size", async () => {
    const { user } = setup();
    await expand(user, /Merged, but held/);
    const row = await screen.findByRole("listitem", { name: "spike/cache" });
    expect(row).toHaveAccessibleDescription(
      "spike-cache · has uncommitted changes · last activity 30 min ago · takes 3.2 GB",
    );
  });

  it("starts the held and in-progress sections collapsed, and expands them", async () => {
    const { user } = setup();
    await screen.findByRole("banner");
    const held = screen.getByRole("button", { name: /Merged, but held/ });
    expect(held).toHaveAttribute("aria-expanded", "false");
    expect(held).toHaveTextContent("1 in use · 1 uncommitted (3.2 GB) · 1 locked");
    expect(screen.queryByRole("listitem", { name: "spike/cache" })).not.toBeInTheDocument();
    expect(screen.queryByRole("listitem", { name: "feat/api" })).not.toBeInTheDocument();
    await user.click(held);
    expect(held).toHaveAttribute("aria-expanded", "true");
    expect(rowNamesIn("Merged, but held")).toEqual(["spike/cache", "docs/pass", "build/usb"]);
    await expand(user, /In progress/);
    expect(screen.getByRole("listitem", { name: "feat/api" })).toBeInTheDocument();
  });

  it("lifts a stale worktree into Needs you with its sentence and text buttons", async () => {
    setup();
    const row = await screen.findByRole("listitem", { name: "feat/redesign" });
    expect(within(row).getByText("Pull request #12 closed without merging. reap won’t remove it.")).toBeInTheDocument();
    expect(within(row).getByText("example-site · 12 d")).toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "Open in Finder" })).toBeInTheDocument();
    expect(within(row).queryByRole("button", { name: "Try again" })).not.toBeInTheDocument();
  });

  it("opens a worktree in Finder by its listed path", async () => {
    const { api, user } = setup();
    const open = vi.spyOn(api, "openInFinder");
    const row = await screen.findByRole("listitem", { name: "feat/redesign" });
    await user.click(within(row).getByRole("button", { name: "Open in Finder" }));
    expect(open).toHaveBeenCalledWith("~/Development/worktrees/old-redesign");
  });

  it("puts a repo error under Needs you and gh notes and empty repos under Repositories", async () => {
    const { api, user } = setup({
      repos: [
        { name: "quiet", path: "~/Development/quiet", base: "origin/main", policy: "auto", gh_note: "gh is not installed", worktrees: [] },
        { name: "broken", path: "~/Development/broken", base: "origin/main", policy: "auto", error: "not a git repository", worktrees: [] },
      ],
    });
    const needsYou = await screen.findByRole("region", { name: "Needs you" });
    const errorRow = within(needsYou).getByRole("listitem", { name: "broken" });
    expect(within(errorRow).getByText("not a git repository")).toBeInTheDocument();
    expect(needsYou).toHaveTextContent("Needs you1");
    const notes = screen.getByRole("region", { name: "Repositories" });
    expect(within(notes).getByText("quiet · no linked worktrees")).toBeInTheDocument();
    expect(within(notes).getByText("quiet · gh is not installed")).toBeInTheDocument();
    expect(within(notes).queryByText(/broken/)).not.toBeInTheDocument();
    const open = vi.spyOn(api, "openInFinder");
    await user.click(within(errorRow).getByRole("button", { name: "Open in Finder" }));
    expect(open).toHaveBeenCalledWith("~/Development/broken");
  });

  it("says Nothing to remove when nothing is ready or needs you", async () => {
    setup({ repos: [sampleRepoWithoutStale()] });
    expect(await screen.findByText("Nothing to remove")).toBeInTheDocument();
    expect(screen.getByText(/No worktree has merged since the last sweep/)).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Ready to remove" })).not.toBeInTheDocument();
  });

  it("does not say Nothing to remove while something is ready", async () => {
    setup();
    await screen.findByRole("banner");
    expect(screen.queryByText("Nothing to remove")).not.toBeInTheDocument();
  });

  it("offers to add a repository when nothing is listed, with only Pause in the footer", async () => {
    const { user } = setup({ repos: [] });
    expect(await screen.findByText(/reap removes the worktrees whose work has merged/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Refresh" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add repository" }));
    expect(screen.getByRole("form", { name: "Add repository" })).toBeInTheDocument();
  });

  it("shows checking and a disabled Refresh before the first report", () => {
    const api = { ...createFakeApi(), report: () => new Promise<Snapshot>(() => {}) };
    render(<Popover api={api} />);
    expect(screen.getByRole("status")).toHaveTextContent("checking…");
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
    expect(screen.queryByText(/reap removes the worktrees/)).not.toBeInTheDocument();
  });

  it("shows refreshing while a refresh runs and disables Refresh", async () => {
    const { user } = setup({ repos: [sampleRepoWithoutStale()] });
    await user.click(await screen.findByRole("button", { name: "Refresh" }));
    expect(await screen.findByText("refreshing…")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
    await waitFor(() => expect(screen.queryByText("refreshing…")).not.toBeInTheDocument());
  });

  it("marks a row removing, with the header hairline, while Remove now runs", async () => {
    const { user } = setup({ holdsRemovals: true });
    const row = await screen.findByRole("listitem", { name: "fix/login" });
    await user.click(within(row).getByRole("button", { name: "Remove now" }));
    expect(row).toHaveAttribute("aria-busy", "true");
    expect(within(row).getByText("removing…")).toBeInTheDocument();
    expect(within(row).queryByRole("button", { name: "Remove now" })).not.toBeInTheDocument();
    expect(screen.getByRole("banner").querySelector(".animate-progress")).not.toBeNull();
    expect(screen.queryByRole("button", { name: /^Remove all/ })).not.toBeInTheDocument();
  });

  it("keeps the last report under a notice when repos.toml stops reading", async () => {
    setup({ repos: [readyRepo(1)], error: "expected `=` at line 4" });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Cannot read repos.toml: expected `=` at line 4. Showing the last report.",
    );
    expect(screen.getByRole("listitem", { name: "feat/done-0" })).toBeInTheDocument();
  });

  it("says refreshing in the header while a refresh runs", async () => {
    setup({ refreshing: true, repos: [readyRepo(1)] });
    await screen.findByRole("banner");
    expect(screen.getByRole("status")).toHaveTextContent("refreshing…");
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
  });

  it("says paused in the header even while something needs you", async () => {
    setup({ paused: true });
    await screen.findByRole("banner");
    expect(screen.getByRole("status")).toHaveTextContent(/^paused$/);
  });

  it("toggles Pause to Resume, says paused in the header, and makes Resume the primary action", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Pause" }));
    const resume = await screen.findByRole("button", { name: "Resume" });
    expect(screen.getByRole("status")).toHaveTextContent(/^paused$/);
    expect(resume).toHaveClass("bg-accent");
    expect(screen.getByRole("button", { name: "Add repository" })).not.toHaveClass("bg-accent");
  });

  it("shows the read error panel and reloads from Try again", async () => {
    const api = createFakeApi();
    const broken: Snapshot = { report: null, error: "expected a table at line 3", refreshing: false, note: null };
    const report = vi.fn().mockResolvedValueOnce(broken).mockImplementation(() => createFakeApi().report());
    render(<Popover api={{ ...api, report }} />);
    expect(await screen.findByText("Cannot read repos.toml")).toBeInTheDocument();
    expect(screen.getByText("expected a table at line 3")).toBeInTheDocument();
    expect(screen.getByText("Fix the file, then try again. Nothing is checked until it reads.")).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("region", { name: "Needs you" })).toBeInTheDocument();
  });

  it("hides on Escape and quits on Cmd+Q", async () => {
    const { api, user } = setup();
    const hide = vi.spyOn(api, "hide");
    const quit = vi.spyOn(api, "quit");
    await screen.findByRole("banner");
    await user.keyboard("{Escape}");
    expect(hide).toHaveBeenCalled();
    await user.keyboard("{Meta>}q{/Meta}");
    expect(quit).toHaveBeenCalled();
  });

  it("adds a repository from the form and returns to the list", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Add repository" }));
    expect(screen.getByRole("heading", { name: "Add repository" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Choose…" }));
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(await screen.findByText("example-new · no linked worktrees")).toBeInTheDocument();
    expect(screen.queryByRole("form", { name: "Add repository" })).not.toBeInTheDocument();
  });
});

describe("Popover worktree actions", () => {
  it("removes a ready worktree with Remove now", async () => {
    const { user } = setup();
    const row = await screen.findByRole("listitem", { name: "fix/login" });
    await user.click(within(row).getByRole("button", { name: "Remove now" }));
    expect(screen.queryByRole("listitem", { name: "fix/login" })).not.toBeInTheDocument();
  });

  it("keeps a ready worktree, then offers Unkeep and puts it back with Unkeep", async () => {
    const { user } = setup();
    const row = await screen.findByRole("listitem", { name: "fix/login" });
    await user.click(within(row).getByRole("button", { name: "Keep" }));
    await expand(user, /Kept/);
    const kept = await screen.findByRole("listitem", { name: "fix/login" });
    expect(within(kept).getByText("Merged into origin/main")).toBeInTheDocument();
    expect(within(kept).getByRole("button", { name: "Unkeep" })).toBeInTheDocument();
    expect(within(kept).queryByRole("button", { name: "Remove now" })).not.toBeInTheDocument();
    await user.click(within(kept).getByRole("button", { name: "Unkeep" }));
    expect(within(screen.getByRole("list", { name: "Ready to remove" })).getByRole("listitem", { name: "fix/login" })).toBeInTheDocument();
  });

  it("offers no Keep or Remove now on work in progress", async () => {
    const { user } = setup();
    await expand(user, /In progress/);
    const row = await screen.findByRole("listitem", { name: "feat/api" });
    expect(within(row).queryByRole("button", { name: "Keep" })).not.toBeInTheDocument();
    expect(within(row).queryByRole("button", { name: "Remove now" })).not.toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "Open in Finder" })).toBeInTheDocument();
  });

  it("offers Keep but not Remove now on a held worktree", async () => {
    const { user } = setup();
    await expand(user, /Merged, but held/);
    const row = await screen.findByRole("listitem", { name: "spike/cache" });
    expect(within(row).getByRole("button", { name: "Keep" })).toBeInTheDocument();
    expect(within(row).queryByRole("button", { name: "Remove now" })).not.toBeInTheDocument();
  });

  it("lifts a refused removal into Needs you with Try again, which removes it again", async () => {
    const { api, user } = setup();
    const remove = vi.spyOn(api, "removeNow");
    const row = await screen.findByRole("listitem", { name: "build/old" });
    expect(within(row).getByText(/git refused to remove it/)).toBeInTheDocument();
    await user.click(within(row).getByRole("button", { name: "Try again" }));
    expect(remove).toHaveBeenCalledWith("~/Development/worktrees/old-build");
  });

  it("keeps a refused worktree from its Needs-you row", async () => {
    const { api, user } = setup();
    const keep = vi.spyOn(api, "keep");
    const row = await screen.findByRole("listitem", { name: "build/old" });
    await user.click(within(row).getByRole("button", { name: "Keep" }));
    expect(keep).toHaveBeenCalledWith("~/Development/worktrees/old-build");
  });

  it("tells when a ready worktree will go", async () => {
    setup();
    const row = await screen.findByRole("listitem", { name: "fix/login" });
    expect(row).toHaveAccessibleDescription(/removes in \d+ h/);
  });
});

describe("Popover Ready to remove", () => {
  it("shows four rows, then Show N more reveals the rest for good", async () => {
    const { user } = setup({ repos: [readyRepo(6)] });
    await screen.findByRole("banner");
    expect(rowNamesIn("Ready to remove")).toHaveLength(4);
    await user.click(screen.getByRole("button", { name: "Show 2 more" }));
    expect(rowNamesIn("Ready to remove")).toHaveLength(6);
    expect(screen.queryByRole("button", { name: /Show \d+ more/ })).not.toBeInTheDocument();
  });

  it("leads with the policy sentence", async () => {
    setup({ repos: [readyRepo(1)] });
    expect(await screen.findByText("Merged and clean. You remove them here; reap waits (policy: ask).")).toBeInTheDocument();
  });

  it("removes every ready worktree, in order, after Remove all and a confirm", async () => {
    const { api, user } = setup({ repos: [readyRepo(6)] });
    const remove = vi.spyOn(api, "removeNow");
    await user.click(await screen.findByRole("button", { name: "Remove all 6" }));
    expect(screen.getByText("Remove 6 worktrees?")).toBeInTheDocument();
    expect(remove).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(remove).toHaveBeenCalledTimes(6));
    expect(remove.mock.calls.map(([path]) => path)).toEqual(
      Array.from({ length: 6 }, (_, index) => `~/Development/worktrees/done-${index}`),
    );
    await waitFor(() => expect(screen.queryByRole("region", { name: "Ready to remove" })).not.toBeInTheDocument());
  });

  it("leaves a missing worktree out of Remove all and gives its row no Remove now", async () => {
    const repo = readyRepo(2);
    repo.worktrees.unshift({
      name: "gone",
      path: "~/Development/worktrees/gone",
      branch: "feat/gone",
      state: "missing",
      reason: "folder is gone",
    });
    const { api, user } = setup({ repos: [repo] });
    const remove = vi.spyOn(api, "removeNow");
    const gone = await screen.findByRole("listitem", { name: "feat/gone" });
    expect(within(gone).queryByRole("button", { name: "Remove now" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Remove all 2" }));
    await user.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(remove).toHaveBeenCalledTimes(2));
    expect(remove.mock.calls.map(([path]) => path)).toEqual([
      "~/Development/worktrees/done-0",
      "~/Development/worktrees/done-1",
    ]);
  });

  it("moves focus to Cancel, and Cancel closes the confirm without removing", async () => {
    const { api, user } = setup({ repos: [readyRepo(2)] });
    const remove = vi.spyOn(api, "removeNow");
    await user.click(await screen.findByRole("button", { name: "Remove all 2" }));
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("button", { name: "Remove all 2" })).toBeInTheDocument();
    expect(remove).not.toHaveBeenCalled();
  });

  it("closes the confirm on Escape without removing and without hiding the popover", async () => {
    const { api, user } = setup({ repos: [readyRepo(2)] });
    const remove = vi.spyOn(api, "removeNow");
    const hide = vi.spyOn(api, "hide");
    await user.click(await screen.findByRole("button", { name: "Remove all 2" }));
    await user.keyboard("{Escape}");
    expect(screen.getByRole("button", { name: "Remove all 2" })).toBeInTheDocument();
    expect(remove).not.toHaveBeenCalled();
    expect(hide).not.toHaveBeenCalled();
  });

  it("stops at the first failed removal and keeps its error in view", async () => {
    const api = createFakeApi({ repos: [readyRepo(3)] });
    const removeNow = vi.fn().mockRejectedValueOnce("git refused to remove done-0").mockImplementation(api.removeNow);
    const user = userEvent.setup();
    render(<Popover api={{ ...api, removeNow }} />);
    await user.click(await screen.findByRole("button", { name: "Remove all 3" }));
    await user.click(screen.getByRole("button", { name: "Remove" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("git refused to remove done-0");
    expect(removeNow).toHaveBeenCalledTimes(1);
  });
});

function sampleRepoWithoutStale() {
  return {
    name: "calm",
    path: "~/Development/calm",
    base: "origin/main",
    policy: "auto" as const,
    worktrees: [
      { name: "one", path: "~/Development/calm-one", branch: "one", state: "open" as const, reason: "not merged into origin/main" },
    ],
  };
}
