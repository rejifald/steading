import type { Discovery, RootInfo, Row, Suggestion } from "./api";
import { minutesAgo, row, suggestion, type FakeOptions } from "./api.fake";

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

/* Clone discovery (ADR 0004), `docs/design/discovery/reference/`: one entry per panel and edge. */

const DEV = "~/Development";
const CODE = "~/Code";

const root = (path: string, clones: number | null, fields: Partial<RootInfo> = {}): RootInfo => ({
  path,
  clones,
  stopped_early: false,
  error: null,
  ...fields,
});

const cache = suggestion("cache");
const queue = suggestion("queue");
const search = suggestion("search", DEV, "develop");
const gateway = suggestion("gateway");
const notes = suggestion("notes", CODE);
const wiki = suggestion("wiki", CODE);
const six: Suggestion[] = [cache, queue, search, gateway, notes, wiki];

/** A finished scan of the two default roots. */
const scanned = (suggestions: Suggestion[], minutes = 0, roots?: RootInfo[]): Partial<Discovery> => ({
  scanned_at: minutesAgo(minutes, now),
  scanning: null,
  roots: roots ?? [
    root(DEV, suggestions.filter((s) => s.root === DEV).length),
    root(CODE, suggestions.filter((s) => s.root === CODE).length),
  ],
  suggestions,
});

const listed: Row[] = ["example-app", "example-api", "example-handbook", "example-infra"].map((name) => inSync(name));

const skippedClones = [
  { path: `${DEV}/scratch`, name: "scratch", root: DEV },
  { path: `${CODE}/old-experiments`, name: "old-experiments", root: CODE },
];

const discovery: Record<string, FakeOptions> = {
  /** Panel 1, idle: the invitation, nothing scanned. */
  invite: { rows: [] },
  "invite-no-roots": { rows: [], discovery: { roots: [] } },
  "invite-many-roots": {
    rows: [],
    discovery: { roots: [DEV, CODE, "~/Projects", "~/src"].map((path) => root(path, null)) },
  },
  /** Panel 2: partway through, the second root still being walked. */
  looking: {
    rows: [],
    discovery: {
      scanning: CODE,
      roots: [root(DEV, 3), root(CODE, null)],
      suggestions: [cache, queue],
    },
  },
  /** Panel 3: a finished scan with nothing listed. */
  found: { rows: [], discovery: scanned(six) },
  /** Panel 4 once one suggestion is skipped (its play step): two listed, four still found. */
  triage: {
    rows: [
      row({ name: "example-api", path: dev("example-api"), last_sync: minutesAgo(1, now) }),
      row({ name: "example-notebook", path: dev("example-notebook") }),
    ],
    busy: [dev("example-notebook")],
    discovery: scanned([cache, queue, search, notes, wiki]),
  },
  /** Panel 5: repos listed, two suggestions left unanswered three days ago. */
  later: { rows: listed, discovery: scanned([notes, wiki], 3 * 24 * 60) },
  /** Panel 6: looked again, every clone found is listed or skipped. */
  "looked-again": {
    rows: listed,
    discovery: scanned([], 0, [root(DEV, 4), root(CODE, 2)]),
  },
  /** Panel 1, settled: the scan found no clone at all. */
  "no-suggestions": { rows: listed, discovery: scanned([], 0, [root(DEV, 0), root(CODE, 0)]) },
  /** Panel 7: first look found nothing; one root stopped at the folder cap. */
  "found-nothing": {
    rows: [],
    discovery: scanned([], 0, [root(DEV, 0), root(CODE, 0, { stopped_early: true })]),
  },
  /** Panel 8: one folder unreadable, the rest shows. */
  "found-error": {
    rows: [],
    discovery: scanned([cache, queue], 0, [
      root(DEV, 2),
      root(CODE, 0),
      root("~/work", null, { error: "permission denied" }),
    ]),
  },
  /** Panel 9: folders and skipped clones (open it from Found). */
  "where-to-look": { rows: [], discovery: { ...scanned(six), skipped: skippedClones } },
  "where-to-look-no-roots": { rows: [], discovery: { roots: [], skipped: skippedClones } },
  "found-long-names": {
    rows: [],
    discovery: scanned([
      suggestion("example-monorepo-with-a-really-long-name", DEV, "release/2026-11-campaign-and-tether"),
      suggestion("example-infra-terraform-modules-and-more", "~/Code/clients/example-agency"),
      cache,
    ]),
  },
  /** *Add* is refused (as `add` does for a branch the remote lacks): the row stays, the strip says why. */
  "add-fails": {
    rows: [],
    discovery: scanned(six),
    suggestionAddFails: `${DEV}/search: origin has no branch develop`,
  },
  /** *Add* never finishes: the row stays, *Add* disabled. */
  "add-pending": { rows: [], discovery: scanned(six), pendingActions: true },
};

export const SCENARIOS: Record<string, FakeOptions> = {
  ...discovery,
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
