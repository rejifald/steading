import type { Repo } from "../api";

/** The notes worth a line: a repository with no linked worktrees, and why `gh` was not used. */
export function repoNotesOf(repos: Repo[]): string[] {
  return repos.flatMap((repo) => {
    const notes: string[] = [];
    if (repo.worktrees.length === 0 && !repo.error) notes.push(`${repo.name} · no linked worktrees`);
    if (repo.gh_note) notes.push(`${repo.name} · ${repo.gh_note}`);
    return notes;
  });
}

/** The faint "Repositories" list at the end of the scroll; nothing when there is nothing to say. */
export function RepoNotes({ repos }: { repos: Repo[] }) {
  const notes = repoNotesOf(repos);
  if (notes.length === 0) return null;
  return (
    <section aria-label="Repositories" className="pb-3">
      <h2 className="px-4 pt-4 pb-1 text-xs font-semibold text-muted">Repositories</h2>
      <ul>
        {notes.map((note) => (
          <li key={note} className="px-4 text-xs text-faint">
            {note}
          </li>
        ))}
      </ul>
    </section>
  );
}
