import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { Row, Snapshot, TetherApi } from "./api";
import { createFakeApi, noDiscovery, sampleRows } from "./api.fake";
import { Popover } from "./Popover";

function setup(options: Parameters<typeof createFakeApi>[0] = {}) {
  const api = createFakeApi(options);
  const user = userEvent.setup();
  render(<Popover api={api} />);
  return { api, user };
}

const rowNames = () =>
  within(screen.getByRole("list", { name: "Repositories" }))
    .getAllByRole("listitem")
    .map((li) => li.getAttribute("aria-label"));

describe("Popover", () => {
  it("shows each state as a word, with the CLI's detail as the row's description", async () => {
    setup();
    const api = await screen.findByRole("listitem", { name: "example-api" });
    expect(within(api).getByText("behind")).toBeInTheDocument();
    expect(api).toHaveAccessibleDescription(/3 commit\(s\) to pull on the next sync/);
    expect(
      within(screen.getByRole("listitem", { name: "example-docs" })).getByText("skipped"),
    ).toBeInTheDocument();
    const app = screen.getByRole("listitem", { name: "example-app" });
    expect(within(app).getByText("in sync")).toBeInTheDocument();
    expect(app).toHaveAccessibleDescription("last sync 2 min ago");
  });

  it("shows a needs-you repo as a callout with its reason, not as a list item", async () => {
    setup();
    const callout = await screen.findByRole("region", { name: "example-site needs you" });
    expect(within(callout).getByText("on another branch (feature/new-header)")).toBeInTheDocument();
    expect(within(callout).getByRole("button", { name: "Sync now" })).toBeInTheDocument();
    expect(screen.queryByRole("listitem", { name: "example-site" })).not.toBeInTheDocument();
  });

  it("sorts callouts first and keeps the listed order of the other rows", async () => {
    setup();
    await screen.findByRole("region", { name: "example-site needs you" });
    expect(rowNames()).toEqual(["example-app", "example-api", "example-docs"]);
  });

  it("syncs a needs-you repo from its callout by its listed path", async () => {
    const { api, user } = setup();
    const sync = vi.spyOn(api, "sync");
    const callout = await screen.findByRole("region", { name: "example-site needs you" });
    await user.click(within(callout).getByRole("button", { name: "Sync now" }));
    expect(sync).toHaveBeenCalledWith("~/Development/example-site");
  });

  it("offers to find or add a repository when nothing is listed, with only Pause in the footer", async () => {
    const { user } = setup({ rows: [] });
    expect(
      await screen.findByText(
        "tether keeps each clone you list on its branch. Find the clones on this Mac, or add one by hand.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Find clones" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Sync all" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add repository" }));
    expect(screen.getByRole("form", { name: "Add repository" })).toBeInTheDocument();
  });

  it("shows skeleton rows, not the empty copy, before the first report", () => {
    const api = { ...createFakeApi(), report: () => new Promise<Snapshot>(() => {}) };
    render(<Popover api={api} />);
    expect(document.querySelector('[aria-busy="true"]')).toBeInTheDocument();
    expect(screen.queryByText(/tether keeps each clone/)).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("checking…");
    expect(screen.getByRole("button", { name: "Sync all" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Pause" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Add repository" })).toBeEnabled();
  });

  it("shows the title alone, with no state, when nothing is listed", async () => {
    setup({ rows: [] });
    await screen.findByText(/tether keeps each clone you list on its branch/);
    expect(screen.getByRole("heading", { name: "tether" })).toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(within(screen.getByRole("banner")).queryByText("in sync")).not.toBeInTheDocument();
  });

  describe("when repos.toml cannot be read", () => {
    const broken = (): Snapshot => ({
      report: null,
      error: "line 3: expected a table",
      syncing: false,
      busy: [],
      note: null,
      details: {},
      discovery: noDiscovery(),
    });

    it("shows the error panel, no footer, and Try again asks for the report", async () => {
      const report = vi.fn<TetherApi["report"]>().mockResolvedValue(broken());
      const user = userEvent.setup();
      render(<Popover api={{ ...createFakeApi(), report }} />);
      const panel = await screen.findByRole("alert");
      expect(panel).toHaveTextContent("Cannot read repos.toml");
      expect(panel).toHaveTextContent("line 3: expected a table");
      expect(panel).toHaveTextContent("Fix the file, then try again. Nothing syncs until it reads.");
      expect(screen.getAllByRole("alert")).toHaveLength(1);
      expect(screen.getByRole("status")).toHaveTextContent("needs you");
      expect(screen.queryByRole("button", { name: "Pause" })).not.toBeInTheDocument();
      const fixed = await createFakeApi().report();
      report.mockResolvedValue(fixed);
      await user.click(screen.getByRole("button", { name: "Try again" }));
      expect(report).toHaveBeenCalledTimes(2);
      expect(await screen.findByRole("listitem", { name: "example-app" })).toBeInTheDocument();
    });

    it("keeps the list and shows the error in the alert strip when a report is still held", async () => {
      const stale = { ...(await createFakeApi().report()), error: "line 3: expected a table" };
      render(<Popover api={{ ...createFakeApi(), report: () => Promise.resolve(stale) }} />);
      expect(await screen.findByRole("listitem", { name: "example-app" })).toBeInTheDocument();
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Cannot read repos.toml: line 3: expected a table. Showing the last report.",
      );
      expect(screen.queryByRole("button", { name: "Try again" })).not.toBeInTheDocument();
      expect(screen.getByRole("status")).toHaveTextContent("needs you (1)");
    });
  });

  describe("header summary", () => {
    it("counts the repos while any is not in sync", async () => {
      setup();
      await screen.findByRole("listitem", { name: "example-app" });
      expect(screen.getByText("4 repos")).toBeInTheDocument();
    });

    it("gives the newest last sync when every repo is in sync", async () => {
      const { rows } = sampleRows();
      const inSync = rows.map((r) => ({ ...r, health: "in-sync" as const }));
      setup({ rows: inSync });
      await screen.findByRole("listitem", { name: "example-app" });
      expect(screen.getByText("last sync 2 min ago")).toBeInTheDocument();
      expect(screen.queryByRole("region")).not.toBeInTheDocument();
    });

    it("says paused as the state and keeps the repo count", async () => {
      const { user } = setup();
      await user.click(await screen.findByRole("button", { name: "Pause" }));
      await screen.findByRole("button", { name: "Resume" });
      expect(screen.getByRole("status")).toHaveTextContent(/^paused$/);
      expect(screen.getByText("4 repos")).toBeInTheDocument();
    });

    it("keeps needs you while a sync runs, and counts the busy repos", async () => {
      const { user } = setup();
      await user.click(await screen.findByRole("button", { name: "Sync all" }));
      expect(await screen.findByText("4 of 4")).toBeInTheDocument();
      expect(screen.getByRole("status")).toHaveTextContent("needs you (1)");
      expect(screen.getByRole("button", { name: "Sync all" })).toBeDisabled();
      expect(screen.getByRole("banner").querySelector(".animate-progress")).not.toBeNull();
    });
  });

  it("queues a sync of one repo by its listed path", async () => {
    const { api, user } = setup();
    const sync = vi.spyOn(api, "sync");
    const row = await screen.findByRole("listitem", { name: "example-api" });
    await user.click(within(row).getByRole("button", { name: "Sync now" }));
    expect(sync).toHaveBeenCalledWith("~/Development/example-api");
    expect(within(row).getByRole("button", { name: "Sync now" })).toHaveAttribute(
      "aria-busy",
      "true",
    );
  });

  it("queues a sync of every repo from Sync all", async () => {
    const { api, user } = setup();
    const sync = vi.spyOn(api, "sync");
    await screen.findByRole("listitem", { name: "example-api" });
    await user.click(screen.getByRole("button", { name: "Sync all" }));
    expect(sync).toHaveBeenCalledWith(null);
  });

  it("opens a repo in Finder by its listed path", async () => {
    const { api, user } = setup();
    const open = vi.spyOn(api, "openInFinder");
    const row = await screen.findByRole("listitem", { name: "example-docs" });
    await user.click(within(row).getByRole("button", { name: "Open in Finder" }));
    expect(open).toHaveBeenCalledWith("~/Mirrors/example-docs");
  });

  it("keeps the needs-you callout while paused", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Pause" }));
    await screen.findByRole("button", { name: "Resume" });
    expect(screen.getByRole("region", { name: "example-site needs you" })).toBeInTheDocument();
  });

  it("toggles Pause and offers Resume", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Pause" }));
    expect(await screen.findByRole("button", { name: "Resume" })).toBeInTheDocument();
  });

  it("hides the window on Escape", async () => {
    const { api, user } = setup();
    const hide = vi.spyOn(api, "hide");
    await screen.findByRole("listitem", { name: "example-app" });
    await user.keyboard("{Escape}");
    expect(hide).toHaveBeenCalled();
  });

  it("shows a sync failure the app reported", async () => {
    const { api, user } = setup();
    vi.spyOn(api, "sync").mockRejectedValue("repos.toml: duplicate path");
    await user.click(await screen.findByRole("button", { name: "Sync all" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("repos.toml: duplicate path");
  });
});

describe("Dismissing a callout", () => {
  const SITE = "~/Development/example-site";

  it("lists the repo as a needs-you row, and the header still counts it", async () => {
    const { user } = setup();
    const callout = await screen.findByRole("region", { name: "example-site needs you" });
    await user.click(within(callout).getByRole("button", { name: "Dismiss until it changes" }));
    expect(screen.queryByRole("region")).not.toBeInTheDocument();
    expect(rowNames()).toEqual(["example-site", "example-app", "example-api", "example-docs"]);
    expect(within(screen.getByRole("listitem", { name: "example-site" })).getByText("needs you")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("needs you (1)");
  });

  it("survives a restart", async () => {
    const { user } = setup();
    const callout = await screen.findByRole("region", { name: "example-site needs you" });
    await user.click(within(callout).getByRole("button", { name: "Dismiss until it changes" }));
    cleanup();
    setup();
    expect(await screen.findByRole("listitem", { name: "example-site" })).toBeInTheDocument();
    expect(screen.queryByRole("region")).not.toBeInTheDocument();
  });

  it("brings the callout back when the reason changes", async () => {
    const { rows, details } = sampleRows();
    localStorage.setItem("tether.dismissed", JSON.stringify({ [SITE]: details[SITE] }));
    setup({ rows, details: { ...details, [SITE]: "HEAD is detached" } });
    const callout = await screen.findByRole("region", { name: "example-site needs you" });
    expect(callout).toHaveTextContent("HEAD is detached");
  });

  it("folds three needs-you repos behind the first", async () => {
    const { rows, details } = sampleRows();
    const needing = (name: string): Row => ({
      ...rows[3]!,
      name,
      path: `~/Development/${name}`,
    });
    setup({ rows: [...rows, needing("example-handbook"), needing("example-cli")], details });
    await screen.findByRole("region", { name: "example-site needs you" });
    expect(screen.getAllByRole("region")).toHaveLength(1);
    expect(screen.getByText("2 more need you")).toBeInTheDocument();
  });
});

describe("Add repository", () => {
  it("adds the chosen folder with its checked-out branch and the chosen mode", async () => {
    const { api, user } = setup({ pickedFolder: "~/Development/example-new" });
    const add = vi.spyOn(api, "add");
    vi.spyOn(api, "branchOf").mockResolvedValue("develop");
    await user.click(await screen.findByRole("button", { name: "Add repository" }));
    await user.click(screen.getByRole("button", { name: "Choose…" }));
    const branch = screen.getByRole("textbox", { name: "Branch" });
    await waitFor(() => expect(branch).toHaveValue("develop"));
    await user.click(screen.getByRole("radio", { name: /mirror/ }));
    await user.type(screen.getByRole("textbox", { name: "Clone URL" }), "https://example.com/r.git");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(add).toHaveBeenCalledWith({
      path: "~/Development/example-new",
      branch: "develop",
      mode: "mirror",
      url: "https://example.com/r.git",
      interval: null,
      gate: null,
      after: null,
    });
    expect(await screen.findByRole("listitem", { name: "example-new" })).toBeInTheDocument();
  });

  it("defaults to follow and sends no URL", async () => {
    const { api, user } = setup();
    const add = vi.spyOn(api, "add");
    await user.click(await screen.findByRole("button", { name: "Add repository" }));
    expect(screen.queryByRole("radio")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Choose…" }));
    await waitFor(() => expect(screen.getByRole("textbox", { name: "Branch" })).toHaveValue("main"));
    expect(screen.getByRole("radio", { name: /follow/ })).toBeChecked();
    expect(screen.queryByRole("textbox", { name: "Clone URL" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(add).toHaveBeenCalledWith({
      path: "~/Development/example-new",
      branch: "main",
      mode: "follow",
      url: null,
      interval: null,
      gate: null,
      after: null,
    });
  });

  it("shows why the app refused, and stays in the form", async () => {
    const { api, user } = setup();
    vi.spyOn(api, "add").mockRejectedValue("origin has no branch main");
    await user.click(await screen.findByRole("button", { name: "Add repository" }));
    await user.click(screen.getByRole("button", { name: "Choose…" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Add" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("origin has no branch main");
    expect(screen.getByRole("form", { name: "Add repository" })).toBeInTheDocument();
  });

  it("cannot submit before a folder is chosen", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Add repository" }));
    expect(screen.getByRole("button", { name: "Add" })).toBeDisabled();
  });

  it("runs the header hairline while adding", async () => {
    const { api, user } = setup();
    vi.spyOn(api, "add").mockReturnValue(new Promise(() => {}));
    await user.click(await screen.findByRole("button", { name: "Add repository" }));
    await user.click(screen.getByRole("button", { name: "Choose…" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Add" })).toBeEnabled());
    expect(screen.getByRole("banner").querySelector(".animate-progress")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByRole("banner").querySelector(".animate-progress")).not.toBeNull();
  });

  it("returns to the list on Cancel", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Add repository" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("list", { name: "Repositories" })).toBeInTheDocument();
  });
});

describe("Repo settings", () => {
  const openSettings = async (user: ReturnType<typeof userEvent.setup>, name: string) => {
    const row = await screen.findByRole("listitem", { name });
    await user.click(within(row).getByRole("button", { name: "Settings" }));
    return screen.getByRole("form", { name: `${name} settings` });
  };

  it("opens from a row, titled with the repo's name and its state", async () => {
    const { user } = setup();
    const form = await openSettings(user, "example-api");
    expect(screen.getByRole("heading", { name: "example-api" })).toBeInTheDocument();
    expect(screen.getByRole("banner")).toHaveTextContent("behind · 3");
    expect(form).toHaveTextContent("~/Development/example-api");
    expect(screen.getByRole("combobox", { name: "Pull every" })).toHaveTextContent("5 min");
  });

  it("opens from a needs-you callout", async () => {
    const { user } = setup();
    const callout = await screen.findByRole("region", { name: "example-site needs you" });
    await user.click(within(callout).getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("form", { name: "example-site settings" })).toBeInTheDocument();
  });

  it("saves a new interval by the listed path and returns to the list", async () => {
    const { api, user } = setup();
    const setInterval = vi.spyOn(api, "setInterval");
    await openSettings(user, "example-api");
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    await user.click(screen.getByRole("combobox", { name: "Pull every" }));
    await user.click(await screen.findByRole("option", { name: "1 hour" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(setInterval).toHaveBeenCalledWith("~/Development/example-api", "1h");
    expect(await screen.findByRole("list", { name: "Repositories" })).toBeInTheDocument();
    await openSettings(user, "example-api");
    expect(screen.getByRole("combobox", { name: "Pull every" })).toHaveTextContent("1 hour");
  });

  it("removes only after the confirm, and the repo leaves the list", async () => {
    const { api, user } = setup();
    const remove = vi.spyOn(api, "remove");
    await openSettings(user, "example-api");
    await user.click(screen.getByRole("button", { name: "Remove from tether…" }));
    expect(remove).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Remove" }));
    expect(remove).toHaveBeenCalledWith("~/Development/example-api");
    await screen.findByRole("list", { name: "Repositories" });
    expect(rowNames()).not.toContain("example-api");
  });

  it("backs out of the remove with Keep", async () => {
    const { api, user } = setup();
    const remove = vi.spyOn(api, "remove");
    await openSettings(user, "example-api");
    await user.click(screen.getByRole("button", { name: "Remove from tether…" }));
    await user.click(screen.getByRole("button", { name: "Keep" }));
    expect(screen.getByRole("button", { name: "Remove from tether…" })).toBeInTheDocument();
    expect(remove).not.toHaveBeenCalled();
  });

  it("shows why the app refused, and stays in the form", async () => {
    const { api, user } = setup();
    vi.spyOn(api, "remove").mockRejectedValue("~/Development/example-api is no longer listed");
    await openSettings(user, "example-api");
    await user.click(screen.getByRole("button", { name: "Remove from tether…" }));
    await user.click(screen.getByRole("button", { name: "Remove" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("is no longer listed");
    expect(screen.getByRole("form", { name: "example-api settings" })).toBeInTheDocument();
  });

  it("returns to the list on Cancel without saving", async () => {
    const { api, user } = setup();
    const setInterval = vi.spyOn(api, "setInterval");
    await openSettings(user, "example-api");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("list", { name: "Repositories" })).toBeInTheDocument();
    expect(setInterval).not.toHaveBeenCalled();
  });
});

describe("Add repository's interval", () => {
  it("sends the chosen interval", async () => {
    const { api, user } = setup();
    const add = vi.spyOn(api, "add");
    await user.click(await screen.findByRole("button", { name: "Add repository" }));
    await user.click(screen.getByRole("button", { name: "Choose…" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Add" })).toBeEnabled());
    await user.click(screen.getByRole("combobox", { name: "Pull every" }));
    await user.click(await screen.findByRole("option", { name: "15 min" }));
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(add).toHaveBeenCalledWith(expect.objectContaining({ interval: "15m" }));
  });
});

describe("createFakeApi", () => {
  it("keeps a second subscription of the same handler when the first ends", async () => {
    const api = createFakeApi();
    const handler = vi.fn();
    const stopFirst = await api.onReport(handler);
    await api.onReport(handler);
    stopFirst();
    await api.togglePause();
    expect(handler).toHaveBeenCalledTimes(1);
  });
});

describe("sampleRows", () => {
  it("covers every state", () => {
    const states = new Set(sampleRows().rows.map((r) => r.health));
    expect(states).toEqual(new Set(["in-sync", "behind", "skipped", "needs-you"]));
  });
});
