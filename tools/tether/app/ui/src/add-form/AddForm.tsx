import { Field } from "@base-ui/react/field";
import { RadioGroup } from "@base-ui/react/radio-group";
import { useEffect, useId, useState, type FormEvent } from "react";

import type { AddRequest, Mode, TetherApi } from "../api";
import { Callout } from "../Callout";
import { NeedsYouIcon } from "../controls/icons";
import { IntervalSelect } from "../interval/IntervalSelect";
import { DEFAULT_INTERVAL_SECONDS, PRESETS, choiceOf } from "../interval/intervals";
import { RepoLine } from "../RepoLine";
import { TextButton } from "../controls/TextButton";
import { ModeOption } from "./ModeOption";

const MODES: { value: Mode; label: string; hint: string }[] = [
  {
    value: "follow",
    label: "follow",
    hint: "Fast-forward only; never destroys anything.",
  },
  {
    value: "mirror",
    label: "mirror",
    hint: "Shallow, forced to the tip; local changes are discarded.",
  },
];

const DEFAULT_INTERVAL = choiceOf(DEFAULT_INTERVAL_SECONDS).value;
const FIELD = "flex flex-col gap-1";
const LABEL = "text-xs font-medium text-muted";
const INPUT =
  "h-7 min-w-0 rounded-control border border-border bg-surface-raised px-2 text-sm text-text placeholder:text-faint disabled:opacity-50 focus:outline-2 focus:-outline-offset-1 focus:outline-focus read-only:text-muted";

interface AddFormProps {
  api: Pick<TetherApi, "pickFolder" | "branchOf" | "add">;
  onDone: () => void;
  onCancel: () => void;
  /** Told while an add runs, so the header can draw its progress hairline. */
  onPendingChange?: (isPending: boolean) => void;
}

/**
 * *Add repository*: folder, branch (the one checked out), mode, how often to pull, a clone URL
 * for a mirror, and, behind *Add commands…*, the repo's `gate` and `after` (ADR 0004).
 */
export function AddForm({ api, onDone, onCancel, onPendingChange }: AddFormProps) {
  const [path, setPath] = useState("");
  const [branch, setBranch] = useState("");
  const [mode, setMode] = useState<Mode>("follow");
  const [url, setUrl] = useState("");
  const [interval, setInterval] = useState(DEFAULT_INTERVAL);
  const [hasCommands, setHasCommands] = useState(false);
  const [gate, setGate] = useState("");
  const [after, setAfter] = useState("");
  const [checkedOut, setCheckedOut] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setSubmitting] = useState(false);
  const modeLabel = useId();
  const intervalLabel = useId();

  useEffect(() => onPendingChange?.(isSubmitting), [isSubmitting, onPendingChange]);

  const choose = async () => {
    setError(null);
    try {
      const picked = await api.pickFolder();
      if (picked === null) return;
      setPath(picked);
      const checkedOut = await api.branchOf(picked);
      setCheckedOut(checkedOut);
      setBranch(checkedOut ?? "main");
    } catch (e) {
      setError(String(e));
    }
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const request: AddRequest = {
      path,
      branch: branch.trim(),
      mode,
      url: mode === "mirror" && url.trim() ? url.trim() : null,
      interval: interval === DEFAULT_INTERVAL ? null : interval,
      gate: gate.trim() || null,
      after: after.trim() || null,
    };
    setSubmitting(true);
    setError(null);
    try {
      await api.add(request);
      onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  const hasFolder = path !== "";
  const isMirror = mode === "mirror";
  const name = path.split("/").filter(Boolean).pop() ?? path;

  return (
    <form
      aria-label="Add repository"
      onSubmit={submit}
      className="flex min-h-0 flex-1 flex-col"
    >
      <div
        className={`flex min-h-0 flex-1 flex-col overflow-y-auto p-4 ${isMirror ? "gap-3" : "gap-4"}`}
      >
        <Field.Root className={FIELD} disabled={isSubmitting}>
          <Field.Label className={LABEL}>Folder</Field.Label>
          <div className="flex gap-2">
            <Field.Control
              readOnly
              value={path}
              placeholder="No folder chosen"
              className={`flex-1 ${INPUT}`}
            />
            <TextButton
              variant={hasFolder ? "quiet" : "primary"}
              onClick={choose}
              disabled={isSubmitting}
            >
              Choose…
            </TextButton>
          </div>
        </Field.Root>

        <Field.Root className={FIELD} disabled={!hasFolder || isSubmitting}>
          <Field.Label className={LABEL}>Branch</Field.Label>
          <Field.Control
            value={branch}
            onValueChange={setBranch}
            placeholder="filled from the folder"
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            className={INPUT}
          />
          {checkedOut !== null && branch === checkedOut && (
            <span className="text-xs text-faint">checked out now</span>
          )}
        </Field.Root>

        {!hasFolder && (
          <p className="text-xs text-muted">
            tether reads the branch the clone has checked out. You pick how it
            keeps it there next.
          </p>
        )}

        {hasFolder && (
          <div className={FIELD}>
            <span id={modeLabel} className={LABEL}>
              Mode
            </span>
            <RadioGroup
              aria-labelledby={modeLabel}
              value={mode}
              disabled={isSubmitting}
              onValueChange={(value) => setMode(value as Mode)}
              className="flex flex-col gap-2"
            >
              {MODES.map((m) => (
                <ModeOption key={m.value} {...m} checked={mode === m.value} />
              ))}
            </RadioGroup>
          </div>
        )}

        {hasFolder && (
          <div className={FIELD}>
            <span id={intervalLabel} className={LABEL}>
              Pull every
            </span>
            <IntervalSelect
              labelledBy={intervalLabel}
              choices={[...PRESETS]}
              value={interval}
              onValueChange={setInterval}
              disabled={isSubmitting}
            />
          </div>
        )}

        {isMirror && hasFolder && (
          <Field.Root className={FIELD} disabled={isSubmitting}>
            <Field.Label className={LABEL}>Clone URL</Field.Label>
            <Field.Control
              type="url"
              value={url}
              onValueChange={setUrl}
              placeholder="https://example.com/team/repo.git"
              spellCheck={false}
              className={INPUT}
            />
            <Field.Description className="text-xs text-muted">
              Only used when the folder is empty.
            </Field.Description>
          </Field.Root>
        )}

        {hasFolder && !hasCommands && (
          <div>
            <TextButton onClick={() => setHasCommands(true)} disabled={isSubmitting}>
              Add commands…
            </TextButton>
          </div>
        )}

        {hasFolder && hasCommands && (
          <>
            <Field.Root className={FIELD} disabled={isSubmitting}>
              <Field.Label className={LABEL}>Gate</Field.Label>
              <Field.Control
                value={gate}
                onValueChange={setGate}
                placeholder="optional"
                spellCheck={false}
                autoCapitalize="off"
                autoCorrect="off"
                className={`font-mono ${INPUT}`}
              />
              <Field.Description className="text-xs text-muted">
                Asked before a background sync; exit 0 lets it run.
              </Field.Description>
            </Field.Root>
            <Field.Root className={FIELD} disabled={isSubmitting}>
              <Field.Label className={LABEL}>After</Field.Label>
              <Field.Control
                value={after}
                onValueChange={setAfter}
                placeholder="optional"
                spellCheck={false}
                autoCapitalize="off"
                autoCorrect="off"
                className={`font-mono ${INPUT}`}
              />
              <Field.Description className="text-xs text-muted">
                Runs in the clone after a sync moved the tip.
              </Field.Description>
            </Field.Root>
          </>
        )}

        {hasFolder && (
          <div className="rounded-control border border-dashed border-border">
            <RepoLine preview name={name} branch={branch} mode={mode} />
          </div>
        )}

        {error && (
          <div className="mt-auto">
            <Callout
              tone="needs-you"
              role="alert"
              heading={
                <>
                  <NeedsYouIcon className="shrink-0 text-needs-you" />
                  <span className="font-medium">Not added</span>
                </>
              }
            >
              <p className="text-needs-you">{error}</p>
            </Callout>
          </div>
        )}
      </div>

      <div className="flex justify-end gap-2 border-t border-border px-4 py-3">
        <TextButton onClick={onCancel} disabled={isSubmitting}>
          Cancel
        </TextButton>
        <TextButton
          type="submit"
          variant="primary"
          disabled={!hasFolder || !branch.trim() || isSubmitting}
        >
          {isSubmitting ? "Adding…" : "Add"}
        </TextButton>
      </div>
    </form>
  );
}
