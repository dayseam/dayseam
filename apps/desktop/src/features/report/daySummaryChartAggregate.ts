import type { RenderedSection, SourceKind } from "@dayseam/ipc-types";

// Keep this ordering in lockstep with `SOURCE_KIND_ORDER` in
// `StreamingPreview.tsx` (which is itself in lockstep with
// `dayseam_core::SourceKind::render_order` on the Rust side). The
// chart uses this list both as the canonical "kind we know how to
// render" allow-list (any bullet whose `source_kind` is missing
// from this set is dropped from the chart) AND as the tie-breaker
// when two kinds carry the same bullet count, so the slice order
// stays stable across re-renders rather than depending on
// `Array.prototype.sort` engine implementation details.
//
// The allow-list role is load-bearing: the e2e mocks (and any
// pre-DAY-104 persisted draft) can carry bullets whose
// `source_kind` is `null`, `undefined`, or a string not in
// `SLICE_ORDER`. Treating those as plot points would let unknown
// kinds into the slice pipeline (where every plotted kind must
// resolve accents safely), which would unmount `StreamingPreview`
// entirely instead of just hiding the chart — the regression
// DAY-211's first CI run caught.
const SLICE_ORDER: readonly SourceKind[] = [
  "LocalGit",
  "GitHub",
  "GitLab",
  "Jira",
  "Confluence",
  "Outlook",
];

const SLICE_ORDER_INDEX = new Map<SourceKind, number>(
  SLICE_ORDER.map((kind, index) => [kind, index]),
);

/** Type guard: is this value a `SourceKind` the chart knows how
 *  to render? Both stronger and looser than the TypeScript type:
 *  stronger because it actively excludes the legacy `null` /
 *  `undefined` shapes the runtime can carry, and looser because
 *  it treats any unknown string as "drop, don't crash" rather
 *  than relying on the static type alone. */
function isKnownKind(value: unknown): value is SourceKind {
  return typeof value === "string" && SLICE_ORDER_INDEX.has(value as SourceKind);
}

export interface KindCount {
  readonly kind: SourceKind;
  readonly count: number;
}

/**
 * Aggregate bullets across every section of a draft into one
 * count per `SourceKind`, dropping kinds with zero bullets.
 *
 * Several bullet shapes never contribute to slice counts:
 *
 *   1. `source_kind === null` (legacy pre-DAY-104 drafts, plus DAY-201
 *      `sync_issues` diagnostics — prose may bold “GitLab” etc., but the
 *      typed field stays null so connector failures never read as “work”).
 *   2. `source_kind === undefined` (the e2e mock omits the field
 *      from its baseline LocalGit bullets; runtime data does
 *      not always honour the ts-rs-generated `SourceKind | null`
 *      type, and a `SourceKind | null | undefined` widening on
 *      the type alias would force every other consumer to
 *      narrow it again).
 *   3. Any string not in this module's `SLICE_ORDER` allow-list
 *      (unknown labels or a new Rust-side enum value before the
 *      list grows). `isKnownKind` consults `SLICE_ORDER_INDEX` —
 *      not `connectorAccent` / `MARKS`, which only matter once a
 *      kind is already admitted for plotting.
 *
 * Bullets in those shapes still render in the report itself
 * (they go through `groupBulletsByKind` in `StreamingPreview`,
 * which tolerates `null` directly); the chart just doesn't
 * claim to count them.
 *
 * Result is sorted by count descending, ties broken by the same
 * canonical kind order as chart slices, so re-renders are stable.
 *
 * Exported so the unit test can pin the aggregation rules
 * independently of the SVG output.
 */
export function aggregateByKind(
  sections: readonly RenderedSection[],
): KindCount[] {
  const counts = new Map<SourceKind, number>();
  for (const section of sections) {
    for (const bullet of section.bullets) {
      const kind: unknown = bullet.source_kind;
      if (!isKnownKind(kind)) continue;
      counts.set(kind, (counts.get(kind) ?? 0) + 1);
    }
  }
  const entries: KindCount[] = [];
  for (const [kind, count] of counts.entries()) {
    if (count > 0) entries.push({ kind, count });
  }
  entries.sort((a, b) => {
    if (b.count !== a.count) return b.count - a.count;
    const aIdx = SLICE_ORDER_INDEX.get(a.kind) ?? Number.POSITIVE_INFINITY;
    const bIdx = SLICE_ORDER_INDEX.get(b.kind) ?? Number.POSITIVE_INFINITY;
    return aIdx - bIdx;
  });
  return entries;
}
