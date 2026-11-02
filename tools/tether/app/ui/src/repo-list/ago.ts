const MINUTE = 60;
const HOUR = 3600;
const DAY = 86_400;

/** "3 min ago" style age of an RFC 3339 time, as the tray has always shown it. */
export function ago(rfc3339: string | undefined, now: Date): string {
  const then = rfc3339 ? Date.parse(rfc3339) : Number.NaN;
  if (Number.isNaN(then)) return "never";
  const seconds = Math.max(0, Math.floor((now.getTime() - then) / 1000));
  if (seconds < MINUTE) return "just now";
  if (seconds < HOUR) return `${Math.floor(seconds / MINUTE)} min ago`;
  if (seconds < DAY) return `${Math.floor(seconds / HOUR)} h ago`;
  return `${Math.floor(seconds / DAY)} d ago`;
}
