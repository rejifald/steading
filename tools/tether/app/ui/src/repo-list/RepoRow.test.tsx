import { Tooltip } from "@base-ui/react/tooltip";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { Row } from "../api";
import { RepoRow } from "./RepoRow";

const NOW = new Date("2026-10-29T12:00:00Z");

const ROW: Row = {
  name: "example-api",
  path: "~/code/example-api",
  branch: "main",
  mode: "follow",
  remote: "origin",
  interval_seconds: 300,
  last_sync: "2026-10-29T11:56:00Z",
  health: "behind",
};

function setup(overrides: { detail?: string; busy?: boolean; paused?: boolean; row?: Row } = {}) {
  const onSync = vi.fn();
  const onOpen = vi.fn();
  const onSettings = vi.fn();
  render(
    <Tooltip.Provider>
      <ul>
        <RepoRow
          row={overrides.row ?? ROW}
          detail={overrides.detail}
          busy={overrides.busy ?? false}
          paused={overrides.paused ?? false}
          now={NOW}
          onSync={onSync}
          onOpen={onOpen}
          onSettings={onSettings}
        />
      </ul>
    </Tooltip.Provider>,
  );
  const name = (overrides.row ?? ROW).name;
  return { onSync, onOpen, onSettings, row: screen.getByRole("listitem", { name }) };
}

describe("RepoRow", () => {
  it("shows name, branch and state on one line", () => {
    const { row } = setup();
    expect(row).toHaveTextContent("example-api");
    expect(row).toHaveTextContent("main");
    expect(row).toHaveTextContent("behind");
    expect(row).not.toHaveTextContent("last sync");
  });

  it("describes the row with the detail and the last sync", () => {
    const { row } = setup({ detail: "3 commit(s) to pull on the next sync" });
    expect(row).toHaveAttribute("aria-description", "3 commit(s) to pull on the next sync · last sync 4 min ago");
  });

  it("describes the row with the last sync alone when there is no detail", () => {
    const { row } = setup();
    expect(row).toHaveAttribute("aria-description", "last sync 4 min ago");
  });

  it("opens its detail while focus is inside the row", async () => {
    setup({ detail: "3 commit(s) to pull on the next sync" });
    await userEvent.tab();
    expect(await screen.findByText("3 commit(s) to pull on the next sync · last sync 4 min ago")).toBeInTheDocument();
    await userEvent.tab({ shift: true });
    await waitFor(() =>
      expect(screen.queryByText("3 commit(s) to pull on the next sync · last sync 4 min ago")).not.toBeInTheDocument(),
    );
  });

  it("keeps the hidden actions in the tab order", async () => {
    setup();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Sync now" })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Open in Finder" })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Settings" })).toHaveFocus();
  });

  it("syncs the row's path from Sync now", async () => {
    const { onSync } = setup();
    await userEvent.click(screen.getByRole("button", { name: "Sync now" }));
    expect(onSync).toHaveBeenCalledWith(ROW.path);
  });

  it("opens the row's path from Open in Finder", async () => {
    const { onOpen } = setup();
    await userEvent.click(screen.getByRole("button", { name: "Open in Finder" }));
    expect(onOpen).toHaveBeenCalledWith(ROW.path);
  });

  it("opens the row's settings by its path", async () => {
    const { onSettings } = setup();
    await userEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(onSettings).toHaveBeenCalledWith(ROW.path);
  });

  it("marks Sync now busy while syncing", () => {
    setup({ busy: true });
    expect(screen.getByRole("button", { name: "Sync now" })).toHaveAttribute("aria-busy", "true");
  });

  it("washes the row and runs a hairline under it while syncing", () => {
    const { row } = setup({ busy: true });
    expect(row).toHaveClass("bg-behind/10");
    expect(row.querySelector("[aria-hidden='true'] .animate-progress")).not.toBeNull();
    expect(row).toHaveTextContent("syncing…");
  });

  it("draws no hairline when idle", () => {
    const { row } = setup();
    expect(row).not.toHaveClass("bg-behind/10");
    expect(row.querySelector(".animate-progress")).toBeNull();
  });

  it("leads with the pause glyph while paused", () => {
    const { row } = setup({ paused: true });
    expect(row.querySelector("svg")).toHaveClass("text-faint");
    expect(row).toHaveTextContent("behind");
  });

  it("lists a dismissed needs-you repo with its reason as the description", () => {
    const { row } = setup({
      row: { ...ROW, name: "example-site", health: "needs-you" },
      detail: "on another branch (feature/new-header)",
    });
    expect(row).toHaveTextContent("needs you");
    expect(row).toHaveAttribute(
      "aria-description",
      "on another branch (feature/new-header) · last sync 4 min ago",
    );
  });
});
