import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { Health } from "./api";
import { StateLabel } from "./StateLabel";

describe("StateLabel", () => {
  it.each<[Health, string, string]>([
    ["in-sync", "in sync", "text-faint"],
    ["behind", "behind", "text-behind"],
    ["skipped", "skipped", "text-skipped"],
    ["needs-you", "needs you", "text-needs-you"],
  ])("colours the %s word only where it differs", (health, word, colour) => {
    render(<StateLabel health={health} />);
    expect(screen.getByText(word)).toHaveClass(colour);
  });

  it("adds the behind count only when given", () => {
    const { container, rerender } = render(<StateLabel health="behind" behind={3} />);
    expect(container).toHaveTextContent("behind · 3");
    rerender(<StateLabel health="behind" />);
    expect(container).toHaveTextContent(/^behind$/);
  });

  it("reads syncing… while busy", () => {
    render(<StateLabel health="behind" behind={3} busy />);
    expect(screen.getByText("syncing…")).toHaveClass("text-behind");
    expect(screen.queryByText(/behind/)).not.toBeInTheDocument();
  });

  it("draws no glyph in the row tone", () => {
    const { container } = render(<StateLabel health="in-sync" />);
    expect(container.querySelector("svg")).toBeNull();
  });

  it("leads with a glyph in the header tone, muted when in sync", () => {
    const { container } = render(<StateLabel tone="header" health="in-sync" />);
    expect(container.querySelector("svg")).toHaveClass("text-in-sync");
    expect(screen.getByText("in sync")).toHaveClass("text-muted");
  });

  it("says paused with the pause glyph in the header tone", () => {
    const { container } = render(<StateLabel tone="header" health="behind" paused />);
    expect(screen.getByText("paused")).toHaveClass("text-muted");
    expect(container.querySelector("svg")).toHaveClass("text-faint");
  });

  it("spins the arc and says syncing… in the header tone", () => {
    const { container } = render(<StateLabel tone="header" health="in-sync" busy />);
    expect(screen.getByText("syncing…")).toHaveClass("text-behind");
    expect(container.querySelector("svg")).toHaveClass("animate-spin");
  });
});
