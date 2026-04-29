// DAY-211. Lock the SVG `d` strings emitted by `donutPaths` so a
// regression in the arc-flag logic shows up here, not as a chart
// that is mysteriously a few pixels off in the streaming preview.
//
// The test cases cover the four shapes the chart can be in:
//   1. Empty input (zero segments).
//   2. All-zero values (segments exist but everything is 0).
//   3. A single non-zero segment — the full-circle / two-arc
//      degeneracy that needs `fullRingPath`.
//   4. The realistic multi-segment case used by the chart.
//
// Path strings are compared verbatim. A formula change that moves a
// vertex even a hundredth of a pixel will fail the comparison, which
// is the regression bar we want — the chart is small (100x100
// viewBox) so any visible drift is geometrically large at that scale.

import { describe, expect, it } from "vitest";
import { donutPaths } from "../donutGeometry";

describe("donutPaths", () => {
  it("returns no paths for an empty segment list", () => {
    expect(donutPaths([], 50, 50, 48, 28)).toEqual([]);
  });

  it("returns no paths when the total weight is zero", () => {
    expect(
      donutPaths([{ value: 0 }, { value: 0 }, { value: 0 }], 50, 50, 48, 28),
    ).toEqual([]);
  });

  it("renders a single non-zero segment as a closed ring (full-circle case)", () => {
    const paths = donutPaths([{ value: 7 }], 50, 50, 48, 28);
    expect(paths).toHaveLength(1);
    // The full ring is two outer semicircles (clockwise) joined to
    // two inner semicircles (counter-clockwise) so the SVG nonzero
    // fill rule paints the ring, not the disk.
    expect(paths[0]).toBe(
      "M 50.00 2.00 A 48 48 0 1 1 50.00 98.00 A 48 48 0 1 1 50.00 2.00 " +
        "M 50.00 22.00 A 28 28 0 1 0 50.00 78.00 A 28 28 0 1 0 50.00 22.00 Z",
    );
  });

  it("emits empty strings for zero-weight slices in a mixed input", () => {
    // A zero-weight slice should not contribute geometry but it MUST
    // keep the result-array length equal to the input-array length,
    // so a caller zipping `paths` with `segments` to pick a colour
    // still lines up.
    const paths = donutPaths(
      [{ value: 5 }, { value: 0 }, { value: 5 }],
      50,
      50,
      48,
      28,
    );
    expect(paths).toHaveLength(3);
    expect(paths[1]).toBe("");
    expect(paths[0]).not.toBe("");
    expect(paths[2]).not.toBe("");
  });

  it("renders the realistic four-segment day correctly", () => {
    // Counts modelled on a midweek day: 7 GitHub commits, 3 Jira
    // tickets, 2 Confluence edits, 1 Outlook meeting. Total = 13.
    const paths = donutPaths(
      [{ value: 7 }, { value: 3 }, { value: 2 }, { value: 1 }],
      50,
      50,
      48,
      28,
    );
    expect(paths).toEqual([
      "M 50.00 2.00 " +
        "A 48 48 0 1 1 38.51 96.61 " +
        "L 43.30 77.19 " +
        "A 28 28 0 1 0 50.00 22.00 Z",
      "M 38.51 96.61 " +
        "A 48 48 0 0 1 2.35 44.21 " +
        "L 22.20 46.62 " +
        "A 28 28 0 0 0 43.30 77.19 Z",
      "M 2.35 44.21 " +
        "A 48 48 0 0 1 27.69 7.50 " +
        "L 36.99 25.21 " +
        "A 28 28 0 0 0 22.20 46.62 Z",
      "M 27.69 7.50 " +
        "A 48 48 0 0 1 50.00 2.00 " +
        "L 50.00 22.00 " +
        "A 28 28 0 0 0 36.99 25.21 Z",
    ]);
  });
});
