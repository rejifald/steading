import type { Row } from "./api";
import { minutesAgo, row, type FakeOptions } from "./api.fake";

/**
 * The popover's states as drawn in `docs/design/popover/reference/`, for `npm run dev`:
 * open `/?scenario=<name>` to render one. Dev-only; the app never loads this file.
 */
const now = new Date();
const dev = (name: string) => `~/Development/${name}`;

const SITE_REASON = "on another branch (feature/new-header)";
const HANDBOOK_REASON = "after failed: reconcile: 2 links point outside the clone";

const site = row({
  name: "example-site",
  path: dev("example-site"),
  branch: "release",
  health: "needs-you",
  reason: { kind: "other-branch" },
  hint: "Switch back to release and tether resumes.",
  last_sync: minutesAgo(65, now),
});
const handbookNeedsYou = row({
  name: "example-handbook",
  path: dev("example-handbook"),
  health: "needs-you",
  reason: { kind: "after-failed" },
  hint: "Fix what the after command reports, then Sync now to run it again.",
  gate: 'test -z "$(git status --porcelain)"',
  after: "./scripts/reconcile.sh",
  interval_seconds: 90,
  last_sync: minutesAgo(5, now),
});
const cliNeedsYou = row({
  name: "example-cli",
  path: dev("example-cli"),
  health: "needs-you",
  reason: { kind: "detached" },
  hint: "Check out main and tether resumes.",
  last_sync: minutesAgo(20, now),
});

const inSync = (name: string, minutes = 2) =>
  row({ name, path: dev(name), last_sync: minutesAgo(minutes, now) });
const api = row({ name: "example-api", path: dev("example-api"), health: "behind", behind: 3, last_sync: minutesAgo(4, now) });
const docs = row({
  name: "example-docs",
  path: "~/Mirrors/example-docs",
  mode: "mirror",
  health: "skipped",
  reason: { kind: "busy" },
  last_sync: minutesAgo(12, now),
});

const baseDetails: Record<string, string> = {
  [dev("example-api")]: "3 commit(s) to pull on the next sync",
  "~/Mirrors/example-docs": "another tether run is syncing it; retrying next round",
  [dev("example-site")]: SITE_REASON,
  [dev("example-handbook")]: HANDBOOK_REASON,
  [dev("example-cli")]: "HEAD is detached",
};

/** The seven repos of the round 3 boards: one needing you, one behind, one skipped. */
const regular: Row[] = [
  site,
  inSync("example-app"),
  api,
  docs,
  inSync("example-cli"),
  inSync("example-infra"),
  inSync("example-handbook"),
];

const nine: Row[] = [
  site,
  handbookNeedsYou,
  cliNeedsYou,
  inSync("example-app"),
  api,
  docs,
  inSync("example-infra"),
  inSync("example-design"),
  inSync("example-billing"),
];

const fifteen: Row[] = [
  site,
  api,
  docs,
  ...[
    "cli",
    "infra",
    "handbook",
    "design",
    "billing",
    "search",
    "mobile",
    "web",
    "auth",
    "worker",
    "ops",
    "data",
  ].map((name) => inSync(`example-${name}`)),
];

const longNames: Row[] = [
  {
    ...site,
    name: "example-site-marketing-2026",
    branch: "release/2026-11-campaign-and-tether",
    last_sync: minutesAgo(3 * 24 * 60, now),
  },
  inSync("example-monorepo-with-a-really-long-name"),
  { ...api, branch: "feature/2026-q4-refactor-of-everything" },
  docs,
  inSync("example-cli"),
  inSync("example-infra-terraform-modules-and-more"),
];

const allInSync = regular.map((r) => ({ ...r, health: "in-sync" as const, last_sync: minutesAgo(2, now) }));

export const SCENARIOS: Record<string, FakeOptions> = {
  regular: { rows: regular, details: baseDetails },
  syncing: { rows: regular, details: baseDetails, busy: [dev("example-api"), "~/Mirrors/example-docs"] },
  "all-in-sync": { rows: allInSync, details: {} },
  paused: { rows: regular, details: baseDetails, paused: true },
  loading: { isLoading: true },
  empty: { rows: [] },
  error: { rows: [], error: "expected `=` at line 4, column 9" },
  stacked: { rows: nine, details: baseDetails },
  "long-names": {
    rows: longNames,
    details: {
      ...baseDetails,
      [dev("example-site")]: "on another branch (feature/new-header-with-a-very-long-descriptive-branch-name)",
    },
  },
  scrolled: { rows: fifteen, details: baseDetails },
  "two-need-you-syncing": {
    rows: [site, handbookNeedsYou, inSync("example-app"), api, docs, inSync("example-infra"), inSync("example-design")],
    details: baseDetails,
    busy: [dev("example-site"), dev("example-api")],
  },
  "action-failed": {
    rows: regular,
    details: baseDetails,
    note: "~/Development/example-api: git fetch: exit status 128",
  },
  "paused-needs-you": { rows: regular, details: baseDetails, paused: true },
  "stale-error": {
    rows: regular,
    details: baseDetails,
    error: "expected `=` at line 4, column 9",
    hasReport: true,
  },
  "settings-needs-you": {
    rows: [handbookNeedsYou, inSync("example-app"), api],
    details: baseDetails,
  },
  "reason-changed": {
    rows: nine.map((r) =>
      r === site
        ? { ...site, reason: { kind: "detached" }, hint: "Check out release and tether resumes.", last_sync: minutesAgo(0, now) }
        : r,
    ),
    details: { ...baseDetails, [dev("example-site")]: "HEAD is detached" },
  },
  adding: { rows: regular, details: baseDetails, pendingActions: true },
  "add-refused": {
    rows: [...regular, inSync("example-new")],
    details: baseDetails,
  },
  refused: {
    rows: regular,
    details: baseDetails,
    rejectWith: "~/Development/example-api is no longer listed",
  },
  "add-long-path": {
    rows: regular,
    details: baseDetails,
    pickedFolder: "~/Development/clients/example-agency/example-campaign-2026-landing",
    checkedOutBranch: "release/2026-11-campaign-landing",
  },
  "add-branch-not-read": { rows: regular, details: baseDetails, checkedOutBranch: null },
  "long-settings": {
    rows: [
      row({
        name: "example-campaign-2026-landing-page",
        path: "~/Development/clients/example-agency/example-campaign-2026-landing-page",
        branch: "release/2026-11-campaign-landing",
        health: "behind",
        behind: 12,
      }),
    ],
    details: {},
  },
};
