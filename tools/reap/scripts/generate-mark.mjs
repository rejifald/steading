// Writes every git-reap mark from one geometry: the brand SVGs in docs/brand/ and the
// icon sources in app/icons/src/. scripts/render-icons.sh runs it before rendering.
//
//   node scripts/generate-mark.mjs
//
// The mark is a square slab in 2:1 isometric on a 100-unit tile, cut into four equal cubes.
// The back cube has been lifted straight out: it floats in blue above the notch it leaves,
// the worktree reap removes once its work has merged. Every cut is drawn as a gap between
// shapes, never a mask, because AppKit rasterises masks coarsely.
import { writeFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "..");

/**
 * The app icon and brand masters. `edge` is the slab's side and `cut` the gap between the
 * cubes, in tile units; each cube's side is what is left of the edge, halved. The lifted cube
 * floats one cut above the others, so every gap in the mark is the same.
 */
const MASTER = { edge: 29, cut: 3 };

/** The 18pt tray glyph: the same slab, its cut opened to about 2px at 36px. */
const TRAY = { edge: 29, cut: 3.6 };

/** The tray canvas: 36px (18pt at 2x), scaled so the slab's edge is exactly 16px. */
const TRAY_PX = 36;
const TRAY_EDGE_PX = 16;

const COLOUR = {
  shell: "#EAF1F6",
  blockTop: "#6CC6F7",
  blockLeft: "#2C9BDB",
  blockRight: "#1B6A9E",
  tileTop: "#172129",
  tileBottom: "#0A0F13",
  needsYouTop: "#F5894A",
  needsYouLeft: "#E8590C",
  needsYouRight: "#B3440A",
};

/**
 * Opacities of the shell and the block. A side plane is split between two cubes: the outer
 * cube takes the first value, the front cube the second.
 */
const TONE = {
  colour: { top: 0.95, left: [0.74, 0.6], right: [0.44, 0.32], block: [1, 1, 1] },
  // One colour is lit from the side, the top on the middle step: a template image is dark
  // on a light menu bar and bright on a dark one, so a top-lit shape looks lit from below.
  mono: { top: 0.38, left: [0.55, 0.47], right: [0.24, 0.19], block: [0.78, 1, 0.56] },
};

const n = (value) => Number(value.toFixed(3));
const pt = ([x, y]) => `${n(x)},${n(y)}`;

/**
 * Maps slab coordinates to the tile: x runs from the back vertex to the right, y to the left,
 * z up, in fractions of the edge; the slab's footprint is centred on (50,50).
 */
const project = (edge) => (x, y, z) => [50 + edge * (x - y), 50 + edge * ((x + y) / 2 - z)];

/** One polygon per visible face, each tagged with its face and its tone index. */
function shapes(g) {
  const P = project(g.edge);
  const cut = g.cut / g.edge;
  const side = (1 - cut) / 2;
  const [mid, near] = [side, side + cut];
  const face = (kind, tone, corners) => ({ kind, tone, points: corners.map((c) => P(...c)) });
  const top = (x0, x1, y0, y1, z) => [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]];
  const left = (x0, x1, y, z0, z1) => [[x0, y, z1], [x1, y, z1], [x1, y, z0], [x0, y, z0]];
  const right = (x, y0, y1, z0, z1) => [[x, y0, z1], [x, y1, z1], [x, y1, z0], [x, y0, z0]];
  const h = side;
  const [z0, z1] = [side + cut, 2 * side + cut];
  return [
    // The three cubes left standing: right, left, and front, back to front.
    face("top", 0, top(near, 1, 0, mid, h)),
    face("right", 0, right(1, 0, mid, 0, h)),
    face("top", 0, top(0, mid, near, 1, h)),
    face("left", 0, left(0, mid, 1, 0, h)),
    face("top", 0, top(near, 1, near, 1, h)),
    face("left", 1, left(near, 1, 1, 0, h)),
    face("right", 1, right(1, near, 1, 0, h)),
    // The lifted cube, straight above its notch.
    face("blockTop", 0, top(0, mid, 0, mid, z1)),
    face("blockLeft", 1, left(0, mid, mid, z0, z1)),
    face("blockRight", 2, right(mid, 0, mid, z0, z1)),
  ];
}

/** The mark's bounding box in tile units. */
function bounds(faces) {
  const xs = faces.flatMap((f) => f.points.map(([x]) => x));
  const ys = faces.flatMap((f) => f.points.map(([, y]) => y));
  return { x0: Math.min(...xs), x1: Math.max(...xs), y0: Math.min(...ys), y1: Math.max(...ys) };
}

/** Moves the faces so the whole mark, block included, is centred on (50,50). */
function centred(faces) {
  const { x0, x1, y0, y1 } = bounds(faces);
  const [dx, dy] = [50 - (x0 + x1) / 2, 50 - (y0 + y1) / 2];
  return faces.map((f) => ({ ...f, points: f.points.map(([x, y]) => [x + dx, y + dy]) }));
}

const polygon = (points, fill, opacity) =>
  `<polygon points="${points.map(pt).join(" ")}" fill="${fill}"${opacity < 1 ? ` fill-opacity="${opacity}"` : ""}/>`;

/** Fills every face; `fill` maps a face kind to a colour, `tone` to an opacity. */
function paint(g, fill, tone) {
  return centred(shapes(g))
    .map((f) => polygon(f.points, fill[f.kind], tone(f)))
    .join("");
}

/** The opacity of a face under a TONE set. */
const toneOf = (t) => (f) =>
  f.kind === "top" ? t.top : f.kind.startsWith("block") ? t.block[f.tone] : t[f.kind][f.tone];

function colourMark() {
  const fill = {
    top: COLOUR.shell,
    left: COLOUR.shell,
    right: COLOUR.shell,
    blockTop: COLOUR.blockTop,
    blockLeft: COLOUR.blockLeft,
    blockRight: COLOUR.blockRight,
  };
  return paint(MASTER, fill, toneOf(TONE.colour));
}

function monoMark(g, colour) {
  const fill = Object.fromEntries(
    ["top", "left", "right", "blockTop", "blockLeft", "blockRight"].map((k) => [k, colour]),
  );
  return paint(g, fill, toneOf(TONE.mono));
}

/** Removal pending: the three cubes as a wireframe round the notch, the lifted one filled. */
function wireframe(g) {
  const faces = centred(shapes(g));
  const corner = (i, j) => faces[i].points[j];
  // Corners by face (see shapes): 0 right top, 1 right side, 2 left top, 3 left side,
  // 4 front top, 5 front left side, 6 front right side.
  const outline = [
    corner(0, 0), corner(0, 1), corner(1, 3), corner(6, 2), corner(3, 3),
    corner(3, 0), corner(2, 0), corner(2, 1), corner(0, 3),
  ];
  const inner =
    `M${pt(corner(3, 0))} L${pt(corner(4, 2))} L${pt(corner(0, 1))} ` +
    `M${pt(corner(4, 2))} L${pt(corner(6, 2))}`;
  const block = faces
    .filter((f) => f.kind.startsWith("block"))
    .map((f) => polygon(f.points, "#000", TONE.mono.block[f.tone]))
    .join("");
  // The stroke reaches half its width past the bottom vertex and the block has none, so
  // lifting the glyph by a quarter of it leaves the same margin above and below.
  const stroke = 3.2;
  return (
    `<g transform="translate(0 ${-stroke / 4})">` +
    `<path d="M${outline.map(pt).join(" L")} Z ${inner}" fill="none" stroke="#000" stroke-width="${stroke}" stroke-linejoin="round" stroke-linecap="round"/>` +
    `${block}</g>`
  );
}

/**
 * Needs you: the whole mark in orange; not a template, so it keeps its colour. The block
 * floats clear of the other cubes, so a white one would vanish on a light menu bar.
 */
function needsYou(g) {
  const fill = {
    top: COLOUR.needsYouTop,
    left: COLOUR.needsYouLeft,
    right: COLOUR.needsYouRight,
    blockTop: COLOUR.needsYouTop,
    blockLeft: COLOUR.needsYouLeft,
    blockRight: COLOUR.needsYouRight,
  };
  return paint(g, fill, () => 1);
}

const TILE_RADIUS = 22.5;

const defs =
  `<defs>` +
  `<linearGradient id="tile" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${COLOUR.tileTop}"/><stop offset="1" stop-color="${COLOUR.tileBottom}"/></linearGradient>` +
  `<linearGradient id="rim" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.16"/><stop offset="0.45" stop-color="#fff" stop-opacity="0"/></linearGradient>` +
  `</defs>`;

const tile =
  `<rect width="100" height="100" rx="${TILE_RADIUS}" fill="url(#tile)"/>` +
  `<rect x="0.5" y="0.5" width="99" height="99" rx="${TILE_RADIUS - 0.5}" fill="none" stroke="url(#rim)"/>`;

const svg = (attributes, body) =>
  `<svg xmlns="http://www.w3.org/2000/svg" ${attributes}>${body}</svg>\n`;

/** The tray canvas in tile units, centred on the mark. */
function trayViewBox() {
  const size = n((TRAY_PX * TRAY.edge) / TRAY_EDGE_PX);
  const origin = n(50 - size / 2);
  return `viewBox="${origin} ${origin} ${size} ${size}" width="${TRAY_PX}" height="${TRAY_PX}"`;
}

/** The one-colour glyph's canvas: the mark's bounds and one unit round them. */
function glyphViewBox() {
  const { x0, x1, y0, y1 } = bounds(centred(shapes(MASTER)));
  return `viewBox="${n(x0 - 1)} ${n(y0 - 1)} ${n(x1 - x0 + 2)} ${n(y1 - y0 + 2)}"`;
}

const files = {
  "docs/brand/logo.svg": svg(
    `viewBox="0 0 100 100" width="200" height="200" role="img" aria-label="git-reap"`,
    `${defs}${tile}${colourMark()}`,
  ),
  "docs/brand/glyph-mono.svg": svg(
    `${glyphViewBox()} role="img" aria-label="git-reap"`,
    `<style>g{color:#000}@media (prefers-color-scheme: dark){g{color:#fff}}</style><g>${monoMark(MASTER, "currentColor")}</g>`,
  ),
  // macOS icon grid: the tile at 824 of 1024, centred.
  "app/icons/src/app-icon.svg": svg(
    `viewBox="0 0 1024 1024" width="1024" height="1024"`,
    `${defs}<g transform="translate(100 100) scale(8.24)">${tile}${colourMark()}</g>`,
  ),
  "app/icons/src/tray-tidy.svg": svg(trayViewBox(), monoMark(TRAY, "#000")),
  "app/icons/src/tray-pending.svg": svg(trayViewBox(), wireframe(TRAY)),
  "app/icons/src/tray-needs-you.svg": svg(trayViewBox(), needsYou(TRAY)),
};

for (const [file, contents] of Object.entries(files)) {
  writeFileSync(join(root, file), contents);
}
