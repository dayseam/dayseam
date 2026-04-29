// DAY-211. Pure geometry helper for the per-day donut chart that
// sits at the top of the streaming preview.
//
// Why a separate module:
//   - The maths is independent of React, the IPC types, and the
//     theme system, so a tight unit test can lock the rendered
//     `d` strings byte-for-byte without spinning up jsdom or
//     mocking `connectorAccent`. That matters because SVG arc
//     pathing is full of off-by-one traps (large-arc-flag,
//     sweep-flag, the full-circle degeneracy below) and we want
//     the test failures to point at the formula, not the chart.
//   - The marketing site at `dayseam/dayseam.github.io` already
//     ships a near-identical helper at `src/components/
//     ReportMock.tsx` line 194. The desktop and the site are
//     intentionally not coupled (the site is a static GitHub
//     Pages app; pulling in `@dayseam/ui` would force a published
//     package), so we accept a small duplication in exchange for
//     keeping each repo independently buildable. When this
//     formula changes, change both. The test in this file's
//     `__tests__` folder is the regression guard for the desktop
//     side.

/**
 * One slice descriptor passed in to {@link donutPaths}. Only the
 * relative `value` matters; absolute scale is collapsed by dividing
 * by the sum of all values, so the caller can pass raw counts (3, 7,
 * 1) or normalised percentages (30, 70, 10) interchangeably.
 */
export interface DonutSegment {
  readonly value: number;
}

/**
 * Convert a list of donut segments into SVG arc `d` strings for a
 * donut centred at `(cx, cy)` with outer radius `r` and inner radius
 * `ri`. Returns one path per segment, in input order, so the caller
 * can paint each with its own colour.
 *
 * Conventions
 *
 *   - First slice starts at the 12 o'clock position (`-π/2`) and
 *     each successive slice continues clockwise, so the chart
 *     reads the way users expect (top, then right, then bottom,
 *     then left).
 *   - `ri = 0` produces a pie chart; `ri > 0` produces a donut.
 *     The function never asserts `ri < r`, so a pathological caller
 *     with `ri >= r` will produce an inverted ring; we treat that
 *     as caller-side garbage-in / garbage-out rather than guarding
 *     it because the only call sites in the repo pass fixed
 *     constants and a runtime check would burn bytes for no win.
 *
 * Edge cases
 *
 *   - Empty `segments` → empty output. The chart's empty state is
 *     a presentation concern, not a geometry one.
 *   - Total `value` of 0 (every segment zero) → empty output, same
 *     reasoning. The caller is responsible for rendering an
 *     "no activity" affordance instead.
 *   - A single segment carrying ALL the value collapses the arc
 *     into a 2π sweep, which an SVG `A` command cannot represent
 *     (start and end points coincide, so the renderer has no way
 *     to choose a direction). We special-case this by emitting two
 *     semicircle arcs that together close the ring. Without this,
 *     Chromium and Safari render an empty path while Firefox draws
 *     a thin line — the kind of "donut disappeared on a single-
 *     source day" bug that survives multiple review passes.
 */
export function donutPaths(
  segments: readonly DonutSegment[],
  cx: number,
  cy: number,
  r: number,
  ri: number,
): string[] {
  if (segments.length === 0) return [];

  const total = segments.reduce((acc, s) => acc + s.value, 0);
  if (total <= 0) return [];

  const paths: string[] = [];
  let angle = -Math.PI / 2;

  for (const seg of segments) {
    if (seg.value <= 0) {
      // Skip zero-weight slices so a kind with no bullets doesn't
      // contribute a hairline at angle = previous-end. Preserves the
      // input-order → output-order contract because the caller can
      // still iterate the input and zip results when they re-add a
      // colour or label step.
      paths.push("");
      continue;
    }

    const sweep = (seg.value / total) * Math.PI * 2;
    const start = angle;
    const end = angle + sweep;
    angle = end;

    if (Math.abs(sweep - Math.PI * 2) < 1e-9) {
      paths.push(fullRingPath(cx, cy, r, ri));
      continue;
    }

    const outerStartX = cx + r * Math.cos(start);
    const outerStartY = cy + r * Math.sin(start);
    const outerEndX = cx + r * Math.cos(end);
    const outerEndY = cy + r * Math.sin(end);
    const innerStartX = cx + ri * Math.cos(end);
    const innerStartY = cy + ri * Math.sin(end);
    const innerEndX = cx + ri * Math.cos(start);
    const innerEndY = cy + ri * Math.sin(start);
    const largeArc = sweep > Math.PI ? 1 : 0;

    paths.push(
      [
        `M ${outerStartX.toFixed(2)} ${outerStartY.toFixed(2)}`,
        `A ${r} ${r} 0 ${largeArc} 1 ${outerEndX.toFixed(2)} ${outerEndY.toFixed(2)}`,
        `L ${innerStartX.toFixed(2)} ${innerStartY.toFixed(2)}`,
        `A ${ri} ${ri} 0 ${largeArc} 0 ${innerEndX.toFixed(2)} ${innerEndY.toFixed(2)}`,
        "Z",
      ].join(" "),
    );
  }

  return paths;
}

/**
 * Build a closed donut ring as two outer semicircles joined to two
 * inner semicircles, walking outer-clockwise then inner-counter-
 * clockwise. The fill rule is `nonzero` by default in SVG, so the
 * resulting path renders as a filled ring rather than a filled disk.
 *
 * Used only for the full-circle (single non-zero segment) case in
 * {@link donutPaths}; factored out so the path-string format stays
 * close to the multi-segment branch and so the unit test can
 * snapshot it without re-deriving the geometry.
 */
function fullRingPath(cx: number, cy: number, r: number, ri: number): string {
  const top = { x: cx, y: cy - r };
  const bottom = { x: cx, y: cy + r };
  const innerTop = { x: cx, y: cy - ri };
  const innerBottom = { x: cx, y: cy + ri };
  return [
    `M ${top.x.toFixed(2)} ${top.y.toFixed(2)}`,
    `A ${r} ${r} 0 1 1 ${bottom.x.toFixed(2)} ${bottom.y.toFixed(2)}`,
    `A ${r} ${r} 0 1 1 ${top.x.toFixed(2)} ${top.y.toFixed(2)}`,
    `M ${innerTop.x.toFixed(2)} ${innerTop.y.toFixed(2)}`,
    `A ${ri} ${ri} 0 1 0 ${innerBottom.x.toFixed(2)} ${innerBottom.y.toFixed(2)}`,
    `A ${ri} ${ri} 0 1 0 ${innerTop.x.toFixed(2)} ${innerTop.y.toFixed(2)}`,
    "Z",
  ].join(" ");
}
