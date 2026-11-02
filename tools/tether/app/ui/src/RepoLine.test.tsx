import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { Health } from "./api";
import { RepoLine } from "./RepoLine";

const glyph = (container: HTMLElement) => container.querySelector("svg");

describe("RepoLine", () => {
  it("shows the mirror suffix for a mirror", () => {
    render(<RepoLine name="app" branch="main" mode="mirror" health="in-sync" />);
    expect(screen.getByText("main · mirror")).toBeTruthy();
  });

  it("shows the bare branch for a follow", () => {
    render(<RepoLine name="app" branch="main" mode="follow" health="in-sync" />);
    expect(screen.getByText("main")).toBeTruthy();
    expect(screen.queryByText(/mirror/)).toBeNull();
  });

  it.each<[Health, string]>([
    ["in-sync", "text-in-sync"],
    ["behind", "text-behind"],
    ["skipped", "text-skipped"],
    ["needs-you", "text-needs-you"],
  ])("leads a %s line with its glyph in the state colour", (health, colour) => {
    const { container } = render(<RepoLine name="app" branch="main" mode="follow" health={health} />);
    expect(glyph(container)).toHaveClass(colour);
  });

  it("shows the state word and the behind count", () => {
    render(<RepoLine name="app" branch="main" mode="follow" health="behind" behind={3} />);
    expect(screen.getByText("behind")).toBeTruthy();
    expect(screen.getByText("· 3", { exact: false })).toBeTruthy();
  });

  it("spins the arc and says syncing… while busy", () => {
    const { container } = render(<RepoLine name="app" branch="main" mode="follow" health="behind" busy />);
    expect(glyph(container)).toHaveClass("animate-spin", "text-behind");
    expect(screen.getByText("syncing…")).toBeTruthy();
  });

  it("leads with the pause glyph while paused, keeping the word", () => {
    const { container } = render(<RepoLine name="app" branch="main" mode="follow" health="skipped" paused />);
    expect(glyph(container)).toHaveClass("text-faint");
    expect(screen.getByText("skipped")).toBeTruthy();
  });

  it("previews a repo not listed yet with a dashed circle", () => {
    const { container } = render(<RepoLine preview name="app" branch="main" mode="follow" />);
    expect(screen.getByText("not listed yet")).toBeTruthy();
    expect(glyph(container)?.querySelector("circle")).toHaveAttribute("stroke-dasharray", "2 2.4");
    expect(container.firstElementChild?.getAttribute("aria-hidden")).toBe("true");
  });

  it("hides a ghost line from assistive technology", () => {
    const { container } = render(<RepoLine ghost name="app" branch="main" mode="follow" health="in-sync" />);
    expect(container.firstElementChild?.getAttribute("aria-hidden")).toBe("true");
  });

  it("puts the path on the truncated name and branch", () => {
    render(<RepoLine name="app" branch="main" mode="follow" health="in-sync" title="~/code/app" />);
    expect(screen.getByText("app")).toHaveAttribute("title", "~/code/app");
    expect(screen.getByText("main")).toHaveAttribute("title", "~/code/app");
  });
});
