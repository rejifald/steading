import { Tooltip } from "@base-ui/react/tooltip";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { Row } from "../api";
import { NeedsYouCallout } from "./NeedsYouCallout";

const row: Row = {
  name: "example-site",
  path: "~/code/example-site",
  branch: "release",
  mode: "follow",
  remote: "origin",
  interval_seconds: 300,
  last_sync: "2026-10-29T09:00:00Z",
  health: "needs-you",
};
const now = new Date("2026-10-29T10:00:00Z");
const reason = "on another branch (feature/new-header)";

function setup(busy = false, shown: Row = row) {
  const onSync = vi.fn();
  const onOpen = vi.fn();
  const onSettings = vi.fn();
  const onDismiss = vi.fn();
  render(
    <Tooltip.Provider>
      <NeedsYouCallout
        row={shown}
        detail={reason}
        busy={busy}
        now={now}
        onSync={onSync}
        onOpen={onOpen}
        onSettings={onSettings}
        onDismiss={onDismiss}
      />
    </Tooltip.Provider>,
  );
  return { onSync, onOpen, onSettings, onDismiss, user: userEvent.setup() };
}

describe("NeedsYouCallout", () => {
  it("renders the reason, the branch and mode, and the age", () => {
    setup();
    const section = screen.getByRole("region", { name: "example-site needs you" });
    expect(section).toBeInTheDocument();
    expect(screen.getByText(reason)).toBeInTheDocument();
    expect(screen.getByText("release · follow")).toBeInTheDocument();
    expect(screen.getByText("1 h ago")).toBeInTheDocument();
  });

  it("says what to do about the reason", () => {
    setup(false, { ...row, hint: "Switch back to release and tether resumes." });
    expect(screen.getByText("Switch back to release and tether resumes.")).toBeInTheDocument();
  });

  it("leads with Open in Finder when a sync cannot clear the reason", () => {
    setup();
    const labels = screen.getAllByRole("button").map((button) => button.textContent).filter(Boolean);
    expect(labels).toEqual(["Open in Finder", "Sync now"]);
  });

  it("leads with Sync now when only a sync can clear the reason", () => {
    setup(false, { ...row, reason: { kind: "fetch-failed" } });
    const labels = screen.getAllByRole("button").map((button) => button.textContent).filter(Boolean);
    expect(labels).toEqual(["Sync now", "Open in Finder"]);
  });

  it("calls onSync and onOpen with the listed path", async () => {
    const { onSync, onOpen, user } = setup();
    await user.click(screen.getByRole("button", { name: "Sync now" }));
    await user.click(screen.getByRole("button", { name: "Open in Finder" }));
    expect(onSync).toHaveBeenCalledWith(row.path);
    expect(onOpen).toHaveBeenCalledWith(row.path);
  });

  it("opens the repo's settings by its listed path", async () => {
    const { onSettings, user } = setup();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(onSettings).toHaveBeenCalledWith(row.path);
  });

  it("dismisses by its listed path from the ×", async () => {
    const { onDismiss, user } = setup();
    await user.click(screen.getByRole("button", { name: "Dismiss until it changes" }));
    expect(onDismiss).toHaveBeenCalledWith(row.path);
  });

  it("tints its top strip and raises the box", () => {
    setup();
    const section = screen.getByRole("region", { name: "example-site needs you" });
    expect(section).toHaveClass("shadow-raised");
    expect(screen.getByText("1 h ago").parentElement).toHaveClass("bg-needs-you/14");
  });

  it("reads Syncing… and is disabled while busy", async () => {
    const { onSync, user } = setup(true);
    const button = screen.getByRole("button", { name: "Syncing…" });
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("aria-busy", "true");
    await user.click(button);
    expect(onSync).not.toHaveBeenCalled();
  });
});
