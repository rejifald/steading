import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { Row, Snapshot } from "./api";
import { noDiscovery } from "./api.fake";
import { Header } from "./Header";

const NOW = new Date("2026-11-01T12:00:00Z");

const row = (name: string, fields: Partial<Row> = {}): Row => ({
  name,
  path: `~/code/${name}`,
  branch: "main",
  mode: "follow",
  remote: "origin",
  interval_seconds: 300,
  health: "in-sync",
  last_sync: "2026-11-01T11:58:00Z",
  ...fields,
});

const seven = [
  row("a", { health: "needs-you" }),
  row("b", { health: "behind", behind: 3 }),
  ...["c", "d", "e", "f", "g"].map((name) => row(name)),
];

function snapshot(fields: Partial<Snapshot> = {}, repos: Row[] = seven, paused = false): Snapshot {
  const needsYou = repos.filter((r) => r.health === "needs-you").length;
  return {
    report: { paused, overall: needsYou ? "needs-you" : "in-sync", needs_you: needsYou, repos },
    error: null,
    syncing: false,
    busy: [],
    note: null,
    details: {},
    discovery: noDiscovery(),
    ...fields,
  };
}

const state = () => screen.getByRole("status");

describe("Header", () => {
  it("says checking… before the first report, with no right slot", () => {
    render(<Header snapshot={null} now={NOW} />);
    expect(state()).toHaveTextContent("checking…");
    expect(screen.queryByText(/repos/)).not.toBeInTheDocument();
  });

  it("says needs you on a read error, with no right slot", () => {
    render(<Header snapshot={{ ...snapshot(), report: null, error: "bad" }} now={NOW} />);
    expect(state()).toHaveTextContent(/^needs you$/);
    expect(screen.queryByText(/repos/)).not.toBeInTheDocument();
  });

  it("says paused before anything else, and keeps the repo count", () => {
    render(<Header snapshot={snapshot({}, seven, true)} now={NOW} />);
    expect(state()).toHaveTextContent(/^paused$/);
    expect(screen.getByText("7 repos")).toBeInTheDocument();
  });

  it("keeps the count of needs you while a sync runs, and counts the busy repos", () => {
    render(<Header snapshot={snapshot({ syncing: true, busy: ["~/code/b", "~/code/c"] })} now={NOW} isPending />);
    expect(state()).toHaveTextContent("needs you (1)");
    expect(screen.getByText("2 of 7")).toBeInTheDocument();
  });

  it("says syncing… when nothing needs you", () => {
    const calm = seven.map((r) => ({ ...r, health: "in-sync" as const }));
    render(<Header snapshot={snapshot({ syncing: true, busy: ["~/code/a"] }, calm)} now={NOW} />);
    expect(state()).toHaveTextContent("syncing…");
    expect(screen.getByText("1 of 7")).toBeInTheDocument();
  });

  it("gives the newest last sync when everything is in sync", () => {
    const calm = seven.map((r) => ({ ...r, health: "in-sync" as const }));
    render(<Header snapshot={snapshot({}, calm)} now={NOW} />);
    expect(state()).toHaveTextContent("in sync");
    expect(screen.getByText("last sync 2 min ago")).toBeInTheDocument();
  });

  it("draws the progress hairline only while pending", () => {
    const { container, rerender } = render(<Header snapshot={snapshot()} now={NOW} />);
    expect(container.querySelector("header [aria-hidden='true'] .animate-progress")).toBeNull();
    rerender(<Header snapshot={snapshot()} now={NOW} isPending />);
    expect(container.querySelector("header [aria-hidden='true'] .animate-progress")).not.toBeNull();
  });

  it("titles a repo's settings with its name and its state as a row shows it", () => {
    const repo = row("example-campaign-2026-landing", { health: "behind", behind: 12 });
    render(<Header snapshot={snapshot()} now={NOW} title={repo.name} showState={false} repo={repo} />);
    expect(screen.getByRole("heading", { name: repo.name })).toHaveClass("truncate");
    expect(screen.getByRole("banner")).toHaveTextContent("behind · 12");
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  describe("with nothing listed", () => {
    const found = (suggestions: number, scanning: string | null) =>
      snapshot(
        {
          discovery: {
            ...noDiscovery(),
            scanning,
            suggestions: Array.from({ length: suggestions }, (_, i) => ({
              path: `~/Code/c${i}`,
              name: `c${i}`,
              root: "~/Code",
              branch: "main",
            })),
          },
        },
        [],
      );

    it("counts the suggestions", () => {
      render(<Header snapshot={found(6, null)} now={NOW} showState={false} showFound />);
      expect(screen.getByText("6 found")).toBeInTheDocument();
    });

    it("says 0 found while a scan has found nothing yet", () => {
      render(<Header snapshot={found(0, "~/Code")} now={NOW} showState={false} showFound />);
      expect(screen.getByText("0 found")).toBeInTheDocument();
    });

    it("says nothing when there are no suggestions and no scan", () => {
      render(<Header snapshot={found(0, null)} now={NOW} showState={false} showFound />);
      expect(screen.queryByText(/found/)).not.toBeInTheDocument();
    });

    it("keeps the count out of views that show the title alone", () => {
      render(<Header snapshot={found(6, null)} now={NOW} showState={false} />);
      expect(screen.queryByText(/found/)).not.toBeInTheDocument();
    });
  });
});
