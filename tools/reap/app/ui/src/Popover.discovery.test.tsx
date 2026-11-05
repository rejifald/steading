import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { Repo, RootInfo } from "./api";
import { createFakeApi, DEFAULT_ROOTS, type FakeOptions, sampleSuggestions } from "./api.fake";
import { Popover } from "./Popover";

const root = (path: string, fields: Partial<RootInfo> = {}): RootInfo => ({
  path,
  clones: null,
  stopped_early: false,
  error: null,
  ...fields,
});
const SCANNED = [root("~/Development", { clones: 5 }), root("~/Code", { clones: 3 })];

const listedRepo: Repo = {
  name: "git-reap",
  path: "~/Development/git-reap",
  base: "origin/main",
  policy: "auto",
  gh_note: "gh is not installed: only ancestry merges are seen",
  worktrees: [],
};

function setup(options: FakeOptions = {}) {
  const api = createFakeApi({ scanTime: 5, ...options });
  const user = userEvent.setup();
  render(<Popover api={api} />);
  return { api, user };
}

/** Nothing listed, a scan done, four suggestions. */
const FOUND: FakeOptions = { repos: [], found: sampleSuggestions(), scannedMinutesAgo: 0, roots: SCANNED };
/** One repo listed, a scan done 3 days ago, four suggestions. */
const LISTED: FakeOptions = {
  repos: [listedRepo],
  found: sampleSuggestions(),
  scannedMinutesAgo: 3 * 24 * 60,
  roots: SCANNED,
};

const found = () => screen.getByRole("region", { name: "Found" });
const footer = () => screen.getByRole("contentinfo");
const header = () => screen.getByRole("banner");

describe("Popover with nothing listed", () => {
  it("invites to find clones before any scan, with only Pause in the footer", async () => {
    setup({ repos: [] });
    expect(await screen.findByRole("button", { name: "Find clones" })).toBeInTheDocument();
    expect(screen.getByText("Looks in ~/Development and ~/Code.")).toBeInTheDocument();
    expect(within(footer()).getAllByRole("button").map((button) => button.textContent)).toEqual(["Pause"]);
    expect(within(header()).queryByText(/found/)).not.toBeInTheDocument();
  });

  it("says Looks nowhere yet with no root, and names three or more roots with a count", async () => {
    const { unmount } = render(<Popover api={createFakeApi({ repos: [], roots: [] })} />);
    expect(await screen.findByText("Looks nowhere yet.")).toBeInTheDocument();
    unmount();
    const roots = [...DEFAULT_ROOTS, "~/Projects", "~/src", "~/work"].map((path) => root(path));
    render(<Popover api={createFakeApi({ repos: [], roots })} />);
    expect(await screen.findByText("Looks in ~/Development, ~/Code and 3 more.")).toBeInTheDocument();
  });

  it("scans from Find clones: the hairline, rows per root, then the finished section", async () => {
    const { user } = setup({ repos: [], scanTime: 150 });
    await user.click(await screen.findByRole("button", { name: "Find clones" }));
    expect(await screen.findByRole("region", { name: "Found" })).toBeInTheDocument();
    expect(within(found()).getByRole("status")).toHaveTextContent("looking in ~/Development…");
    expect(within(found()).getByText("You can add these now; more appear as the search goes.")).toBeInTheDocument();
    expect(header().querySelector("span[aria-hidden='true']")).not.toBeNull();
    await waitFor(() => expect(within(found()).getByRole("status")).toBeEmptyDOMElement(), { timeout: 2000 });
    expect(within(found()).getAllByRole("listitem")).toHaveLength(4);
    expect(header().querySelector("span[aria-hidden='true']")).toBeNull();
    expect(within(header()).getByText("4 found")).toBeInTheDocument();
  });

  it("shows Found as the whole body while a scan runs, with rows so far and ghosts", async () => {
    setup({ repos: [], scanning: "~/Code", found: sampleSuggestions().slice(0, 2), roots: [root("~/Development", { clones: 5 }), root("~/Code")] });
    await screen.findByRole("region", { name: "Found" });
    expect(within(found()).getByRole("status")).toHaveTextContent("looking in ~/Code…");
    expect(within(found()).getByRole("list", { name: "Found" }).children).toHaveLength(2);
    expect(found().querySelectorAll("ul[aria-hidden='true'] > li")).toHaveLength(3);
    expect(within(header()).getByText("2 found")).toBeInTheDocument();
    expect(within(footer()).getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Pause",
      "Add repository",
    ]);
    expect(within(footer()).getByRole("button", { name: "Add repository" })).not.toHaveClass("bg-accent");
  });

  it("counts 0 found while the first root is still being walked", async () => {
    setup({ repos: [], scanning: "~/Development" });
    await screen.findByRole("region", { name: "Found" });
    expect(within(header()).getByText("0 found")).toBeInTheDocument();
  });

  it("lists the suggestions most worktrees first, with the count and root, once the scan is done", async () => {
    setup(FOUND);
    await screen.findByRole("region", { name: "Found" });
    expect(within(header()).getByText("4 found")).toBeInTheDocument();
    expect(within(found()).getByRole("heading", { name: "Found 4" })).toBeInTheDocument();
    expect(
      within(found()).getByText(
        "Clones with worktrees in ~/Development and ~/Code, just now. Merged worktrees go 24 h after reap first sees them.",
      ),
    ).toBeInTheDocument();
    const rows = within(found()).getAllByRole("listitem");
    expect(rows.map((row) => row.textContent?.replace(/Add$/, ""))).toEqual([
      "api7 worktrees · ~/Development",
      "web3 worktrees · ~/Development",
      "docs2 worktrees · ~/Code",
      "tools1 worktree · ~/Code",
    ]);
    expect(within(found()).getByText("api")).toHaveAttribute("title", "~/Development/api");
    expect(within(found()).getByRole("button", { name: "Where to look…" })).toBeInTheDocument();
    expect(within(found()).getByRole("button", { name: "Skip api" })).toBeInTheDocument();
  });

  it("reports an unreadable root and names the ones searched", async () => {
    setup({
      repos: [],
      found: sampleSuggestions().slice(0, 2),
      scannedMinutesAgo: 0,
      roots: [root("~/Development", { clones: 5 }), root("~/Code", { clones: 3 }), root("~/work", { error: "permission denied" })],
    });
    expect(
      await screen.findByText("Cannot read ~/work: permission denied. ~/Development and ~/Code were searched."),
    ).toBeInTheDocument();
    expect(within(found()).getAllByRole("listitem")).toHaveLength(2);
  });

  it("says nothing was found, where it looked and where it stopped early", async () => {
    const { user } = setup({
      repos: [],
      found: [],
      scannedMinutesAgo: 0,
      roots: [root("~/Development", { clones: 0 }), root("~/Code", { clones: 0, stopped_early: true })],
    });
    expect(
      await screen.findByText("No clones with worktrees in ~/Development and ~/Code. Look somewhere else, or add one by hand."),
    ).toBeInTheDocument();
    expect(screen.getByText("Looked just now · stopped early in ~/Code after 20 000 folders.")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Found" })).not.toBeInTheDocument();
    expect(within(footer()).getAllByRole("button").map((button) => button.textContent)).toEqual(["Pause"]);
    await user.click(screen.getByRole("button", { name: "Add repository" }));
    expect(screen.getByRole("form", { name: "Add repository" })).toBeInTheDocument();
  });

  it("falls to the nothing-found state when the last suggestion is skipped", async () => {
    const { user } = setup({ ...FOUND, found: sampleSuggestions().slice(0, 1) });
    await user.click(await screen.findByRole("button", { name: "Skip api" }));
    expect(await screen.findByText(/^No clones with worktrees in/)).toBeInTheDocument();
  });
});

describe("Popover with repos listed", () => {
  it("puts Found last in the scroll, after the outcome sections and the repository notes", async () => {
    setup({ ...LISTED, repos: [{ ...listedRepo, worktrees: [] }] });
    await screen.findByRole("region", { name: "Found" });
    const names = screen.getAllByRole("region").map((region) => region.getAttribute("aria-label"));
    expect(names.at(-1)).toBe("Found");
    expect(names.at(-2)).toBe("Repositories");
  });

  it("collapses Found to one line while suggestions remain and nothing was done this session", async () => {
    const { user } = setup(LISTED);
    const toggle = await screen.findByRole("button", { name: /Found/ });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(toggle).toHaveTextContent("Found 4 · looked 3 days ago");
    expect(screen.queryByRole("button", { name: "Skip api" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Look again" })).toBeInTheDocument();
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("button", { name: "Skip api" })).toBeInTheDocument();
  });

  it("opens Found, with a rule above, once a suggestion is answered, and keeps it open", async () => {
    const { user } = setup(LISTED);
    await user.click(await screen.findByRole("button", { name: /Found/ }));
    await user.click(screen.getByRole("button", { name: "Skip api" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Skip api" })).not.toBeInTheDocument());
    expect(screen.queryByRole("button", { name: /^Found/ })).not.toBeInTheDocument();
    expect(within(found()).getByRole("heading", { name: "Found 3" }).parentElement).toHaveClass("border-t", "border-border");
  });

  it("folds back to one line when the popover is shown again", async () => {
    const { user } = setup(LISTED);
    await user.click(await screen.findByRole("button", { name: /Found/ }));
    await user.click(screen.getByRole("button", { name: "Skip api" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Skip api" })).not.toBeInTheDocument());
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(await screen.findByRole("button", { name: /Found/ })).toHaveAttribute("aria-expanded", "false");
  });

  it("opens Found while a scan runs", async () => {
    setup({ ...LISTED, scanning: "~/Code" });
    await screen.findByRole("region", { name: "Found" });
    expect(within(found()).getByRole("heading", { name: "Found 4" })).toBeInTheDocument();
    expect(within(found()).getByRole("status")).toHaveTextContent("looking in ~/Code…");
    expect(header().querySelector("span[aria-hidden='true']")).not.toBeNull();
  });

  it("opens Found from Look again on the collapsed line and scans", async () => {
    const { user } = setup({ ...LISTED, scanTime: 150 });
    await user.click(await screen.findByRole("button", { name: "Look again" }));
    expect(await screen.findByRole("heading", { name: "Found 0" })).toBeInTheDocument();
    await waitFor(() => expect(within(found()).getByRole("heading", { name: "Found 4" })).toBeInTheDocument(), {
      timeout: 2000,
    });
  });

  it("settles when a scan ran and none remain, saying nothing new when the roots held clones", async () => {
    setup({ ...LISTED, found: [] });
    expect(await screen.findByText("Nothing new in ~/Development and ~/Code · looked 3 days ago")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Look again" })).toBeInTheDocument();
  });

  it("settles with No suggestions when the roots held no clone", async () => {
    setup({ ...LISTED, found: [], roots: [root("~/Development", { clones: 0 })], scannedMinutesAgo: 0 });
    expect(await screen.findByText("No suggestions · looked just now")).toBeInTheDocument();
  });

  it("shows no Found section before the first scan", async () => {
    setup({ repos: [listedRepo] });
    await screen.findByRole("banner");
    expect(screen.queryByRole("region", { name: "Found" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Look again" })).not.toBeInTheDocument();
  });

  it("keeps the usual footer and the worktree summary in the header", async () => {
    setup(LISTED);
    await screen.findByRole("region", { name: "Found" });
    expect(within(footer()).getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Refresh",
      "Pause",
      "Add repository",
    ]);
    expect(within(header()).queryByText(/found/)).not.toBeInTheDocument();
  });
});

describe("Popover Add and Skip", () => {
  it("moves an added clone into the list and keeps Found open under it", async () => {
    const { api, user } = setup(FOUND);
    const addSuggestion = vi.spyOn(api, "addSuggestion");
    const row = (await within(await screen.findByRole("region", { name: "Found" })).findByText("api")).closest("li");
    await user.click(within(row as HTMLElement).getByRole("button", { name: "Add" }));
    expect(addSuggestion).toHaveBeenCalledWith("~/Development/api");
    await waitFor(() => expect(within(found()).getByRole("heading", { name: "Found 3" })).toBeInTheDocument());
    expect(within(found()).queryByText("api")).not.toBeInTheDocument();
    expect(within(found()).getByRole("heading", { name: "Found 3" }).parentElement).toHaveClass("border-border");
    expect(within(header()).getByText("7 worktrees · 1 repo")).toBeInTheDocument();
  });

  it("shows add's own error verbatim in the notice strip and keeps the row", async () => {
    const { user } = setup({ ...FOUND, addError: "~/Development/api is a linked worktree" });
    const row = (await screen.findByText("api")).closest("li") as HTMLElement;
    await user.click(within(row).getByRole("button", { name: "Add" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("~/Development/api is a linked worktree");
    expect(screen.getByText("api")).toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "Add" })).toBeEnabled();
    expect(within(found()).getAllByRole("listitem")).toHaveLength(4);
  });

  it("disables Add on the row while its add is in flight", async () => {
    const { user } = setup({ ...FOUND, holdsAdds: true });
    const row = (await screen.findByText("api")).closest("li") as HTMLElement;
    await user.click(within(row).getByRole("button", { name: "Add" }));
    await waitFor(() => expect(within(row).getByRole("button", { name: "Add" })).toBeDisabled());
    expect(within(row).getByRole("button", { name: "Add" })).toHaveAttribute("aria-busy", "true");
    expect(header().querySelector("span[aria-hidden='true']")).not.toBeNull();
    expect(within(found()).getAllByRole("listitem")).toHaveLength(4);
  });

  it("skips a clone: the row goes, the count drops, and Where to look lists it", async () => {
    const { api, user } = setup(FOUND);
    const skipClone = vi.spyOn(api, "skipClone");
    await user.click(await screen.findByRole("button", { name: "Skip web" }));
    expect(skipClone).toHaveBeenCalledWith("~/Development/web");
    await waitFor(() => expect(within(header()).getByText("3 found")).toBeInTheDocument());
    expect(screen.queryByText("web")).not.toBeInTheDocument();
    await user.click(within(found()).getByRole("button", { name: "Where to look…" }));
    expect(within(screen.getByRole("region", { name: "Skipped" })).getByText("web")).toBeInTheDocument();
  });
});

describe("Popover Where to look", () => {
  const SKIPPED = [{ path: "~/Development/scratch", name: "scratch", root: "~/Development" }];

  async function openWhereToLook(options: FakeOptions) {
    const result = setup(options);
    await result.user.click(await screen.findByRole("button", { name: "Where to look…" }));
    return result;
  }

  it("replaces the body with the folders and the skipped clones, under its own title", async () => {
    await openWhereToLook({ ...FOUND, skipped: SKIPPED });
    expect(within(header()).getByRole("heading", { name: "Where to look" })).toBeInTheDocument();
    expect(within(header()).queryByText(/found/)).not.toBeInTheDocument();
    const folders = screen.getByRole("list", { name: "Folders" });
    expect(within(folders).getAllByRole("listitem").map((item) => item.textContent)).toEqual([
      "~/Development5 clones",
      "~/Code3 clones",
    ]);
    expect(screen.getByText("Desktop, Documents and Downloads are searched only if you add them here. macOS asks you first.")).toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: /Skipped/ })).getByText("scratch")).toBeInTheDocument();
    expect(within(footer()).getAllByRole("button").map((button) => button.textContent)).toEqual(["Done", "Look again"]);
    expect(within(footer()).getByRole("button", { name: "Look again" })).toHaveClass("bg-accent");
  });

  it("says 1 clone in the singular, hides an empty Skipped, and shows a root's error in place of its count", async () => {
    await openWhereToLook({
      ...FOUND,
      roots: [root("~/Development", { clones: 1 }), root("~/work", { error: "permission denied" }), root("~/src")],
    });
    const folders = screen.getByRole("list", { name: "Folders" });
    expect(within(folders).getAllByRole("listitem").map((item) => item.textContent)).toEqual([
      "~/Development1 clone",
      "~/workpermission denied",
      "~/src",
    ]);
    expect(screen.queryByRole("region", { name: "Skipped" })).not.toBeInTheDocument();
  });

  it("returns to the list from Done without scanning", async () => {
    const { api, user } = setup(LISTED);
    // Where to look sits in the open heading: open Found by answering a suggestion first.
    await user.click(await screen.findByRole("button", { name: /Found/ }));
    await user.click(screen.getByRole("button", { name: "Skip api" }));
    await user.click(await screen.findByRole("button", { name: "Where to look…" }));
    const findClones = vi.spyOn(api, "findClones");
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.queryByRole("heading", { name: "Where to look" })).not.toBeInTheDocument();
    expect(await screen.findByRole("region", { name: "Found" })).toBeInTheDocument();
    expect(findClones).not.toHaveBeenCalled();
  });

  it("returns to the list and scans from Look again", async () => {
    const { api, user } = await openWhereToLook({ ...FOUND, scanTime: 150 });
    const findClones = vi.spyOn(api, "findClones");
    await user.click(screen.getByRole("button", { name: "Look again" }));
    expect(findClones).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("heading", { name: "Where to look" })).not.toBeInTheDocument();
    expect(await within(await screen.findByRole("region", { name: "Found" })).findByRole("status")).toHaveTextContent(
      "looking in ~/Development…",
    );
  });

  it("stops looking in a folder, and disables Look again with none left", async () => {
    const { user } = await openWhereToLook(FOUND);
    await user.click(screen.getByRole("button", { name: "Stop looking in ~/Development" }));
    await waitFor(() => expect(screen.queryByText("~/Development")).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Stop looking in ~/Code" }));
    await waitFor(() => expect(screen.queryByRole("list", { name: "Folders" })).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Add folder…" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Look again" })).toBeDisabled();
  });

  it("adds a folder from the picker", async () => {
    const { api, user } = await openWhereToLook(FOUND);
    const addScanRoot = vi.spyOn(api, "addScanRoot");
    await user.click(screen.getByRole("button", { name: "Add folder…" }));
    expect(addScanRoot).toHaveBeenCalledWith("~/Development/example-new");
    expect(await screen.findByText("~/Development/example-new")).toBeInTheDocument();
  });

  it("adds nothing when the picker is cancelled", async () => {
    const { api, user } = await openWhereToLook({ ...FOUND, pickedFolder: null });
    const addScanRoot = vi.spyOn(api, "addScanRoot");
    await user.click(screen.getByRole("button", { name: "Add folder…" }));
    expect(addScanRoot).not.toHaveBeenCalled();
  });

  it("unskips a clone: it leaves Skipped and is back in Found at once", async () => {
    const { user } = setup({ ...FOUND, skipped: [{ path: "~/Development/web", name: "web", root: "~/Development" }], found: sampleSuggestions() });
    await user.click(await screen.findByRole("button", { name: "Where to look…" }));
    await user.click(screen.getByRole("button", { name: "Unskip" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Skipped" })).not.toBeInTheDocument());
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(within(await screen.findByRole("region", { name: "Found" })).getByText("web")).toBeInTheDocument();
  });

  it("opens from the invitation too", async () => {
    const { user } = setup({ repos: [] });
    await user.click(await screen.findByRole("button", { name: "Where to look…" }));
    expect(screen.getByRole("heading", { name: "Where to look" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(await screen.findByRole("button", { name: "Find clones" })).toBeInTheDocument();
  });
});
