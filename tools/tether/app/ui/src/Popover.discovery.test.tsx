import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { createFakeApi, type FakeOptions } from "./api.fake";
import { SCENARIOS } from "./api.scenarios";
import { Popover } from "./Popover";

function setup(scenario: string, options: FakeOptions = {}) {
  const api = createFakeApi({ ...SCENARIOS[scenario], scanStep: 150, ...options });
  const user = userEvent.setup();
  render(<Popover api={api} />);
  return { api, user };
}

const found = () => screen.getByRole("region", { name: "Found" });
const foundRows = () => within(found()).queryAllByRole("listitem");
const rowNamed = (name: string) => screen.getByRole("button", { name: `Skip ${name}` }).closest("li")!;
const hairline = () => screen.getByRole("banner").querySelector(".animate-progress");

/** The popover shown again, as the tray does after hiding it. */
function showAgain() {
  const state = vi.spyOn(document, "visibilityState", "get");
  state.mockReturnValue("hidden");
  document.dispatchEvent(new Event("visibilitychange"));
  state.mockReturnValue("visible");
  act(() => document.dispatchEvent(new Event("visibilitychange")));
  state.mockRestore();
}

describe("nothing listed, no scan yet", () => {
  it("invites, with only Pause in the footer and no count in the header", async () => {
    setup("invite");
    expect(await screen.findByRole("button", { name: "Find clones" })).toBeInTheDocument();
    expect(screen.getByText(/^Looks in ~\/Development and ~\/Code\./)).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Found" })).not.toBeInTheDocument();
    expect(screen.queryByText(/found/)).not.toBeInTheDocument();
    expect(screen.getByRole("contentinfo")).toHaveTextContent(/^Pause$/);
  });

  it("starts a scan from Find clones, and Found fills in as each root is walked", async () => {
    const { api, user } = setup("invite");
    const find = vi.spyOn(api, "findClones");
    await user.click(await screen.findByRole("button", { name: "Find clones" }));
    expect(find).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("looking in ~/Development…")).toBeInTheDocument();
    expect(hairline()).not.toBeNull();
    expect(screen.getByText("You can add these now; more appear as the search goes.")).toBeInTheDocument();
    await waitFor(() => expect(foundRows()).toHaveLength(3));
    await waitFor(() => expect(foundRows()).toHaveLength(5));
    await waitFor(() => expect(hairline()).toBeNull());
    expect(screen.queryByText(/looking in/)).not.toBeInTheDocument();
    expect(screen.getByRole("banner")).toHaveTextContent("5 found");
    expect(found()).toHaveTextContent("In ~/Development and ~/Code, just now. Add the ones to keep on their branch.");
  });

  it("keeps Add repository at the right of the footer, quiet, beside Pause", async () => {
    setup("found");
    await screen.findByRole("region", { name: "Found" });
    const footer = screen.getByRole("contentinfo");
    expect(within(footer).getByRole("button", { name: "Pause" })).toBeInTheDocument();
    expect(within(footer).getByRole("button", { name: "Add repository" })).toHaveClass("ml-auto");
    expect(within(footer).getByRole("button", { name: "Add repository" })).not.toHaveClass("bg-accent");
    expect(within(footer).queryByRole("button", { name: "Sync all" })).not.toBeInTheDocument();
  });
});

describe("Found as the body", () => {
  it("shows the count, Where to look…, and a row per suggestion with the branch and root", async () => {
    setup("found");
    await screen.findByRole("region", { name: "Found" });
    expect(screen.getByRole("banner")).toHaveTextContent("6 found");
    expect(within(found()).getByRole("heading", { name: /Found\s*6/ })).toBeInTheDocument();
    expect(within(found()).getByRole("button", { name: "Where to look…" })).toBeInTheDocument();
    expect(foundRows()).toHaveLength(6);
    const search = rowNamed("search");
    expect(search).toHaveTextContent("develop · ~/Development");
    expect(within(search).getByText("search")).toHaveAttribute("title", "~/Development/search");
  });

  it("shows partial results with the root being walked and placeholder rows", async () => {
    setup("looking");
    expect(await screen.findByText("looking in ~/Code…")).toBeInTheDocument();
    expect(screen.getByRole("banner")).toHaveTextContent("2 found");
    expect(hairline()).not.toBeNull();
    expect(foundRows()).toHaveLength(2);
    expect(found().querySelectorAll("ul[aria-hidden='true'] li")).toHaveLength(3);
    expect(within(found()).queryByRole("button", { name: "Where to look…" })).not.toBeInTheDocument();
  });

  it("lets a row be answered before the scan ends", async () => {
    const { api, user } = setup("looking");
    const add = vi.spyOn(api, "addSuggestion");
    await user.click(await screen.findByRole("button", { name: "Skip cache" }));
    expect(foundRows()).toHaveLength(1);
    await user.click(within(rowNamed("queue")).getByRole("button", { name: "Add" }));
    expect(add).toHaveBeenCalledWith("~/Development/queue");
  });

  it("says which folder could not be read, and shows the rest", async () => {
    setup("found-error");
    await screen.findByRole("region", { name: "Found" });
    expect(found()).toHaveTextContent("Cannot read ~/work: permission denied. ~/Development and ~/Code were searched.");
    expect(found()).not.toHaveTextContent("Add the ones to keep");
    expect(foundRows()).toHaveLength(2);
    expect(screen.getByRole("banner")).toHaveTextContent("2 found");
  });

  it("truncates long names and keeps the path as the tooltip", async () => {
    setup("found-long-names");
    await screen.findByRole("region", { name: "Found" });
    const name = screen.getByText("example-monorepo-with-a-really-long-name");
    expect(name).toHaveClass("truncate");
    expect(name).toHaveAttribute("title", "~/Development/example-monorepo-with-a-really-long-name");
  });
});

describe("Add", () => {
  it("lists the clone as a syncing row and removes the suggestion", async () => {
    const { api, user } = setup("found");
    const add = vi.spyOn(api, "addSuggestion");
    await screen.findByText("queue");
    await user.click(within(rowNamed("queue")).getByRole("button", { name: "Add" }));
    expect(add).toHaveBeenCalledWith("~/Development/queue");
    const row = await screen.findByRole("listitem", { name: "queue" });
    expect(within(row).getByText("syncing…")).toBeInTheDocument();
    expect(foundRows()).toHaveLength(5);
    expect(screen.getByRole("list", { name: "Repositories" })).toContainElement(row);
    // The section stays open under the list while this session works through it.
    expect(found().querySelector("h2")).toHaveTextContent(/Found\s*5/);
  });

  it("disables Add on the row while it is in flight, and the row stays", async () => {
    const { user } = setup("add-pending");
    await screen.findByText("cache");
    await user.click(within(rowNamed("cache")).getByRole("button", { name: "Add" }));
    const row = rowNamed("cache");
    expect(within(row).getByRole("button", { name: "Add" })).toBeDisabled();
    expect(within(row).getByRole("button", { name: "Add" })).toHaveAttribute("aria-busy", "true");
    expect(hairline()).not.toBeNull();
    expect(foundRows()).toHaveLength(6);
  });

  it("shows add's own error verbatim in the notice strip, and the row stays", async () => {
    const { user } = setup("add-fails");
    await screen.findByText("search");
    await user.click(within(rowNamed("search")).getByRole("button", { name: "Add" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "~/Development/search: origin has no branch develop",
    );
    expect(foundRows()).toHaveLength(6);
    expect(within(rowNamed("search")).getByRole("button", { name: "Add" })).toBeEnabled();
    expect(hairline()).toBeNull();
  });
});

describe("Skip", () => {
  it("skips by path and the row leaves at once", async () => {
    const { api, user } = setup("found");
    const skip = vi.spyOn(api, "skipClone");
    await user.click(await screen.findByRole("button", { name: "Skip wiki" }));
    expect(skip).toHaveBeenCalledWith("~/Code/wiki");
    expect(screen.queryByRole("button", { name: "Skip wiki" })).not.toBeInTheDocument();
    expect(screen.getByRole("banner")).toHaveTextContent("5 found");
  });

  it("with nothing listed, becomes the nothing-found state once the last one is skipped", async () => {
    const { user } = setup("found", {
      discovery: { ...SCENARIOS["found"]!.discovery!, suggestions: [SCENARIOS["found"]!.discovery!.suggestions![0]!] },
    });
    await user.click(await screen.findByRole("button", { name: "Skip cache" }));
    expect(await screen.findByText(/^No clones in/)).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Found" })).not.toBeInTheDocument();
  });
});

describe("nothing found", () => {
  it("says where it looked and that one root stopped early", async () => {
    setup("found-nothing");
    expect(
      await screen.findByText("No clones in ~/Development or ~/Code. Look somewhere else, or add one by hand."),
    ).toBeInTheDocument();
    expect(screen.getByText("Looked just now · stopped early in ~/Code after 20 000 folders.")).toBeInTheDocument();
    expect(screen.getByRole("contentinfo")).toHaveTextContent(/^Pause$/);
    expect(screen.queryByText(/found/)).not.toBeInTheDocument();
  });
});

describe("Found under the listed repos", () => {
  it("is absent before the first scan", async () => {
    setup("regular");
    await screen.findByRole("list", { name: "Repositories" });
    expect(screen.queryByRole("region", { name: "Found" })).not.toBeInTheDocument();
  });

  it("is one collapsed row when suggestions remain, and opens on the chevron", async () => {
    const { user } = setup("later");
    await screen.findByRole("list", { name: "Repositories" });
    const toggle = screen.getByRole("button", { name: /^Found\s*2/ });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(toggle).toHaveTextContent("· looked 3 days ago");
    expect(screen.getByRole("button", { name: "Look again" })).toBeInTheDocument();
    expect(foundRows()).toHaveLength(0);
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(foundRows()).toHaveLength(2);
    // No count in the header's aside: that is the list's own summary.
    expect(screen.getByRole("banner")).not.toHaveTextContent(/found/);
  });

  it("settles once a scan found nothing new", async () => {
    setup("looked-again");
    await screen.findByRole("list", { name: "Repositories" });
    expect(found()).toHaveTextContent("Nothing new in ~/Development, ~/Code · looked just now");
    expect(within(found()).getByRole("button", { name: "Look again" })).toBeInTheDocument();
  });

  it("settles on No suggestions when the scan found no clone at all", async () => {
    setup("no-suggestions");
    await screen.findByRole("list", { name: "Repositories" });
    expect(found()).toHaveTextContent("No suggestions · looked just now");
  });

  it("runs a scan from Look again, open while it runs, with the rule above", async () => {
    const { api, user } = setup("later");
    const find = vi.spyOn(api, "findClones");
    await user.click(await screen.findByRole("button", { name: "Look again" }));
    expect(find).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("looking in ~/Development…")).toBeInTheDocument();
    expect(found().querySelector("h2")!.parentElement).toHaveClass("border-border");
    await waitFor(() => expect(hairline()).toBeNull());
    expect(foundRows()).toHaveLength(5);
  });

  it("stays open once this session answered a suggestion, and settles when the last is answered", async () => {
    const { user } = setup("later");
    await user.click(await screen.findByRole("button", { name: /^Found\s*2/ }));
    await user.click(screen.getByRole("button", { name: "Skip notes" }));
    expect(foundRows()).toHaveLength(1);
    expect(found().querySelector("h2")).toHaveTextContent(/Found\s*1/);
    await user.click(screen.getByRole("button", { name: "Skip wiki" }));
    expect(found()).toHaveTextContent(/^Nothing new in ~\/Code · looked 3 days ago/);
    expect(foundRows()).toHaveLength(0);
  });

  it("folds back to one row when the popover is shown again", async () => {
    const { user } = setup("later");
    await user.click(await screen.findByRole("button", { name: /^Found\s*2/ }));
    await user.click(screen.getByRole("button", { name: "Skip notes" }));
    expect(found().querySelector("h2")).toBeInTheDocument();
    showAgain();
    expect(await screen.findByRole("button", { name: /^Found\s*1/ })).toHaveAttribute("aria-expanded", "false");
  });

  it("leaves the footer as it is for a list", async () => {
    setup("later");
    await screen.findByRole("list", { name: "Repositories" });
    const footer = screen.getByRole("contentinfo");
    expect(within(footer).getByRole("button", { name: "Sync all" })).toBeInTheDocument();
    expect(within(footer).getByRole("button", { name: "Add repository" })).toHaveClass("bg-accent");
  });
});

describe("Where to look", () => {
  const open = async (user: ReturnType<typeof userEvent.setup>) =>
    user.click(await screen.findByRole("button", { name: "Where to look…" }));

  it("opens from the invitation, titled for the view with no state, and Done returns", async () => {
    const { user } = setup("invite");
    await open(user);
    expect(screen.getByRole("heading", { name: "Where to look", level: 1 })).toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(screen.getByText("~/Development")).toBeInTheDocument();
    expect(
      screen.getByText("Desktop, Documents and Downloads are searched only if you add them here. macOS asks you first."),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.getByRole("heading", { name: "tether", level: 1 })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Find clones" })).toBeInTheDocument();
  });

  it("opens from the Found heading and from the nothing-found state", async () => {
    const found = setup("found");
    await open(found.user);
    expect(screen.getByRole("heading", { name: "Where to look", level: 1 })).toBeInTheDocument();
    await found.user.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.getByRole("region", { name: "Found" })).toBeInTheDocument();
  });

  it("opens from the nothing-found state", async () => {
    const { user } = setup("found-nothing");
    await open(user);
    expect(screen.getByRole("heading", { name: "Where to look", level: 1 })).toBeInTheDocument();
  });

  it("counts the clones in each folder, and shows an error in its place", async () => {
    const { user } = setup("found-error");
    await open(user);
    const folders = screen.getByRole("list", { name: "Folders" });
    expect(within(folders).getByText("~/Development").closest("li")).toHaveTextContent("2 clones");
    expect(within(folders).getByText("~/Code").closest("li")).toHaveTextContent("0 clones");
    expect(within(folders).getByText("~/work").closest("li")).toHaveTextContent("permission denied");
  });

  it("says 1 clone in the singular", async () => {
    const { user } = setup("found", {
      discovery: { ...SCENARIOS["found"]!.discovery!, roots: [{ path: "~/Code", clones: 1, stopped_early: false, error: null }] },
    });
    await open(user);
    expect(screen.getByText("1 clone")).toBeInTheDocument();
  });

  it("stops looking in a folder", async () => {
    const { api, user } = setup("where-to-look");
    const remove = vi.spyOn(api, "removeScanRoot");
    await open(user);
    await user.click(screen.getByRole("button", { name: "Stop looking in ~/Code" }));
    expect(remove).toHaveBeenCalledWith("~/Code");
    const folders = screen.getByRole("list", { name: "Folders" });
    await waitFor(() => expect(within(folders).queryByText("~/Code")).not.toBeInTheDocument());
  });

  it("adds a folder from the picker", async () => {
    const { api, user } = setup("where-to-look", { pickedFolder: "~/Projects" });
    const add = vi.spyOn(api, "addScanRoot");
    await open(user);
    await user.click(screen.getByRole("button", { name: "Add folder…" }));
    expect(add).toHaveBeenCalledWith("~/Projects");
    expect(await screen.findByText("~/Projects")).toBeInTheDocument();
  });

  it("adds nothing when the picker is cancelled", async () => {
    const { api, user } = setup("where-to-look", { pickedFolder: null });
    const add = vi.spyOn(api, "addScanRoot");
    await open(user);
    await user.click(screen.getByRole("button", { name: "Add folder…" }));
    expect(add).not.toHaveBeenCalled();
  });

  it("lists the skipped clones and unskips one, which comes back to Found", async () => {
    const { api, user } = setup("where-to-look");
    const unskip = vi.spyOn(api, "unskipClone");
    await open(user);
    const skipped = screen.getByRole("list", { name: /Skipped/ });
    expect(within(skipped).getAllByRole("listitem")).toHaveLength(2);
    await user.click(within(skipped).getAllByRole("button", { name: "Unskip" })[0]!);
    expect(unskip).toHaveBeenCalledWith("~/Development/scratch");
    await waitFor(() => expect(within(screen.getByRole("list", { name: /Skipped/ })).getAllByRole("listitem")).toHaveLength(1));
  });

  it("starts a scan from Look again and returns to the list", async () => {
    const { api, user } = setup("later");
    const find = vi.spyOn(api, "findClones");
    await user.click(await screen.findByRole("button", { name: "Look again" }));
    await waitFor(() => expect(hairline()).toBeNull());
    await user.click(screen.getByRole("button", { name: "Where to look…" }));
    find.mockClear();
    await user.click(screen.getByRole("button", { name: "Look again" }));
    expect(find).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("heading", { name: "tether", level: 1 })).toBeInTheDocument();
  });

  it("disables Look again when no folder is left", async () => {
    const { user } = setup("where-to-look-no-roots");
    await open(user);
    expect(screen.getByRole("button", { name: "Look again" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Done" })).toBeEnabled();
  });
});
