import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Footer } from "./Footer";

const noop = () => {};

describe("Footer", () => {
  it("makes Resume the primary button and quiets Add repository while paused", () => {
    render(<Footer hasRepos paused onSyncAll={noop} onTogglePause={noop} onAdd={noop} />);
    expect(screen.getByRole("button", { name: "Resume" })).toHaveClass("bg-accent");
    expect(screen.getByRole("button", { name: "Add repository" })).not.toHaveClass("bg-accent");
  });

  it("keeps Add repository primary while running", () => {
    render(<Footer hasRepos paused={false} onSyncAll={noop} onTogglePause={noop} onAdd={noop} />);
    expect(screen.getByRole("button", { name: "Pause" })).not.toHaveClass("bg-accent");
    expect(screen.getByRole("button", { name: "Add repository" })).toHaveClass("bg-accent");
  });

  it("disables Sync all while a sync runs", () => {
    render(<Footer hasRepos isSyncing paused={false} onSyncAll={noop} onTogglePause={noop} onAdd={noop} />);
    expect(screen.getByRole("button", { name: "Sync all" })).toBeDisabled();
  });
});
