import type { KeyboardEvent, ReactNode } from "react";
import { useState } from "react";

import { TextButton } from "@steading/tray-popover/controls/TextButton";

import type { Repo, Salvaged } from "../api";
import { ago, inTime } from "./ago";
import { OutcomeSection } from "./OutcomeSection";

interface Entry {
  repo: Repo;
  salvaged: Salvaged;
}

/** Every save of one kind across repos, newest first. */
export function salvagedEntriesOf(repos: Repo[], kind: Salvaged["kind"] = "merged"): Entry[] {
  return repos
    .flatMap((repo) => (repo.salvaged ?? []).map((salvaged) => ({ repo, salvaged })))
    .filter(({ salvaged }) => (salvaged.kind ?? "merged") === kind)
    .sort((a, b) => Date.parse(b.salvaged.saved_at) - Date.parse(a.salvaged.saved_at));
}

/** The branch it was saved from, else the folder the ref is named for. */
export function salvagedTitleOf(salvaged: Salvaged): string {
  return salvaged.branch ?? salvaged.refname.split("/").at(-2) ?? salvaged.refname;
}

function SavedRow({ entry, line, children }: { entry: Entry; line: string; children?: ReactNode }) {
  const { repo, salvaged } = entry;
  return (
    <li aria-label={salvagedTitleOf(salvaged)} className="flex flex-col gap-0.5 px-4 py-1.5 text-sm">
      <div className="flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate font-medium text-text">{salvagedTitleOf(salvaged)}</span>
        <span className="shrink-0 text-xs text-faint">{repo.name}</span>
      </div>
      <p className="truncate text-xs text-muted">{line}</p>
      <code className="block text-xs break-all text-faint select-all">{salvaged.restore}</code>
      {children}
    </li>
  );
}

/** The uncommitted changes reap saved before it removed merged worktrees, each with the commands
 * that bring them back. */
export function SalvagedSection({ repos, now }: { repos: Repo[]; now: Date }) {
  const entries = salvagedEntriesOf(repos, "merged");
  // The dot takes the quiet tone of Kept: nothing here needs doing.
  return (
    <OutcomeSection outcome="kept" title="Salvaged" count={entries.length} collapsible summary="changes saved to refs">
      {entries.map((entry) => {
        const deletes = entry.salvaged.prunes_at ? ` · deleted ${inTime(entry.salvaged.prunes_at, now)}` : "";
        return (
          <SavedRow
            key={`${entry.repo.path}:${entry.salvaged.refname}`}
            entry={entry}
            line={`Saved ${ago(entry.salvaged.saved_at, now)}${deletes}`}
          />
        );
      })}
    </OutcomeSection>
  );
}

interface DeleteSaveProps {
  entry: Entry;
  onDelete: (repo: string, refname: string) => Promise<boolean> | void;
}

/** *Delete*, then a confirm: the save may be the only copy of the work. */
function DeleteSave({ entry, onDelete }: DeleteSaveProps) {
  const [isConfirming, setIsConfirming] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const cancelOnEscape = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    setIsConfirming(false);
  };
  if (!isConfirming) {
    return (
      <div className="flex gap-2 pt-1">
        <TextButton variant="inset" size="compact" onClick={() => setIsConfirming(true)}>
          Delete
        </TextButton>
      </div>
    );
  }
  const confirm = async () => {
    setIsDeleting(true);
    await onDelete(entry.repo.path, entry.salvaged.refname);
    setIsDeleting(false);
    setIsConfirming(false);
  };
  return (
    <div role="group" aria-label="Confirm delete" onKeyDown={cancelOnEscape} className="flex items-center gap-2 pt-1">
      <span className="text-xs text-muted">Its work exists nowhere else.</span>
      <TextButton variant="inset" size="compact" disabled={isDeleting} onClick={() => void confirm()}>
        {isDeleting ? "Deleting…" : "Delete for good"}
      </TextButton>
      <TextButton variant="inset" size="compact" autoFocus disabled={isDeleting} onClick={() => setIsConfirming(false)}>
        Cancel
      </TextButton>
    </div>
  );
}

interface SavedAbandonedSectionProps {
  repos: Repo[];
  now: Date;
  onDelete: (repo: string, refname: string) => Promise<boolean> | void;
}

/** What *Save and remove* saved from abandoned worktrees (ADR 0006): kept until the owner deletes
 * each, apart from the merged saves that expire. */
export function SavedAbandonedSection({ repos, now, onDelete }: SavedAbandonedSectionProps) {
  const entries = salvagedEntriesOf(repos, "abandoned");
  return (
    <OutcomeSection
      outcome="kept"
      title="Saved abandoned work"
      count={entries.length}
      collapsible
      summary="kept until you delete it"
    >
      {entries.map((entry) => (
        <SavedRow
          key={`${entry.repo.path}:${entry.salvaged.refname}`}
          entry={entry}
          line={`Saved ${ago(entry.salvaged.saved_at, now)} · kept until you delete it`}
        >
          <DeleteSave entry={entry} onDelete={onDelete} />
        </SavedRow>
      ))}
    </OutcomeSection>
  );
}
