import type { Repo, Salvaged } from "../api";
import { ago, inTime } from "./ago";
import { OutcomeSection } from "./OutcomeSection";

interface Entry {
  repo: Repo;
  salvaged: Salvaged;
}

/** Every save across repos, newest first. */
export function salvagedEntriesOf(repos: Repo[]): Entry[] {
  return repos
    .flatMap((repo) => (repo.salvaged ?? []).map((salvaged) => ({ repo, salvaged })))
    .sort((a, b) => Date.parse(b.salvaged.saved_at) - Date.parse(a.salvaged.saved_at));
}

/** The branch it was saved from, else the folder the ref is named for. */
export function salvagedTitleOf(salvaged: Salvaged): string {
  return salvaged.branch ?? salvaged.refname.split("/").at(-2) ?? salvaged.refname;
}

/** The uncommitted changes reap saved before it removed their worktrees, each with the commands
 * that bring them back. */
export function SalvagedSection({ repos, now }: { repos: Repo[]; now: Date }) {
  const entries = salvagedEntriesOf(repos);
  // The dot takes the quiet tone of Kept: nothing here needs doing.
  return (
    <OutcomeSection outcome="kept" title="Salvaged" count={entries.length} collapsible summary="changes saved to refs">
      {entries.map(({ repo, salvaged }) => {
        const deletes = salvaged.prunes_at ? ` · deleted ${inTime(salvaged.prunes_at, now)}` : "";
        return (
          <li
            key={`${repo.path}:${salvaged.refname}`}
            aria-label={salvagedTitleOf(salvaged)}
            className="flex flex-col gap-0.5 px-4 py-1.5 text-sm"
          >
            <div className="flex items-center gap-2">
              <span className="min-w-0 flex-1 truncate font-medium text-text">{salvagedTitleOf(salvaged)}</span>
              <span className="shrink-0 text-xs text-faint">{repo.name}</span>
            </div>
            <p className="truncate text-xs text-muted">{`Saved ${ago(salvaged.saved_at, now)}${deletes}`}</p>
            <code className="block text-xs break-all text-faint select-all">{salvaged.restore}</code>
          </li>
        );
      })}
    </OutcomeSection>
  );
}
