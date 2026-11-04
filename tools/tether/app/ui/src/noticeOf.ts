interface NoticeSources {
  /** A click that failed here, until the next one. */
  actionError: string | null;
  /** The app's last failed action (Pause, a sync), until the next refresh. */
  note: string | null;
  /** repos.toml could not be read but an earlier report is still shown. */
  staleError: string | null;
}

/** The notice strip's text, first match wins; null when all is well. */
export function noticeOf({ actionError, note, staleError }: NoticeSources): string | null {
  return actionError ?? note ?? (staleError ? `Cannot read repos.toml: ${staleError}. Showing the last report.` : null);
}
