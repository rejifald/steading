import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { createFakeApi } from "../api.fake";
import { AddForm } from "./AddForm";

const setup = (options: Parameters<typeof createFakeApi>[0] = {}) => {
  const api = createFakeApi(options);
  const onDone = vi.fn();
  const onCancel = vi.fn();
  const user = userEvent.setup();
  render(<AddForm api={api} onDone={onDone} onCancel={onCancel} />);
  return { api, onDone, onCancel, user };
};

describe("AddForm", () => {
  it("starts with Add disabled and no preview", () => {
    setup();
    expect(screen.getByRole("button", { name: "Add" })).toBeDisabled();
    expect(screen.queryByText("example-new")).not.toBeInTheDocument();
  });

  it("shows the chosen folder with ~ and a preview line without a state word", async () => {
    const { user } = setup();
    await user.click(screen.getByRole("button", { name: "Choose…" }));
    expect(screen.getByLabelText("Folder")).toHaveValue("~/Development/example-new");
    const preview = screen.getByText("example-new").parentElement;
    expect(preview).toHaveTextContent("example-new~/Development/example-new");
    expect(screen.getByRole("button", { name: "Add" })).toBeEnabled();
  });

  it("keeps the form empty when the picker is cancelled", async () => {
    const { user } = setup({ pickedFolder: null });
    await user.click(screen.getByRole("button", { name: "Choose…" }));
    expect(screen.getByLabelText("Folder")).toHaveValue("");
  });

  it("adds the chosen path and reports done", async () => {
    const { api, onDone, user } = setup();
    const add = vi.spyOn(api, "add");
    await user.click(screen.getByRole("button", { name: "Choose…" }));
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(add).toHaveBeenCalledWith("~/Development/example-new");
    expect(onDone).toHaveBeenCalled();
  });

  it("shows the error and stays open when add fails", async () => {
    const { api, onDone, user } = setup();
    vi.spyOn(api, "add").mockRejectedValue("not a git repository");
    await user.click(screen.getByRole("button", { name: "Choose…" }));
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("not a git repository");
    expect(onDone).not.toHaveBeenCalled();
  });

  it("calls onCancel from Cancel", async () => {
    const { onCancel, user } = setup();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalled();
  });
});
