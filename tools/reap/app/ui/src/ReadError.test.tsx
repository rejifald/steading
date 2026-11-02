import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ReadError } from "./ReadError";

describe("ReadError", () => {
  it("is an alert with the title, the message and the next step", () => {
    render(<ReadError message="expected a table at line 3" onRetry={() => {}} />);
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("Cannot read repos.toml");
    expect(alert).toHaveTextContent("expected a table at line 3");
    expect(alert).toHaveTextContent("Fix the file, then try again. Nothing is checked until it reads.");
  });

  it("calls onRetry from Try again", async () => {
    const onRetry = vi.fn();
    render(<ReadError message="bad" onRetry={onRetry} />);
    await userEvent.setup().click(screen.getByRole("button", { name: "Try again" }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });
});
