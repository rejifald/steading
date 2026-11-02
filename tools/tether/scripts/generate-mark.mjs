// Writes every git-tether mark from one geometry: the brand SVGs in docs/brand/ and the
// icon sources in app/icons/src/. scripts/render-icons.sh runs it before rendering.
//
//   node scripts/generate-mark.mjs
//
// The mark is a cube in 2:1 isometric on a 100-unit tile, centred on (50,50). Its top face
// carries the tether: a ring on two cords, cut through, holding a raised blue core. Its
// sides are cut into layers that darken downwards. Every cut is drawn as a gap between
// shapes, never a mask, because AppKit rasterises masks coarsely.
import { writeFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "..");

/** The app icon and brand masters. Lengths on the top face are fractions of the edge. */
const MASTER = {
  edge: 29,
  centre: 0.56,
  ring: 0.26,
  core: 0.155,
  cord: 0.05,
  rise: 3,
  layers: [
    [0, 0.431],
    [0.517, 0.707],
    [0.793, 1],
  ],
};

/** The 18pt tray glyph: same cube, with every cut opened to about 2px at 36px. */
const TRAY = {
  edge: 29,
  centre: 0.52,
  ring: 0.36,
  core: 0.17,
  cord: 0.08,
  rise: 3.5,
  layers: [
    [0, 0.55],
    [0.68, 1],
  ],
};

/** The tray canvas: 36px (18pt at 2x), scaled so the cube's edge is exactly 16px. */
const TRAY_PX = 36;
const TRAY_EDGE_PX = 16;

const COLOUR = {
  shell: "#EAF0F8",
  coreTop: "#5B9DF5",
  coreSideLight: "#2A74E0",
  coreSideDark: "#16479A",
  tileTop: "#1C2636",
  tileBottom: "#0B0F17",
  needsYouTop: "#F5894A",
  needsYouLeft: "#E8590C",
  needsYouRight: "#B3440A",
};

/** Opacities of the top face and of each side layer, top layer first. */
const TONE = {
  colour: { top: 0.95, left: [0.72, 0.6, 0.48], right: [0.46, 0.37, 0.28] },
  // One colour is lit from the side, the top on the middle step: a template image is dark
  // on a light menu bar and bright on a dark one, so a top-lit cube looks lit from below.
  mono: { top: 0.38, left: [0.55, 0.47, 0.39], right: [0.24, 0.2, 0.16], coreTop: 1, coreSide: 0.78 },
};

const n = (value) => Number(value.toFixed(3));
const pt = ([x, y]) => `${n(x)},${n(y)}`;

/** Maps cube-local coordinates to the tile, for a cube of the given edge. */
function project(edge) {
  const back = 50 - edge;
  return {
    // (s, t) on the top face: s runs from the back vertex to the right, t to the left.
    top: (s, t) => [50 + edge * (s - t), back + (edge / 2) * (s + t)],
    // (u, v) on a side face: u runs from the outer edge (left) or front edge (right), v down.
    left: (u, v) => [50 - edge + edge * u, 50 - edge / 2 + (edge / 2) * u + edge * v],
    right: (u, v) => [50 + edge * u, 50 - (edge / 2) * u + edge * v],
  };
}

/** The top face as two paths, back and front of the ring and cords, with the cuts between. */
function topFace(g) {
  const { top } = project(g.edge);
  const { centre: c, ring: r, cord: w } = g;
  const d = Math.sqrt(r * r - w * w);
  const rx = n(g.edge * Math.SQRT2 * r);
  const ry = n(rx / 2);
  const back =
    `M${pt(top(0, c - w))} L${pt(top(0, 0))} L${pt(top(c - w, 0))} L${pt(top(c - w, c - d))} ` +
    `A${rx} ${ry} 0 0 0 ${pt(top(c - d, c - w))} Z`;
  const front =
    `M${pt(top(0, c + w))} L${pt(top(0, 1))} L${pt(top(1, 1))} L${pt(top(1, 0))} ` +
    `L${pt(top(c + w, 0))} L${pt(top(c + w, c - d))} A${rx} ${ry} 0 1 1 ${pt(top(c - d, c + w))} Z`;
  return `${back} ${front}`;
}

/** One polygon per layer of a side face. */
function sideLayers(g, side) {
  const face = project(g.edge)[side];
  return g.layers.map(([v0, v1]) =>
    [face(0, v0), face(1, v0), face(1, v1), face(0, v1)].map(pt).join(" "),
  );
}

/** The raised core: its side (a band under the top ellipse) and its top. */
function core(g) {
  const [x, y] = project(g.edge).top(g.centre, g.centre);
  const rx = n(g.edge * Math.SQRT2 * g.core);
  const ry = n(rx / 2);
  const lift = n(y - g.rise);
  return {
    side: `M${n(x - rx)},${lift} L${n(x - rx)},${n(y)} A${rx} ${ry} 0 0 0 ${n(x + rx)},${n(y)} L${n(x + rx)},${lift} Z`,
    top: `<ellipse cx="${n(x)}" cy="${lift}" rx="${rx}" ry="${ry}"`,
  };
}

/** The cube's shapes, filled per face; `fill` and `opacity` pick the colour and tone. */
function cube(g, { fill, top, left, right, coreTop, coreSide }) {
  const faces = [
    `<path d="${topFace(g)}" fill="${fill.top}" fill-opacity="${top}"/>`,
    ...sideLayers(g, "left").map(
      (points, i) => `<polygon points="${points}" fill="${fill.left}" fill-opacity="${left[i]}"/>`,
    ),
    ...sideLayers(g, "right").map(
      (points, i) => `<polygon points="${points}" fill="${fill.right}" fill-opacity="${right[i]}"/>`,
    ),
  ];
  const { side, top: coreTopEllipse } = core(g);
  faces.push(`<path d="${side}" fill="${coreSide.fill}" fill-opacity="${coreSide.opacity}"/>`);
  faces.push(`${coreTopEllipse} fill="${coreTop.fill}" fill-opacity="${coreTop.opacity}"/>`);
  return faces.join("");
}

function colourCube() {
  const shell = { top: COLOUR.shell, left: COLOUR.shell, right: COLOUR.shell };
  return cube(MASTER, {
    fill: shell,
    ...TONE.colour,
    coreTop: { fill: COLOUR.coreTop, opacity: 1 },
    coreSide: { fill: "url(#core-side)", opacity: 1 },
  });
}

function monoCube(g, colour) {
  const shell = { top: colour, left: colour, right: colour };
  return cube(g, {
    fill: shell,
    ...TONE.mono,
    left: g.layers.map((_, i) => TONE.mono.left[i]),
    right: g.layers.map((_, i) => TONE.mono.right[i]),
    coreTop: { fill: colour, opacity: TONE.mono.coreTop },
    coreSide: { fill: colour, opacity: TONE.mono.coreSide },
  });
}

const TILE_RADIUS = 22.5;

const defs =
  `<defs>` +
  `<linearGradient id="tile" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${COLOUR.tileTop}"/><stop offset="1" stop-color="${COLOUR.tileBottom}"/></linearGradient>` +
  `<linearGradient id="rim" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity="0.16"/><stop offset="0.45" stop-color="#fff" stop-opacity="0"/></linearGradient>` +
  `<linearGradient id="core-side" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="${COLOUR.coreSideLight}"/><stop offset="1" stop-color="${COLOUR.coreSideDark}"/></linearGradient>` +
  `</defs>`;

const tile =
  `<rect width="100" height="100" rx="${TILE_RADIUS}" fill="url(#tile)"/>` +
  `<rect x="0.5" y="0.5" width="99" height="99" rx="${TILE_RADIUS - 0.5}" fill="none" stroke="url(#rim)"/>`;

const svg = (attributes, body) =>
  `<svg xmlns="http://www.w3.org/2000/svg" ${attributes}>${body}</svg>\n`;

/** The tray canvas in tile units, centred on the cube. */
function trayViewBox() {
  const size = n((TRAY_PX * TRAY.edge) / TRAY_EDGE_PX);
  const origin = n(50 - size / 2);
  return `viewBox="${origin} ${origin} ${size} ${size}" width="${TRAY_PX}" height="${TRAY_PX}"`;
}

/** Syncing: the cube as a wireframe, with its core still in place. */
function wireframe(g) {
  const { top, left, right } = project(g.edge);
  const outline = [top(0, 0), top(1, 0), right(1, 1), left(1, 1), left(0, 1), top(0, 1)];
  const outlinePath = `M${outline.map(pt).join(" L")} Z`;
  const inner = `M${pt(top(1, 0))} L${pt(top(1, 1))} L${pt(top(0, 1))} M${pt(top(1, 1))} L${pt(left(1, 1))}`;
  const { side, top: coreTopEllipse } = core(g);
  return (
    `<path d="${outlinePath} ${inner}" fill="none" stroke="#000" stroke-width="4" stroke-linejoin="round" stroke-linecap="round"/>` +
    `<path d="${side}" fill="#000" fill-opacity="${TONE.mono.coreSide}"/>${coreTopEllipse} fill="#000"/>`
  );
}

/** Needs you: the cube in orange with a white core; not a template, so it keeps its colour. */
function needsYou(g) {
  return cube(g, {
    fill: { top: COLOUR.needsYouTop, left: COLOUR.needsYouLeft, right: COLOUR.needsYouRight },
    top: 1,
    left: g.layers.map(() => 1),
    right: g.layers.map(() => 1),
    coreTop: { fill: "#fff", opacity: 1 },
    coreSide: { fill: "#fff", opacity: 0.75 },
  });
}

const files = {
  "docs/brand/logo.svg": svg(
    `viewBox="0 0 100 100" width="200" height="200" role="img" aria-label="git-tether"`,
    `${defs}${tile}${colourCube()}`,
  ),
  "docs/brand/glyph-mono.svg": svg(
    `viewBox="${50 - MASTER.edge - 1} ${50 - MASTER.edge - 1} ${2 * MASTER.edge + 2} ${2 * MASTER.edge + 2}" role="img" aria-label="git-tether"`,
    `<style>g{color:#000}@media (prefers-color-scheme: dark){g{color:#fff}}</style><g>${monoCube(MASTER, "currentColor")}</g>`,
  ),
  // macOS icon grid: the tile at 824 of 1024, centred.
  "app/icons/src/app-icon.svg": svg(
    `viewBox="0 0 1024 1024" width="1024" height="1024"`,
    `${defs}<g transform="translate(100 100) scale(8.24)">${tile}${colourCube()}</g>`,
  ),
  "app/icons/src/tray-in-sync.svg": svg(trayViewBox(), monoCube(TRAY, "#000")),
  "app/icons/src/tray-syncing.svg": svg(trayViewBox(), wireframe(TRAY)),
  "app/icons/src/tray-needs-you.svg": svg(trayViewBox(), needsYou(TRAY)),
};

for (const [file, contents] of Object.entries(files)) {
  writeFileSync(join(root, file), contents);
}
