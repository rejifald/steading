import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { LoadingRows } from "./LoadingRows";

describe("LoadingRows", () => {
  it("is marked busy", () => {
    const { container } = render(<LoadingRows />);
    expect(container.firstElementChild).toHaveAttribute("aria-busy", "true");
  });

  it("draws four rows of two bars", () => {
    const { container } = render(<LoadingRows />);
    const rows = container.firstElementChild?.children ?? [];
    expect(rows).toHaveLength(4);
    for (const row of Array.from(rows)) expect(row.children).toHaveLength(2);
  });

  it("does not show the empty copy or a button", () => {
    render(<LoadingRows />);
    expect(screen.queryByText(/reap removes the worktrees/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
