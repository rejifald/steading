import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { Discovery, RootInfo } from "../api";
import { EmptyState } from "./EmptyState";

const NOW = new Date("2026-01-01T12:00:00Z");
const root = (path: string, fields: Partial<RootInfo> = {}): RootInfo => ({
  path,
  clones: null,
  stopped_early: false,
  error: null,
  ...fields,
});
const discovery = (roots: RootInfo[], fields: Partial<Discovery> = {}): Discovery => ({
  scanned_at: null,
  scanning: null,
  roots,
  suggestions: [],
  skipped: [],
  ...fields,
});
const TWO = [root("~/Development"), root("~/Code")];

const handlers = () => ({ onAdd: vi.fn(), onFind: vi.fn(), onWhereToLook: vi.fn() });

describe("EmptyState invite", () => {
  it("explains what reap does with the clones it finds", () => {
    render(<EmptyState variant="invite" discovery={discovery(TWO)} now={NOW} {...handlers()} />);
    expect(
      screen.getByText(
        "reap removes the worktrees whose work has merged. Find the clones on this Mac that have worktrees, or add one by hand.",
      ),
    ).toBeInTheDocument();
  });

  it("shows two example clones with their worktree counts, hidden from assistive technology", () => {
    render(<EmptyState variant="invite" discovery={discovery(TWO)} now={NOW} {...handlers()} />);
    const preview = screen.getByText("your-app").closest("[aria-hidden='true']");
    expect(preview).not.toBeNull();
    expect(preview).toHaveTextContent("7 worktrees");
    expect(preview).toHaveTextContent("your-site");
    expect(preview).toHaveTextContent("2 worktrees");
    expect(preview).toHaveTextContent("Add");
  });

  it("makes Find clones the primary button and Add repository the quiet one", () => {
    render(<EmptyState variant="invite" discovery={discovery(TWO)} now={NOW} {...handlers()} />);
    expect(screen.getByRole("button", { name: "Find clones" })).toHaveClass("bg-accent");
    expect(screen.getByRole("button", { name: "Add repository" })).not.toHaveClass("bg-accent");
  });

  it("calls the right handler from each button", async () => {
    const { onAdd, onFind, onWhereToLook } = handlers();
    render(<EmptyState variant="invite" discovery={discovery(TWO)} now={NOW} {...{ onAdd, onFind, onWhereToLook }} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Add repository" }));
    await user.click(screen.getByRole("button", { name: "Find clones" }));
    await user.click(screen.getByRole("button", { name: "Where to look…" }));
    expect([onAdd, onFind, onWhereToLook].map((handler) => handler.mock.calls.length)).toEqual([1, 1, 1]);
  });

  it.each([
    [[root("~/Development")], "Looks in ~/Development."],
    [TWO, "Looks in ~/Development and ~/Code."],
    [[...TWO, root("~/Projects"), root("~/src")], "Looks in ~/Development, ~/Code and 2 more."],
    [[], "Looks nowhere yet."],
  ])("names the roots in the copy (%#)", (roots, line) => {
    render(<EmptyState variant="invite" discovery={discovery(roots)} now={NOW} {...handlers()} />);
    expect(screen.getByText(line)).toBeInTheDocument();
    // The link variant: underlined text in the sentence.
    expect(screen.getByRole("button", { name: "Where to look…" })).toHaveClass("underline", "bg-transparent");
  });
});

describe("EmptyState found-nothing", () => {
  const scanned = (roots: RootInfo[]) => discovery(roots, { scanned_at: NOW.toISOString() });

  it("says where it looked, with Where to look… quiet and Add repository primary", () => {
    render(<EmptyState variant="found-nothing" discovery={scanned(TWO)} now={NOW} {...handlers()} />);
    expect(
      screen.getByText("No clones with worktrees in ~/Development and ~/Code. Look somewhere else, or add one by hand."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Where to look…" })).not.toHaveClass("underline");
    expect(screen.getByRole("button", { name: "Add repository" })).toHaveClass("bg-accent");
    expect(screen.getByText("Looked just now.")).toBeInTheDocument();
  });

  it("adds the stopped-early suffix only when a root stopped early", () => {
    render(
      <EmptyState
        variant="found-nothing"
        discovery={scanned([root("~/Development"), root("~/Code", { stopped_early: true })])}
        now={NOW}
        {...handlers()}
      />,
    );
    expect(screen.getByText("Looked just now · stopped early in ~/Code after 20 000 folders.")).toBeInTheDocument();
  });

  it("calls the right handler from each button", async () => {
    const { onAdd, onWhereToLook } = handlers();
    render(<EmptyState variant="found-nothing" discovery={scanned(TWO)} now={NOW} {...{ onAdd, onWhereToLook }} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Where to look…" }));
    await user.click(screen.getByRole("button", { name: "Add repository" }));
    expect([onAdd, onWhereToLook].map((handler) => handler.mock.calls.length)).toEqual([1, 1]);
  });

  it("asks for a folder when none is left to look in", () => {
    render(<EmptyState variant="found-nothing" discovery={scanned([])} now={NOW} {...handlers()} />);
    expect(screen.getByText("No folders to look in. Add one, or add a repository by hand.")).toBeInTheDocument();
  });
});
