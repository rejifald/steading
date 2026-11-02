import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { LoadingRows } from "./LoadingRows";

describe("LoadingRows", () => {
  it("is marked busy", () => {
    const { container } = render(<LoadingRows />);
    expect(container.firstElementChild).toHaveAttribute("aria-busy", "true");
  });

  it("draws five rows of a glyph circle and two bars", () => {
    const { container } = render(<LoadingRows />);
    const rows = container.firstElementChild?.children ?? [];
    expect(rows).toHaveLength(5);
    for (const row of Array.from(rows)) {
      expect(row.children).toHaveLength(3);
      expect(row.firstElementChild).toHaveClass("rounded-full", "size-4");
    }
  });

  it("does not move", () => {
    const { container } = render(<LoadingRows />);
    expect(container.querySelector("[class*='animate']")).toBeNull();
  });

  it("does not show the empty copy or a button", () => {
    render(<LoadingRows />);
    expect(screen.queryByText(/tether keeps each clone/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
