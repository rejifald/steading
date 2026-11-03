import { useEffect, useId, useState, type FormEvent, type ReactNode } from "react";

import type { Row, TetherApi } from "../api";
import { Callout } from "@steading/tray-popover/Callout";
import { WarningIcon } from "@steading/tray-popover/controls/icons";
import { TextButton } from "@steading/tray-popover/controls/TextButton";
import { IntervalSelect } from "../interval/IntervalSelect";
import { choiceOf, choicesFor } from "../interval/intervals";
import { ago } from "../repo-list/ago";

const FIELD = "flex flex-col gap-1";
const LABEL = "text-xs font-medium text-muted";

/** The error callout's title per pending action. */
const ERROR_TITLES = { save: "Not saved", remove: "Not removed" } as const;

type Action = keyof typeof ERROR_TITLES;

interface RepoSettingsProps {
  row: Row;
  /** The CLI's reason, for a repo that needs you. */
  detail: string | undefined;
  now: Date;
  api: Pick<TetherApi, "setInterval" | "remove">;
  /** After a save or a removal; the next report shows the change. */
  onDone: () => void;
  onCancel: () => void;
  /** Told while a save or removal runs, so the header can draw its progress hairline. */
  onPendingChange?: (isPending: boolean) => void;
}

/** One listed repo's settings: its facts, how often it is pulled, and *Remove from tether*. */
export function RepoSettings({ row, detail, now, api, onDone, onCancel, onPendingChange }: RepoSettingsProps) {
  const listed = choiceOf(row.interval_seconds).value;
  const [interval, setInterval] = useState(listed);
  const [isConfirmingRemove, setConfirmingRemove] = useState(false);
  const [error, setError] = useState<{ action: Action; text: string } | null>(null);
  const [pending, setPending] = useState<Action | null>(null);
  const intervalLabel = useId();
  const isPending = pending !== null;

  useEffect(() => onPendingChange?.(isPending), [isPending, onPendingChange]);

  const perform = async (action: Action, run: () => Promise<void>) => {
    setPending(action);
    setError(null);
    try {
      await run();
      onDone();
    } catch (e) {
      setError({ action, text: String(e) });
    } finally {
      setPending(null);
    }
  };

  const save = (event: FormEvent) => {
    event.preventDefault();
    void perform("save", () => api.setInterval(row.path, interval));
  };

  return (
    <form aria-label={`${row.name} settings`} onSubmit={save} className="flex min-h-0 flex-1 flex-col">
      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
        {row.health === "needs-you" && (
          <Callout
            tone="needs-you"
            aria-label={`${row.name} needs you`}
            heading={
              <>
                <WarningIcon className="shrink-0 text-needs-you" />
                <span className="font-medium">needs you</span>
                <span className="ml-auto shrink-0 text-xs text-muted">{ago(row.last_sync, now)}</span>
              </>
            }
          >
            {detail ? <p className="text-needs-you">{detail}</p> : undefined}
          </Callout>
        )}

        <dl className="flex flex-col">
          <Fact label="Folder" title={row.path}>
            {row.path}
          </Fact>
          <Fact label="Branch">
            {row.branch} <span className="text-faint">· {row.mode}</span>
          </Fact>
          <Fact label="Last sync">{ago(row.last_sync, now)}</Fact>
          {row.gate && (
            <Fact label="Gate" title={row.gate} isCode>
              {row.gate}
            </Fact>
          )}
          {row.after && (
            <Fact label="After" title={row.after} isCode>
              {row.after}
            </Fact>
          )}
        </dl>

        <div className={FIELD}>
          <span id={intervalLabel} className={LABEL}>
            Pull every
          </span>
          <IntervalSelect
            labelledBy={intervalLabel}
            choices={choicesFor(row.interval_seconds)}
            value={interval}
            onValueChange={setInterval}
            disabled={isPending}
          />
        </div>

        <div className={`flex flex-col gap-2 ${isConfirmingRemove ? "" : "border-t border-border pt-4"}`}>
          {isConfirmingRemove ? (
            <Callout
              tone="plain"
              heading={
                <span className="font-medium">
                  Stop keeping {row.name} on {row.branch}?
                </span>
              }
            >
              <p className="text-muted">The clone stays as it is.</p>
              <div className="flex gap-2">
                <TextButton
                  variant="inset"
                  onClick={() => void perform("remove", () => api.remove(row.path))}
                  disabled={isPending}
                >
                  {pending === "remove" ? "Removing…" : "Remove"}
                </TextButton>
                <TextButton variant="inset" onClick={() => setConfirmingRemove(false)} disabled={isPending}>
                  Keep
                </TextButton>
              </div>
            </Callout>
          ) : (
            <>
              <TextButton className="self-start" onClick={() => setConfirmingRemove(true)} disabled={isPending}>
                Remove from tether…
              </TextButton>
              <span className="text-xs text-muted">The clone is left as it is.</span>
            </>
          )}
        </div>

        {error && (
          <div className="mt-auto">
            <Callout
              tone="needs-you"
              role="alert"
              heading={
                <>
                  <WarningIcon className="shrink-0 text-needs-you" />
                  <span className="font-medium">{ERROR_TITLES[error.action]}</span>
                </>
              }
            >
              <p className="text-needs-you">{error.text}</p>
            </Callout>
          </div>
        )}
      </div>

      <div className="flex justify-end gap-2 border-t border-border px-4 py-3">
        <TextButton onClick={onCancel} disabled={isPending}>
          Cancel
        </TextButton>
        <TextButton type="submit" variant="primary" disabled={interval === listed || isPending}>
          {pending === "save" ? "Saving…" : "Save"}
        </TextButton>
      </div>
    </form>
  );
}

interface FactProps {
  label: string;
  /** The full value on hover, for one that truncates. */
  title?: string;
  /** A shell command: monospace. */
  isCode?: boolean;
  children: ReactNode;
}

/** One read-only fact as a label and a value on one line. */
function Fact({ label, title, isCode = false, children }: FactProps) {
  return (
    <div className="flex h-7 items-center gap-2">
      <dt className="w-18 shrink-0 text-xs font-medium text-muted">{label}</dt>
      <dd className={`min-w-0 truncate text-sm text-text ${isCode ? "font-mono" : ""}`} title={title}>
        {children}
      </dd>
    </div>
  );
}
