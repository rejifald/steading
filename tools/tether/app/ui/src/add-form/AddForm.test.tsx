import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { createFakeApi } from "../api.fake";
import { AddForm } from "./AddForm";

const setup = () => {
  const api = createFakeApi();
  const onDone = vi.fn();
  const onCancel = vi.fn();
  const user = userEvent.setup();
  render(<AddForm api={api} onDone={onDone} onCancel={onCancel} />);
  return { api, onDone, onCancel, user };
};

const choose = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(screen.getByRole("button", { name: "Choose…" }));
  await screen.findByText("checked out now");
};

describe("AddForm", () => {
  it("starts with Branch and Add disabled", () => {
    setup();
    expect(screen.getByLabelText("Branch")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Add" })).toBeDisabled();
  });

  it("fills the branch and says it is checked out now", async () => {
    const { user } = setup();
    await choose(user);
    expect(screen.getByLabelText("Branch")).toHaveValue("main");
  });

  it("hides the note once the branch is edited", async () => {
    const { user } = setup();
    await choose(user);
    await user.type(screen.getByLabelText("Branch"), "x");
    expect(screen.queryByText("checked out now")).not.toBeInTheDocument();
  });

  it("previews the repo as a row not listed yet", async () => {
    const { user } = setup();
    await choose(user);
    const preview = screen.getByText("example-new").parentElement!;
    expect(preview).toHaveTextContent("example-newmainnot listed yet");
    for (const state of ["in sync", "behind", "skipped", "needs you"]) {
      expect(preview).not.toHaveTextContent(state);
    }
  });

  it("shows the clone URL for a mirror", async () => {
    const { user } = setup();
    await choose(user);
    expect(screen.queryByLabelText("Clone URL")).not.toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: /mirror/ }));
    expect(screen.getByLabelText("Clone URL")).toBeInTheDocument();
  });

  it("sends gate and after only once they are filled in", async () => {
    const { api, user } = setup();
    const add = vi.spyOn(api, "add");
    await choose(user);
    expect(screen.queryByLabelText("Gate")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add commands…" }));
    await user.type(screen.getByLabelText("Gate"), "  tool idle ");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(add).toHaveBeenCalledWith(
      expect.objectContaining({ gate: "tool idle", after: null }),
    );
  });

  it("reads Adding… while add is pending, without fading the form", async () => {
    const { api, user } = setup();
    vi.spyOn(api, "add").mockReturnValue(new Promise(() => {}));
    await choose(user);
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(
      await screen.findByRole("button", { name: "Adding…" }),
    ).toBeDisabled();
    expect(screen.getByRole("form")).not.toHaveClass("opacity-60");
    expect(screen.getByRole("radio", { name: /follow/ })).toHaveAttribute("data-disabled");
  });

  it("tells its parent while an add is pending", async () => {
    const api = createFakeApi();
    const onPendingChange = vi.fn();
    const user = userEvent.setup();
    vi.spyOn(api, "add").mockReturnValue(new Promise(() => {}));
    render(<AddForm api={api} onDone={() => {}} onCancel={() => {}} onPendingChange={onPendingChange} />);
    await choose(user);
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(onPendingChange).toHaveBeenLastCalledWith(true);
  });

  it("shows a rejected add in a Not added callout", async () => {
    const { api, user } = setup();
    vi.spyOn(api, "add").mockRejectedValue("origin has no branch main");
    await choose(user);
    await user.click(screen.getByRole("button", { name: "Add" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Not added");
    expect(alert).toHaveTextContent("origin has no branch main");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Add" })).toBeEnabled(),
    );
  });

  it("calls onCancel", async () => {
    const { onCancel, user } = setup();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalled();
  });
});
