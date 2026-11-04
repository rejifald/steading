import { Field } from "@base-ui/react/field";
import { useState, type FormEvent } from "react";

import { TextButton } from "@steading/tray-popover/controls/TextButton";

import type { ReapApi } from "../api";
import { WorktreeLine } from "../worktree-list/WorktreeLine";

const FIELD = "flex flex-col gap-1";
const LABEL = "text-xs font-medium text-muted";
const INPUT =
  "h-7 min-w-0 rounded-control border border-border bg-surface-raised px-2 text-sm text-text placeholder:text-faint disabled:opacity-50 focus:outline-2 focus:-outline-offset-1 focus:outline-focus read-only:text-muted";

interface AddFormProps {
  api: Pick<ReapApi, "pickFolder" | "add">;
  onDone: () => void;
  onCancel: () => void;
}

/** *Add repository*: just the folder; reap finds the remote and the base itself. */
export function AddForm({ api, onDone, onCancel }: AddFormProps) {
  const [path, setPath] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setSubmitting] = useState(false);

  const choose = async () => {
    setError(null);
    try {
      const picked = await api.pickFolder();
      if (picked !== null) setPath(picked);
    } catch (e) {
      setError(String(e));
    }
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      await api.add(path);
      onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  const hasFolder = path !== "";
  const name = path.split("/").filter(Boolean).pop() ?? path;

  return (
    <form
      aria-label="Add repository"
      onSubmit={submit}
      className={`flex min-h-0 flex-1 flex-col ${isSubmitting ? "opacity-60" : ""}`}
    >
      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
        <Field.Root className={FIELD} disabled={isSubmitting}>
          <Field.Label className={LABEL}>Folder</Field.Label>
          <div className="flex gap-2">
            <Field.Control
              readOnly
              value={path}
              placeholder="No folder chosen"
              className={`flex-1 ${INPUT}`}
            />
            <TextButton variant={hasFolder ? "quiet" : "primary"} onClick={choose} disabled={isSubmitting}>
              Choose…
            </TextButton>
          </div>
        </Field.Root>

        {!hasFolder && (
          <p className="text-xs text-muted">
            Pick a clone. reap lists its linked worktrees and finds the remote and the base branch itself.
          </p>
        )}

        {hasFolder && (
          <div className="rounded-control border border-dashed border-border">
            <WorktreeLine ghost name={name} detail={path} />
          </div>
        )}

        {error && (
          <p role="alert" className="mt-auto text-xs text-needs-you">
            {error}
          </p>
        )}
      </div>

      <div className="flex justify-end gap-2 border-t border-border px-4 py-3">
        <TextButton onClick={onCancel} disabled={isSubmitting}>
          Cancel
        </TextButton>
        <TextButton type="submit" variant="primary" disabled={!hasFolder || isSubmitting}>
          {isSubmitting ? "Adding…" : "Add"}
        </TextButton>
      </div>
    </form>
  );
}
