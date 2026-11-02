import { Tooltip } from "@base-ui/react/tooltip";
import { useState, type FocusEvent } from "react";

import type { Row } from "../api";
import { FolderIcon, SettingsIcon, SyncIcon } from "../controls/icons";
import { IconButton } from "../controls/IconButton";
import { ProgressLine } from "../ProgressLine";
import { RepoLine } from "../RepoLine";
import { ago } from "./ago";

interface RepoRowProps {
  row: Row;
  /** The CLI's detail line (skip reason or commits to pull), if any. */
  detail: string | undefined;
  busy: boolean;
  /** tether is paused: the row leads with the pause glyph. */
  paused?: boolean;
  now: Date;
  onSync: (path: string) => void;
  onOpen: (path: string) => void;
  onSettings: (path: string) => void;
}

/** One listed repo as a single line; its actions and its detail show on hover and on focus inside it. */
export function RepoRow({ row, detail, busy, paused = false, now, onSync, onOpen, onSettings }: RepoRowProps) {
  const lastSync = `last sync ${ago(row.last_sync, now)}`;
  const description = detail ? `${detail} · ${lastSync}` : lastSync;
  const [isHovered, setHovered] = useState(false);
  // The detail also opens while a row's action has keyboard focus, not only on hover.
  const [isFocusWithin, setFocusWithin] = useState(false);
  const leave = (event: FocusEvent<HTMLElement>) => {
    if (!event.currentTarget.contains(event.relatedTarget)) setFocusWithin(false);
  };
  return (
    <Tooltip.Root open={isHovered || isFocusWithin} onOpenChange={setHovered}>
      <Tooltip.Trigger
        render={<li />}
        onFocus={() => setFocusWithin(true)}
        onBlur={leave}
        aria-label={row.name}
        aria-description={description}
        className={`group relative rounded-control text-sm text-text ${busy ? "bg-behind/10" : "hover:bg-surface-hover focus-within:bg-surface-hover"}`}
      >
        <RepoLine
          name={row.name}
          branch={row.branch}
          mode={row.mode}
          health={row.health}
          behind={row.behind}
          busy={busy}
          paused={paused}
          title={row.path}
        >
          <div className="flex w-0 shrink-0 gap-1 overflow-hidden opacity-0 group-hover:w-auto group-hover:overflow-visible group-hover:opacity-100 group-focus-within:w-auto group-focus-within:overflow-visible group-focus-within:opacity-100">
            <IconButton label="Sync now" busy={busy} onClick={() => onSync(row.path)}>
              <SyncIcon className={busy ? "animate-spin" : undefined} />
            </IconButton>
            <IconButton label="Open in Finder" onClick={() => onOpen(row.path)}>
              <FolderIcon />
            </IconButton>
            <IconButton label="Settings" onClick={() => onSettings(row.path)}>
              <SettingsIcon />
            </IconButton>
          </div>
        </RepoLine>
        {busy && <ProgressLine inset />}
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Positioner side="bottom" align="start" sideOffset={4}>
          <Tooltip.Popup className="rounded-control border border-border bg-surface-raised px-2 py-1 text-xs text-text shadow-raised">
            {description}
          </Tooltip.Popup>
        </Tooltip.Positioner>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}
