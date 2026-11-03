import type { Row } from "../api";
import { Callout } from "@steading/tray-popover/Callout";
import { IconButton } from "@steading/tray-popover/controls/IconButton";
import { CrossIcon, WarningIcon, SlidersIcon } from "@steading/tray-popover/controls/icons";
import { TextButton } from "@steading/tray-popover/controls/TextButton";
import { ago } from "./ago";

/** Reasons only a sync can clear, so Sync now leads (tether-core `report::is_remote_side`). */
const CLEARED_BY_SYNC = new Set(["fetch-failed", "no-remote-branch", "fast-forward-refused"]);

export interface NeedsYouCalloutProps {
  row: Row;
  /** The CLI's reason this repo needs the user, shown verbatim. */
  detail: string | undefined;
  busy: boolean;
  now: Date;
  onSync: (path: string) => void;
  onOpen: (path: string) => void;
  onSettings: (path: string) => void;
  /** Hides the callout until its reason changes. */
  onDismiss: (path: string) => void;
}

/**
 * A needs-you repo lifted out of the list: what is wrong, what to do about it, and the two
 * actions, the one that helps first.
 */
export function NeedsYouCallout({
  row,
  detail,
  busy,
  now,
  onSync,
  onOpen,
  onSettings,
  onDismiss,
}: NeedsYouCalloutProps) {
  const syncButton = (
    <TextButton
      key="sync"
      variant="inset"
      disabled={busy}
      aria-busy={busy || undefined}
      onClick={() => onSync(row.path)}
    >
      {busy ? "Syncing…" : "Sync now"}
    </TextButton>
  );
  const openButton = (
    <TextButton key="open" variant="inset" onClick={() => onOpen(row.path)}>
      Open in Finder
    </TextButton>
  );
  const isClearedBySync = CLEARED_BY_SYNC.has(row.reason?.kind ?? "");
  return (
    <Callout
      tone="needs-you"
      aria-label={`${row.name} needs you`}
      heading={
        <>
          <WarningIcon className="shrink-0 text-needs-you" />
          <span className="truncate font-medium" title={row.path}>
            {row.name}
          </span>
          {/* The name keeps its room first; the branch gives way. */}
          <span className="shrink-2 truncate text-muted" title={row.path}>
            {row.branch} · {row.mode}
          </span>
          <span className="ml-auto shrink-0 text-xs text-muted">{ago(row.last_sync, now)}</span>
          <IconButton label="Dismiss until it changes" onClick={() => onDismiss(row.path)}>
            <CrossIcon />
          </IconButton>
        </>
      }
    >
      {detail ? <p className="text-needs-you">{detail}</p> : null}
      {row.hint ? <p className="text-text">{row.hint}</p> : null}
      <div className="flex gap-2">
        {isClearedBySync ? [syncButton, openButton] : [openButton, syncButton]}
        <div className="ml-auto self-center">
          <IconButton label="Settings" onClick={() => onSettings(row.path)}>
            <SlidersIcon />
          </IconButton>
        </div>
      </div>
    </Callout>
  );
}
