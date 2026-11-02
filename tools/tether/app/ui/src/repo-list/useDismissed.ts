import { useCallback, useEffect, useState } from "react";

import type { Row } from "../api";

/** `localStorage` key: per listed path, the reason text the owner dismissed. */
export const DISMISSED_KEY = "tether.dismissed";

type Dismissals = Record<string, string>;

function isDismissals(value: unknown): value is Dismissals {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    Object.values(value).every((reason) => typeof reason === "string")
  );
}

function load(): Dismissals {
  // Storage that cannot be read (blocked, corrupt) means nothing is dismissed; the callouts
  // simply show, which is the safe side.
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(DISMISSED_KEY) ?? "{}");
    return isDismissals(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

function save(dismissals: Dismissals) {
  // A failed write only means the dismissal does not outlive this session.
  try {
    localStorage.setItem(DISMISSED_KEY, JSON.stringify(dismissals));
  } catch {
    return;
  }
}

const reasonOf = (details: Record<string, string> | undefined, path: string) => details?.[path] ?? "";

/**
 * The callouts the owner dismissed, kept until the reason text changes or the repo stops
 * needing them. `rows` is `null` while there is no report, which keeps every dismissal.
 */
export function useDismissed(
  rows: readonly Row[] | null | undefined,
  details: Record<string, string> | undefined,
) {
  const [dismissed, setDismissed] = useState<Dismissals>(load);

  useEffect(() => {
    if (!rows) return;
    setDismissed((current) => {
      const kept = Object.entries(current).filter(([path, reason]) =>
        rows.some(
          (row) => row.path === path && row.health === "needs-you" && reasonOf(details, path) === reason,
        ),
      );
      return kept.length === Object.keys(current).length ? current : Object.fromEntries(kept);
    });
  }, [rows, details]);

  useEffect(() => save(dismissed), [dismissed]);

  const isDismissed = useCallback(
    (path: string) => {
      const row = rows?.find((r) => r.path === path);
      return row?.health === "needs-you" && dismissed[path] === reasonOf(details, path);
    },
    [rows, details, dismissed],
  );

  const dismiss = useCallback(
    (path: string) => setDismissed((current) => ({ ...current, [path]: reasonOf(details, path) })),
    [details],
  );

  return { isDismissed, dismiss };
}
