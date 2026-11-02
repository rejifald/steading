import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { EmptyState } from "./EmptyState";

describe("EmptyState", () => {
  it("explains what reap does and hides the ghost lines from assistive technology", () => {
    render(<EmptyState onAdd={() => {}} />);
    expect(
      screen.getByText("reap removes the worktrees whose work has merged, once nothing runs inside them."),
    ).toBeInTheDocument();
    expect(screen.getByText("fix-login").parentElement).toHaveAttribute("aria-hidden", "true");
  });

  it("calls onAdd from Add repository", async () => {
    const onAdd = vi.fn();
    render(<EmptyState onAdd={onAdd} />);
    await userEvent.setup().click(screen.getByRole("button", { name: "Add repository" }));
    expect(onAdd).toHaveBeenCalledTimes(1);
  });
});
