import type { Row } from "../api";

/** *Needs you* rows first; otherwise the order of repos.toml. */
export function sortRows(rows: readonly Row[]): Row[] {
  const needsYou = rows.filter((r) => r.health === "needs-you");
  return [...needsYou, ...rows.filter((r) => r.health !== "needs-you")];
}
