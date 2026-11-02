/** Shown when nothing needs the owner and nothing is ready to remove. */
export function NothingToRemove() {
  return (
    <div className="px-4 py-6">
      <p className="text-base font-semibold text-text">Nothing to remove</p>
      <p className="text-sm text-muted">
        No worktree has merged since the last sweep. reap checks again on the next sweep.
      </p>
    </div>
  );
}
