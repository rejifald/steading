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

  it("shows only Pause when nothing is listed", () => {
    render(<Footer hasRepos={false} paused={false} onSyncAll={noop} onTogglePause={noop} onAdd={noop} />);
    expect(screen.getAllByRole("button")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
  });

  it("adds a quiet Add repository at the right while Found is the body", () => {
    render(<Footer hasRepos={false} isFoundBody paused={false} onSyncAll={noop} onTogglePause={noop} onAdd={noop} />);
    const add = screen.getByRole("button", { name: "Add repository" });
    expect(add).toHaveClass("ml-auto");
    expect(add).not.toHaveClass("bg-accent");
    expect(screen.queryByRole("button", { name: "Sync all" })).not.toBeInTheDocument();
  });
});
