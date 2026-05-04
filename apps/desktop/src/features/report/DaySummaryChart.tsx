// DAY-211. Per-day at-a-glance donut chart for the streaming
// preview. Sits at the top of a generated report and visualises the
// breakdown of bullets by `SourceKind` for that day, using each
// kind's brand accent so the chart's colours rhyme with the
// connector logos in the sources sidebar.
//
// Why this exists
//
//   For most of the app's life the report has been a vertical wall
//   of text — sections, kind sub-groups, bullets. That's exactly
//   right for a user reading their day, but it makes the very first
//   instant of opening the report ("did anything happen?") harder
//   than it needs to be: you have to scan to find out. The chart
//   answers that question at a glance, mirrors the same kind
//   palette a Dayseam user has already learned, and stays useful
//   the moment your eye moves on to the bullets.
//
//   The marketing site at `dayseam/dayseam.github.io` has been
//   showing a donut in its mock report since DAY-166. Until DAY-211
//   that mock was a "deliberate near-future product hint" — the
//   real app rendered no chart. Building it for real closes the
//   credibility gap between hero and download.
//
// Design rules (locked in the DAY-211 plan, kept here so the next
// change-set has the rationale on hand):
//
//   - Donut, not pie. Inner radius 28 of 50 (matches the marketing
//     mock at `src/components/ReportMock.tsx` line 231 in the site
//     repo), so the hollow centre carries an at-a-glance count
//     without crowding the slices.
//   - Slices ordered by count descending, ties broken by the
//     canonical kind enum order from `StreamingPreview` so a chart
//     re-renders identically across re-stream events even when the
//     final-vs-mid-stream counts produce identical totals.
//   - Each slice is `<button>`-clickable: clicking jumps the
//     viewport to the matching `[data-kind="<kind>"]` group below.
//     Falls back to a static slice if `scrollIntoView` is missing
//     (older Tauri WebViews shouldn't be, but the fallback keeps
//     the chart safe to ship).
//   - `<title>` per slice holds the native browser tooltip — compact
//     `Kind count · pct%` (same string as the visible legend line).
//     No tooltip library, no portal, no state.
//   - Empty state: zero bullets across the whole day → render a
//     muted neutral ring with "No activity recorded today" beside
//     it, so a quiet Saturday doesn't read as a broken chart.
//   - Theme: each slice's fill comes from `connectorAccent(kind)`
//     wrapped in a CSS `light-dark()` so the chart picks up the
//     active theme without a React subscription, the same trick
//     `ConnectorLogo` uses.

import { useMemo } from "react";
import type { JSX } from "react";
import type { RenderedSection, SourceKind } from "@dayseam/ipc-types";
import { connectorAccent } from "../../components/connectorAccent";
import { aggregateByKind, type KindCount } from "./daySummaryChartAggregate";
import { donutPaths } from "./donutGeometry";

const KIND_LABEL: Record<SourceKind, string> = {
  LocalGit: "Local git",
  GitHub: "GitHub",
  GitLab: "GitLab",
  Jira: "Jira",
  Confluence: "Confluence",
  Outlook: "Outlook",
};

export interface DaySummaryChartProps {
  /** Sections from `ReportDraft`, used to compute the breakdown. */
  readonly sections: readonly RenderedSection[];
  /** Optional class hook for the outer wrapper. */
  readonly className?: string;
}

const VIEWBOX_SIZE = 100;
const CENTER = VIEWBOX_SIZE / 2;
const OUTER_RADIUS = 48;
const INNER_RADIUS = 28;

/** Format a slice's percentage for the legend line and slice tooltip.
 *  Whole percent from 10% upward; one decimal below 10% so a 3% slice
 *  does not read as "0%" from rounding. */
function formatPercent(count: number, total: number): string {
  if (total === 0) return "0%";
  const pct = (count / total) * 100;
  if (pct >= 10) return `${Math.round(pct)}%`;
  return `${pct.toFixed(1)}%`;
}

/** Build the screen-reader summary, e.g. "GitHub 7, Jira 3,
 *  Outlook 1: 11 items today." Empty days get a different phrase
 *  so screen readers don't announce "0 items". */
function buildAriaLabel(entries: readonly KindCount[]): string {
  if (entries.length === 0) return "Day summary: no activity recorded today.";
  const total = entries.reduce((acc, e) => acc + e.count, 0);
  const breakdown = entries
    .map((e) => `${KIND_LABEL[e.kind]} ${e.count}`)
    .join(", ");
  return `Day summary: ${breakdown}: ${total} item${total === 1 ? "" : "s"} today.`;
}

/**
 * Try to scroll the matching kind-group into view in the report
 * below the chart. The selector matches `data-kind` set by
 * `StreamingPreview`'s `SectionView` per kind-group. If multiple
 * sections (e.g. COMMITS and PULL REQUESTS) both contain the kind,
 * we hop to the first match — the user is asking "where do my
 * GitHub items live?", and the first one is the most useful answer.
 *
 * Defensive in two places:
 *   - `document` may not exist (SSR-style code paths in tests);
 *     guard so the click is a no-op rather than throwing.
 *   - `scrollIntoView` exists everywhere we ship (Tauri 2's WebView
 *     is recent), but the optional-chained call keeps the chart
 *     usable in oldened-out test environments.
 */
function scrollToKind(kind: SourceKind): void {
  if (typeof document === "undefined") return;
  const target = document.querySelector(`[data-kind="${kind}"]`);
  target?.scrollIntoView?.({ behavior: "smooth", block: "start" });
}

export function DaySummaryChart({
  sections,
  className,
}: DaySummaryChartProps): JSX.Element {
  const entries = useMemo(() => aggregateByKind(sections), [sections]);
  const total = useMemo(
    () => entries.reduce((acc, e) => acc + e.count, 0),
    [entries],
  );
  const paths = useMemo(
    () =>
      donutPaths(
        entries.map((e) => ({ value: e.count })),
        CENTER,
        CENTER,
        OUTER_RADIUS,
        INNER_RADIUS,
      ),
    [entries],
  );

  const ariaLabel = buildAriaLabel(entries);
  const isEmpty = entries.length === 0;

  return (
    <div
      className={`flex items-center gap-4 rounded-md border border-neutral-200 bg-neutral-50 px-4 py-3 dark:border-neutral-800 dark:bg-neutral-900 ${className ?? ""}`}
      data-testid="day-summary-chart"
    >
      <svg
        viewBox={`0 0 ${VIEWBOX_SIZE} ${VIEWBOX_SIZE}`}
        width={72}
        height={72}
        role="img"
        aria-label={ariaLabel}
        className="shrink-0"
      >
        {isEmpty ? (
          // Empty-state ring. Two semicircle arcs joined into a
          // closed band, painted in a muted neutral so it reads as
          // "no data" without being mistaken for one of the brand
          // accents. Geometry is the same ring fullRingPath would
          // produce, inlined here so the empty branch doesn't need
          // a fake KindCount entry to satisfy donutPaths.
          <path
            d={[
              `M ${CENTER} ${CENTER - OUTER_RADIUS}`,
              `A ${OUTER_RADIUS} ${OUTER_RADIUS} 0 1 1 ${CENTER} ${CENTER + OUTER_RADIUS}`,
              `A ${OUTER_RADIUS} ${OUTER_RADIUS} 0 1 1 ${CENTER} ${CENTER - OUTER_RADIUS}`,
              `M ${CENTER} ${CENTER - INNER_RADIUS}`,
              `A ${INNER_RADIUS} ${INNER_RADIUS} 0 1 0 ${CENTER} ${CENTER + INNER_RADIUS}`,
              `A ${INNER_RADIUS} ${INNER_RADIUS} 0 1 0 ${CENTER} ${CENTER - INNER_RADIUS}`,
              "Z",
            ].join(" ")}
            className="fill-neutral-200 dark:fill-neutral-800"
            data-testid="day-summary-chart-empty-ring"
          />
        ) : (
          entries.map((entry, idx) => {
            const accent = connectorAccent(entry.kind);
            const d = paths[idx];
            if (!d) return null;
            const tooltip = `${KIND_LABEL[entry.kind]} ${entry.count} · ${formatPercent(entry.count, total)}`;
            return (
              <path
                key={entry.kind}
                d={d}
                onClick={() => scrollToKind(entry.kind)}
                className="cursor-pointer outline-hidden transition-opacity duration-100 hover:opacity-80 focus-visible:opacity-80"
                style={{
                  fill: `light-dark(${accent.light}, ${accent.dark})`,
                }}
                data-kind-slice={entry.kind}
                data-accent-light={accent.light}
                data-accent-dark={accent.dark}
                data-count={entry.count}
                tabIndex={0}
                role="button"
                aria-label={tooltip}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    scrollToKind(entry.kind);
                  }
                }}
              >
                <title>{tooltip}</title>
              </path>
            );
          })
        )}

        {/* Centre count. Kept outside the slice loop so it doesn't
            duplicate per-slice and so the empty-state branch can
            still render a "0" without entering the slice path. The
            text is decorative for screen readers (the aria-label on
            the parent SVG already communicates the breakdown), so
            we suppress assistive announcement to avoid duplication. */}
        <text
          x={CENTER}
          y={CENTER}
          textAnchor="middle"
          dominantBaseline="central"
          aria-hidden="true"
          className="fill-neutral-700 text-[20px] font-semibold dark:fill-neutral-200"
          data-testid="day-summary-chart-total"
        >
          {total}
        </text>
      </svg>

      {/* Legend. Plain text (kind, count, and share) so screen readers
          receive the same breakdown from the report DOM regardless of
          whether they interpret the SVG. Row order matches slice order
          (largest slice = topmost legend row). */}
      <ul
        className="flex min-w-0 flex-1 flex-col gap-1 text-sm"
        data-testid="day-summary-chart-legend"
      >
        {isEmpty ? (
          <li className="text-neutral-500 dark:text-neutral-400">
            No activity recorded today
          </li>
        ) : (
          entries.map((entry) => {
            const accent = connectorAccent(entry.kind);
            return (
              <li
                key={entry.kind}
                className="flex min-w-0 items-center gap-2 text-neutral-700 dark:text-neutral-200"
                data-kind-legend={entry.kind}
              >
                <span
                  aria-hidden="true"
                  className="inline-block h-2.5 w-2.5 shrink-0 rounded-sm"
                  style={{
                    backgroundColor: `light-dark(${accent.light}, ${accent.dark})`,
                  }}
                  data-accent-light={accent.light}
                  data-accent-dark={accent.dark}
                />
                <span className="min-w-0 truncate tabular-nums">
                  {KIND_LABEL[entry.kind]} {entry.count} ·{" "}
                  {formatPercent(entry.count, total)}
                </span>
              </li>
            );
          })
        )}
      </ul>
    </div>
  );
}
