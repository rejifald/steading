import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { EmptyState } from "./EmptyState";

describe("EmptyState", () => {
  it("explains what tether does", () => {
    render(<EmptyState onAdd={() => {}} />);
    expect(
      screen.getByText("tether keeps each clone you list on its branch, pulling in the background."),
    ).toBeInTheDocument();
  });

  it("calls onAdd from the Add repository button", async () => {
    const onAdd = vi.fn();
    render(<EmptyState onAdd={onAdd} />);
    await userEvent.setup().click(screen.getByRole("button", { name: "Add repository" }));
    expect(onAdd).toHaveBeenCalledTimes(1);
  });

  it("draws the ghost rows in the row shape, glyphs and words", () => {
    const { container } = render(<EmptyState onAdd={() => {}} />);
    expect(container.querySelectorAll("svg")).toHaveLength(2);
    expect(screen.getByText("in sync")).toBeInTheDocument();
    expect(screen.getByText("behind").parentElement).toHaveTextContent("behind · 3");
  });

  it("hides the ghost rows from the accessibility tree", () => {
    render(<EmptyState onAdd={() => {}} />);
    expect(screen.getByText("your-app").closest("[aria-hidden='true']")).not.toBeNull();
    expect(screen.getByText("your-docs").closest("[aria-hidden='true']")).not.toBeNull();
  });
});
