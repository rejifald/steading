import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { PopoverHeader } from "./PopoverHeader";

describe("PopoverHeader", () => {
  it("announces the state as a status beside the title", () => {
    render(<PopoverHeader title="reap" state="tidy" />);
    expect(screen.getByRole("heading", { name: "reap" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("tidy");
  });

  it("has no status without a state", () => {
    render(<PopoverHeader title="Add repository" />);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("draws the progress hairline only while pending", () => {
    const { container, rerender } = render(<PopoverHeader title="x" />);
    expect(container.querySelector(".animate-progress")).toBeNull();
    rerender(<PopoverHeader title="x" isPending />);
    expect(container.querySelector(".animate-progress")).not.toBeNull();
  });
});
