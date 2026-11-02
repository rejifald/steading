import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { Row } from "../api";
import { createFakeApi } from "../api.fake";
import { RepoSettings } from "./RepoSettings";

const NOW = new Date("2026-11-01T12:00:00Z");

const ROW: Row = {
  name: "example-api",
  path: "~/Development/example-api",
  branch: "main",
  mode: "follow",
  remote: "origin",
  interval_seconds: 300,
  health: "behind",
  behind: 3,
  last_sync: "2026-11-01T11:56:00Z",
};

function setup(row: Row = ROW, detail?: string) {
  const api = createFakeApi({ rows: [ROW] });
  const onPendingChange = vi.fn();
  render(
    <RepoSettings
      row={row}
      detail={detail}
      now={NOW}
      api={api}
      onDone={() => {}}
      onCancel={() => {}}
      onPendingChange={onPendingChange}
    />,
  );
  return { api, onPendingChange, user: userEvent.setup() };
}

const fact = (label: string) => screen.getByText(label, { selector: "dt" }).nextElementSibling;

describe("RepoSettings", () => {
  it("lists folder, branch and last sync as one-line facts", () => {
    setup();
    expect(fact("Folder")).toHaveTextContent("~/Development/example-api");
    expect(fact("Folder")).toHaveAttribute("title", "~/Development/example-api");
    expect(fact("Branch")).toHaveTextContent("main · follow");
    expect(fact("Last sync")).toHaveTextContent("4 min ago");
    expect(screen.queryByText("Gate")).not.toBeInTheDocument();
    expect(screen.queryByText("After")).not.toBeInTheDocument();
  });

  it("adds Gate and After in monospace when they are set", () => {
    setup({ ...ROW, gate: 'test -z "$(git status --porcelain)"', after: "./scripts/reconcile.sh" });
    expect(fact("Gate")).toHaveTextContent('test -z "$(git status --porcelain)"');
    expect(fact("Gate")).toHaveClass("font-mono");
    expect(fact("Gate")).toHaveAttribute("title", 'test -z "$(git status --porcelain)"');
    expect(fact("After")).toHaveTextContent("./scripts/reconcile.sh");
  });

  it("reads never for a repo never synced", () => {
    const { last_sync: _, ...neverSynced } = ROW;
    setup(neverSynced);
    expect(fact("Last sync")).toHaveTextContent("never");
  });

  it("opens a needs-you repo with its reason in a callout", () => {
    setup({ ...ROW, health: "needs-you" }, "after failed: reconcile: 2 links point outside the clone");
    const callout = screen.getByRole("region", { name: "example-api needs you" });
    expect(callout).toHaveTextContent("needs you");
    expect(callout).toHaveTextContent("4 min ago");
    expect(callout).toHaveTextContent("after failed: reconcile: 2 links point outside the clone");
    expect(callout.querySelector("button")).toBeNull();
  });

  it("confirms a removal in a plain callout", async () => {
    const { user } = setup();
    await user.click(screen.getByRole("button", { name: "Remove from tether…" }));
    const heading = screen.getByText("Stop keeping example-api on main?");
    expect(heading.parentElement).not.toHaveClass("bg-needs-you/14");
    expect(screen.getByText("The clone stays as it is.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Remove from tether…" })).not.toBeInTheDocument();
  });

  it("reads Removing… and locks the confirm while removing", async () => {
    const { api, onPendingChange, user } = setup();
    vi.spyOn(api, "remove").mockReturnValue(new Promise(() => {}));
    await user.click(screen.getByRole("button", { name: "Remove from tether…" }));
    await user.click(screen.getByRole("button", { name: "Remove" }));
    expect(screen.getByRole("button", { name: "Removing…" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Keep" })).toBeDisabled();
    expect(onPendingChange).toHaveBeenLastCalledWith(true);
    expect(screen.getByRole("form")).not.toHaveClass("opacity-60");
  });

  it("titles a refused removal Not removed", async () => {
    const { api, user } = setup();
    vi.spyOn(api, "remove").mockRejectedValue("~/Development/example-api is no longer listed");
    await user.click(screen.getByRole("button", { name: "Remove from tether…" }));
    await user.click(screen.getByRole("button", { name: "Remove" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Not removed");
    expect(alert).toHaveTextContent("~/Development/example-api is no longer listed");
  });

  it("titles a refused save Not saved", async () => {
    const { api, user } = setup();
    vi.spyOn(api, "setInterval").mockRejectedValue("~/Development/example-api is no longer listed");
    await user.click(screen.getByRole("combobox", { name: "Pull every" }));
    await user.click(await screen.findByRole("option", { name: "1 hour" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Not saved");
    expect(alert).toHaveTextContent("is no longer listed");
  });
});
