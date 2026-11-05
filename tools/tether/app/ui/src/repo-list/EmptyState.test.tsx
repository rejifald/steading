import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { Discovery, RootInfo } from "../api";
import { noDiscovery } from "../api.fake";
import { EmptyState } from "./EmptyState";

const NOW = new Date("2026-11-04T12:00:00Z");

const handlers = () => ({ onAdd: vi.fn(), onFind: vi.fn(), onWhereToLook: vi.fn() });

function invite(discovery: Discovery = noDiscovery(), h = handlers()) {
  render(<EmptyState variant="invite" discovery={discovery} now={NOW} {...h} />);
  return h;
}

/** A finished scan that found nothing, over the given roots. */
const looked = (roots: Partial<RootInfo>[], scannedAt = NOW.toISOString()): Discovery => ({
  ...noDiscovery(),
  scanned_at: scannedAt,
  roots: roots.map((root, i) => ({ path: `~/root-${i}`, clones: 0, stopped_early: false, error: null, ...root })),
});

describe("EmptyState invite", () => {
  it("says what tether does and offers to find the clones", () => {
    invite();
    expect(
      screen.getByText(
        "tether keeps each clone you list on its branch. Find the clones on this Mac, or add one by hand.",
      ),
    ).toBeInTheDocument();
  });

  it("draws the example box with its Add pills, hidden from assistive technology", () => {
    invite();
    const box = screen.getByText("your-app").closest("[aria-hidden='true']");
    expect(box).not.toBeNull();
    expect(box).toHaveTextContent("your-appmain · ~/DevelopmentAddyour-docsmain · ~/CodeAdd");
    expect(screen.queryByRole("button", { name: "Add" })).not.toBeInTheDocument();
  });

  it("calls the right handler from each button", async () => {
    const h = invite();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Add repository" }));
    await user.click(screen.getByRole("button", { name: "Find clones" }));
    await user.click(screen.getByRole("button", { name: "Where to look…" }));
    expect(h.onAdd).toHaveBeenCalledTimes(1);
    expect(h.onFind).toHaveBeenCalledTimes(1);
    expect(h.onWhereToLook).toHaveBeenCalledTimes(1);
  });

  it("makes Find clones primary, Add repository quiet and Where to look… an underlined link", () => {
    invite();
    expect(screen.getByRole("button", { name: "Find clones" })).toHaveClass("bg-accent");
    expect(screen.getByRole("button", { name: "Add repository" })).not.toHaveClass("bg-accent");
    expect(screen.getByRole("button", { name: "Where to look…" })).toHaveClass("underline", "bg-transparent");
  });

  describe("names the roots it looks in", () => {
    const withRoots = (...paths: string[]) => ({
      ...noDiscovery(),
      roots: paths.map((path): RootInfo => ({ path, clones: null, stopped_early: false, error: null })),
    });

    it("one root", () => {
      invite(withRoots("~/Development"));
      expect(screen.getByText(/^Looks in ~\/Development\./)).toBeInTheDocument();
    });

    it("two roots", () => {
      invite(withRoots("~/Development", "~/Code"));
      expect(screen.getByText(/^Looks in ~\/Development and ~\/Code\./)).toBeInTheDocument();
    });

    it("three or more roots: the first two and a count", () => {
      invite(withRoots("~/Development", "~/Code", "~/Projects", "~/src"));
      expect(screen.getByText(/^Looks in ~\/Development, ~\/Code and 2 more\./)).toBeInTheDocument();
    });

    it("no roots: looks nowhere, and Find clones has nothing to do", () => {
      invite(withRoots());
      expect(screen.getByText(/^Looks nowhere yet\./)).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Where to look…" })).toBeEnabled();
      expect(screen.getByRole("button", { name: "Find clones" })).toBeDisabled();
    });
  });
});

describe("EmptyState found-nothing", () => {
  const nothing = (discovery: Discovery, h = handlers()) => {
    render(<EmptyState variant="found-nothing" discovery={discovery} now={NOW} {...h} />);
    return h;
  };

  it("names the roots that were searched, and when", () => {
    nothing(looked([{ path: "~/Development" }, { path: "~/Code" }]));
    expect(
      screen.getByText("No clones in ~/Development or ~/Code. Look somewhere else, or add one by hand."),
    ).toBeInTheDocument();
    expect(screen.getByText("Looked just now.")).toBeInTheDocument();
  });

  it("adds the stopped-early part only when a root stopped", () => {
    nothing(looked([{ path: "~/Development" }, { path: "~/Code", stopped_early: true }]));
    expect(screen.getByText("Looked just now · stopped early in ~/Code after 20 000 folders.")).toBeInTheDocument();
  });

  it("dates the scan", () => {
    nothing(looked([{ path: "~/Development" }], "2026-11-01T12:00:00Z"));
    expect(screen.getByText("Looked 3 days ago.")).toBeInTheDocument();
  });

  it("leaves an unreadable root out of the searched ones and says why", () => {
    nothing(looked([{ path: "~/Development" }, { path: "~/work", clones: null, error: "permission denied" }]));
    expect(screen.getByText(/^No clones in ~\/Development\. /)).toBeInTheDocument();
    expect(screen.getByText("Cannot read ~/work: permission denied. ~/Development was searched.")).toBeInTheDocument();
  });

  it("offers Where to look… quietly and Add repository as the primary action", async () => {
    const h = nothing(looked([{ path: "~/Development" }]));
    const user = userEvent.setup();
    expect(screen.getByRole("button", { name: "Add repository" })).toHaveClass("bg-accent");
    expect(screen.getByRole("button", { name: "Where to look…" })).not.toHaveClass("bg-accent", "underline");
    await user.click(screen.getByRole("button", { name: "Where to look…" }));
    await user.click(screen.getByRole("button", { name: "Add repository" }));
    expect(h.onWhereToLook).toHaveBeenCalledTimes(1);
    expect(h.onAdd).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Find clones" })).not.toBeInTheDocument();
  });
});
