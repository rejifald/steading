import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import type { Repo, Salvaged } from "../api";
import { SalvagedSection, SavedAbandonedSection, salvagedEntriesOf, salvagedTitleOf } from "./SalvagedSection";

const NOW = new Date("2026-11-04T12:00:00Z");

function salvaged(fields: Partial<Salvaged> & Pick<Salvaged, "refname" | "saved_at">): Salvaged {
  return {
    commit: "0123456789abcdef0123456789abcdef01234567",
    kind: "merged",
    delete: `git reap salvaged --delete ${fields.refname}`,
    restore: `git -C ~/Development/app worktree add ~/Development/app-feat ${fields.refname} && git -C ~/Development/app-feat reset HEAD~1`,
    ...fields,
  };
}

function repo(saves: Salvaged[]): Repo {
  return { name: "app", path: "~/Development/app", base: "origin/main", policy: "auto", worktrees: [], salvaged: saves };
}

describe("SalvagedSection", () => {
  it("shows nothing when nothing was saved", () => {
    const older: Repo = { name: "old", path: "~/Development/old", base: "origin/main", policy: "auto", worktrees: [] };
    const { container } = render(<SalvagedSection repos={[repo([]), older]} now={NOW} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("lists each save, newest first, with how to restore it", async () => {
    const older = salvaged({
      refname: "refs/reap/salvage/app-old/20261020-090000",
      saved_at: "2026-10-20T09:00:00Z",
    });
    const newer = salvaged({
      refname: "refs/reap/salvage/app-feat/20261102-120000",
      branch: "feat/login",
      saved_at: "2026-11-02T12:00:00Z",
      prunes_at: "2026-12-02T12:00:00Z",
    });
    render(<SalvagedSection repos={[repo([older, newer])]} now={NOW} />);

    await userEvent.setup().click(screen.getByRole("button", { name: /Salvaged/ }));

    const items = screen.getAllByRole("listitem").map((item) => item.getAttribute("aria-label"));
    expect(items).toEqual(["feat/login", "app-old"]);
    expect(screen.getByText("Saved 2 d ago · deleted in 28 d")).toBeInTheDocument();
    expect(screen.getByText(newer.restore)).toBeInTheDocument();
  });

  it("orders saves across repos and titles one with no branch by its folder", () => {
    const a = salvaged({ refname: "refs/reap/salvage/a/1", saved_at: "2026-11-01T00:00:00Z" });
    const b = salvaged({ refname: "refs/reap/salvage/b/2", saved_at: "2026-11-03T00:00:00Z" });
    expect(salvagedEntriesOf([repo([a]), { ...repo([b]), name: "other" }]).map((entry) => entry.repo.name)).toEqual([
      "other",
      "app",
    ]);
    expect(salvagedTitleOf(a)).toBe("a");
  });

  it("lists abandoned saves apart, with no expiry", async () => {
    const merged = salvaged({ refname: "refs/reap/salvage/m/1", saved_at: "2026-11-01T00:00:00Z" });
    const abandoned = salvaged({
      refname: "refs/reap/salvage/idle/2",
      kind: "abandoned",
      branch: "docs/pages",
      saved_at: "2026-11-02T12:00:00Z",
    });
    render(
      <>
        <SavedAbandonedSection repos={[repo([merged, abandoned])]} now={NOW} onDelete={() => {}} />
        <SalvagedSection repos={[repo([merged, abandoned])]} now={NOW} />
      </>,
    );

    await userEvent.setup().click(screen.getByRole("button", { name: /Saved abandoned work/ }));
    expect(screen.getByRole("listitem", { name: "docs/pages" })).toBeInTheDocument();
    expect(screen.getByText("Saved 2 d ago · kept until you delete it")).toBeInTheDocument();
    expect(screen.queryByRole("listitem", { name: "m" })).not.toBeInTheDocument();
    expect(salvagedEntriesOf([repo([merged, abandoned])], "merged").map((entry) => entry.salvaged.refname)).toEqual([
      merged.refname,
    ]);
  });
});
