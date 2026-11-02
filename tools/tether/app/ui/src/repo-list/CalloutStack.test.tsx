import { Tooltip } from "@base-ui/react/tooltip";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { Row } from "../api";
import { CalloutStack } from "./CalloutStack";

const row = (name: string): Row => ({
  name,
  path: `~/code/${name}`,
  branch: "main",
  mode: "follow",
  remote: "origin",
  interval_seconds: 300,
  health: "needs-you",
});

const ROWS = [row("example-site"), row("example-handbook"), row("example-cli")];

function setup(rows: Row[]) {
  const noop = vi.fn();
  const { container } = render(
    <Tooltip.Provider>
      <CalloutStack
        rows={rows}
        details={{}}
        isBusy={() => false}
        now={new Date("2026-11-01T12:00:00Z")}
        onSync={noop}
        onOpen={noop}
        onSettings={noop}
        onDismiss={noop}
      />
    </Tooltip.Provider>,
  );
  return { container, user: userEvent.setup() };
}

const callouts = () => screen.getAllByRole("region").map((region) => region.getAttribute("aria-label"));

describe("CalloutStack", () => {
  it("shows a single callout alone", () => {
    setup(ROWS.slice(0, 1));
    expect(callouts()).toEqual(["example-site needs you"]);
    expect(screen.queryByRole("button", { name: "Show all" })).not.toBeInTheDocument();
  });

  it("folds two callouts behind the first with one sliver", () => {
    const { container } = setup(ROWS.slice(0, 2));
    expect(callouts()).toEqual(["example-site needs you"]);
    expect(screen.getByText("1 more needs you")).toBeInTheDocument();
    expect(container.querySelectorAll("span[aria-hidden='true'].rounded-b-control")).toHaveLength(1);
  });

  it("folds three callouts behind the first with two slivers", () => {
    const { container } = setup(ROWS);
    expect(screen.getByText("2 more need you")).toBeInTheDocument();
    expect(container.querySelectorAll("span[aria-hidden='true'].rounded-b-control")).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Show all" })).toHaveAttribute("aria-expanded", "false");
  });

  it("opens every callout from Show all, focusing the second one", async () => {
    const { user } = setup(ROWS);
    await user.click(screen.getByRole("button", { name: "Show all" }));
    expect(callouts()).toEqual(["example-site needs you", "example-handbook needs you", "example-cli needs you"]);
    expect(screen.getByRole("button", { name: "Show less" })).toHaveAttribute("aria-expanded", "true");
    const second = screen.getByRole("region", { name: "example-handbook needs you" });
    expect(second.contains(document.activeElement)).toBe(true);
  });

  it("folds back from Show less", async () => {
    const { user } = setup(ROWS);
    await user.click(screen.getByRole("button", { name: "Show all" }));
    await user.click(screen.getByRole("button", { name: "Show less" }));
    expect(callouts()).toEqual(["example-site needs you"]);
    expect(screen.getByRole("button", { name: "Show all" })).toHaveFocus();
  });

  it("folds again when the popover is hidden", async () => {
    const { user } = setup(ROWS);
    await user.click(screen.getByRole("button", { name: "Show all" }));
    window.dispatchEvent(new Event("blur"));
    expect(await screen.findByRole("button", { name: "Show all" })).toBeInTheDocument();
  });
});
