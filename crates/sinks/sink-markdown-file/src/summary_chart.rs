//! Day-summary block (DAY-214) prepended to the saved markdown body.
//!
//! ## Why this lives in the sink, not in `dayseam-report`
//!
//! `dayseam-report` produces the structured [`ReportDraft`] —
//! sections, bullets, evidence — and the sink owns the dialect of
//! the rendered file. The donut chart is a markdown-only artefact
//! (it's an SVG that lives in the `.md` file), so the renderer for
//! it sits in the sink alongside [`crate::markdown`]. Other sinks
//! (a future Notion sink, a future Slack-message sink) get to
//! decide their own equivalent without inheriting an SVG payload
//! they can't use.
//!
//! ## Why HTML-comment markers, not a fenced code block
//!
//! The desktop app's live preview ships a real interactive donut
//! (DAY-211, `apps/desktop/src/features/report/DaySummaryChart.tsx`).
//! A user looking at their saved daily note expects the same
//! at-a-glance breakdown. Two shapes were considered:
//!
//!   1. Fenced ` ```html ... ``` ` — bounded and trivially deletable,
//!      but every standard markdown renderer treats it as
//!      syntax-highlighted text. The SVG would never render as a
//!      donut anywhere.
//!   2. Inline SVG between `<!-- dayseam:chart-begin -->` and
//!      `<!-- dayseam:chart-end -->` — keeps the SVG inline (so
//!      viewers that support inline HTML in markdown — notably
//!      GitHub, GitLab in most contexts, and Obsidian's Reading
//!      View — paint it as the donut) and the markers turn the
//!      block into a single deletable unit in plain editors with
//!      the "delete this block to drop the summary chart" hint
//!      baked in. Viewers that strip inline HTML for safety fall
//!      through to readable XML markup the comment markers still
//!      bracket cleanly.
//!
//! Shape (2) is what this module emits.
//!
//! ## Why the geometry mirrors `donutGeometry.ts` byte-for-byte
//!
//! The TypeScript helper that drives the live preview ships in a
//! frontend bundle that the Rust crate cannot import. Sharing
//! geometry across the boundary would mean either (a) a wasm
//! bridge for ~30 lines of trig, or (b) a published `@dayseam/ui`
//! npm package the desktop bundle could `import` — both
//! disproportionate to the surface. The Rust port instead emits
//! the **same `d` strings** the TS port emits for the same input,
//! and [`tests::donut_paths_match_typescript_reference_byte_for_byte`]
//! locks them against the four canonical strings the TS unit test
//! at `apps/desktop/src/features/report/__tests__/donutGeometry.test.ts`
//! pins. If a contributor changes the TS port, the Rust test
//! breaks; if a contributor changes the Rust port, the byte-lock
//! breaks. That's the cross-port tripwire the duplication is
//! supposed to provide; if it ever gets removed, the docstring
//! above the `donut_paths` function explicitly documents how to
//! re-derive the canonical strings.
//!
//! Output convention (lifted from the TS port unchanged for the
//! donut; legend is Rust-only for Obsidian / GitHub parity):
//!
//!   * Centre `(50, 50)`, outer radius `48`, inner radius `28` in a
//!     `100×100` donut box; a **legend** (swatch + label + count) per
//!     slice appears **below** the ring in the same SVG. `viewBox`
//!     height grows with row count; `width="72"` is fixed to match
//!     the in-app chart width, `height` scales to preserve aspect.
//!   * Radii formatted as integer literals (`A 48 48 …`); vertex
//!     coordinates formatted with two decimals (`50.00`, `38.51`,
//!     …) — matches JavaScript's `Number.toString()` for whole
//!     numbers and `Number.prototype.toFixed(2)` for vertices.
//!   * First slice starts at 12 o'clock (`-π/2`) and slices
//!     progress clockwise.
//!   * Single-segment days emit a four-arc full ring (outer
//!     clockwise, inner counter-clockwise) so the `nonzero` SVG
//!     fill rule paints a ring rather than a disk; without this
//!     Chromium and Safari render an empty path on a single-source
//!     day. See the `fullRingPath` rationale in `donutGeometry.ts`.
//!
//! ## Aggregation rules
//!
//! Identical to `aggregateByKind` in the live chart — drop bullets
//! whose `source_kind` is `None`, sort by count descending, tie-break
//! by [`SourceKind::render_order`]. The Rust enum's exhaustive
//! `match` makes the "unknown kind" case unrepresentable here, so
//! the chart only has to defend against `None`.
//!
//! ## Theming
//!
//! Inline `fill` attributes carry the LIGHT-mode brand accent — the
//! palette is hard-copied from `apps/desktop/src/components/ConnectorLogo.tsx`
//! at the hexes documented inside `connector_accent_light`. A small
//! inline `<style>` block layers on a `@media (prefers-color-scheme:
//! dark)` override for kinds whose brand sits poorly on a dark
//! background (notably GitHub's near-black mark). Viewers that
//! strip `<style>` for security fall back to the inline `fill` —
//! the chart is still visible, just not theme-adaptive.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use dayseam_core::{ReportDraft, SourceKind};

/// Centre of the SVG viewBox. Matches `CENTER` in the live preview.
const CX: f64 = 50.0;
/// Centre of the SVG viewBox. Matches `CENTER` in the live preview.
const CY: f64 = 50.0;
/// Outer ring radius. Matches `OUTER_RADIUS` in `DaySummaryChart.tsx`.
const OUTER_RADIUS: u32 = 48;
/// Inner-hole radius. Matches `INNER_RADIUS` in `DaySummaryChart.tsx`.
const INNER_RADIUS: u32 = 28;
/// Donut geometry lives in a `100×100` square (matches live `viewBox`).
const DONUT_VIEW: u32 = 100;
/// Horizontal `viewBox` width; legend text uses the full width under the donut.
const VIEWBOX_W: u32 = DONUT_VIEW;
/// First legend row text baseline (user units), below the donut.
const LEGEND_FIRST_LINE_Y: u32 = 109;
/// Vertical gap between legend rows (user units).
const LEGEND_LINE_SPACING: u32 = 11;
/// Display width in CSS pixels (matches the live card SVG `width={72}`).
const DISPLAY_W: u32 = 72;

/// Public entry point: render the entire summary block (headline
/// paragraph + delimited inline SVG) for `draft`. Returns an empty
/// string for a draft with no chart-eligible bullets, so the caller
/// can prepend it unconditionally without sprinkling a "do I have a
/// chart?" branch through the body assembly.
pub(crate) fn render_block(draft: &ReportDraft) -> String {
    let counts = aggregate_by_kind(draft);
    let total: u32 = counts.iter().map(|(_, c)| *c).sum();

    if total == 0 {
        // No chart-eligible bullets — emit nothing. A single-line
        // "Quiet day" headline was considered and rejected: the
        // saved file already shows empty section headings, so a
        // separate "nothing happened" sentence is redundant noise.
        return String::new();
    }

    let mut out = String::new();
    out.push_str(&render_headline(&counts, total));
    out.push_str("\n\n");
    out.push_str("<!-- dayseam:chart-begin (delete this block to drop the summary chart) -->\n");
    out.push_str(&render_svg(&counts, total));
    out.push_str("\n<!-- dayseam:chart-end -->\n\n");
    out
}

/// Aggregate every bullet across every section into one count per
/// [`SourceKind`]. Bullets with `source_kind == None` (legacy
/// pre-DAY-104 drafts) are excluded — they have no kind to paint
/// and including them as an "unknown" slice would teach the reader
/// a category that doesn't exist anywhere else in the saved file.
///
/// Sorted by count descending, ties broken by
/// [`SourceKind::render_order`] so re-renders are stable.
pub(crate) fn aggregate_by_kind(draft: &ReportDraft) -> Vec<(SourceKind, u32)> {
    let order = SourceKind::render_order();
    let mut counts: Vec<(SourceKind, u32)> = order.iter().map(|k| (*k, 0u32)).collect();

    for section in &draft.sections {
        for bullet in &section.bullets {
            if let Some(kind) = bullet.source_kind {
                if let Some(slot) = counts.iter_mut().find(|(k, _)| *k == kind) {
                    slot.1 = slot.1.saturating_add(1);
                }
            }
        }
    }

    counts.retain(|(_, n)| *n > 0);
    // Stable sort by count desc; tie-break preserves the original
    // insertion order, which is `render_order()`. `sort_by` is
    // stable in stdlib.
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    counts
}

/// Threshold above which the top kind earns the "Most of the day
/// was X work" framing. 40% of a day is roughly 2/5 of bullets,
/// which on a 10-bullet day is a 4-vs-2-and-2-and-2 spread —
/// dominant enough to claim leadership without overclaiming. The
/// 39%-feels-leader-y boundary case is handled by `whole_percent`'s
/// rounding: a true 39.5% slice rounds to 40 and triggers this
/// branch, while a true 38% slice falls through to the spread
/// framing where each kind is named with its absolute count
/// regardless. Tweak with care — every adjustment shifts the
/// emotional valence of every saved daily note that's near the
/// boundary.
const CLEAR_LEADER_PERCENT: u32 = 40;

/// Build a one-sentence TL;DR for the day. Five cases (in
/// branch-evaluation order so the comment matches the code below):
///
///   1. Empty — handled in [`render_block`] by returning the empty
///      string, never reaches this function.
///   2. Single kind (one slice ≥ 100% by definition) →
///      "Today was all **X** — N items."
///   3. Tied leaders **with non-trivial volume** (≥ 2 kinds share
///      the top count, and that count is ≥ 2) →
///      "**X** and **Y** led the day, N items each."
///      The `≥ 2` floor is load-bearing: a sparse day where every
///      kind has exactly one bullet is a thin day, not a tie of
///      leaders, and saying "GitHub, GitLab, Jira, and Confluence
///      led the day, 1 item each" reads as overclaim. Sparse-tie
///      days fall through to the spread branch, which names every
///      kind with its absolute count and reads as the honest
///      "scattered, low-volume" summary it is.
///   4. Clear leader (top kind's share ≥ [`CLEAR_LEADER_PERCENT`]) →
///      "Most of the day was **X** work — N items, P% of today."
///   5. Spread (no tie of size ≥ 2, no clear leader) →
///      "Spread across **X** (a), **Y** (b), and **Z** (c)."
///
/// Percentages use whole-number rounding so a 33.3% slice reads as
/// "33%" rather than "33.33%". The chart's tooltips do the same.
fn render_headline(counts: &[(SourceKind, u32)], total: u32) -> String {
    debug_assert!(
        !counts.is_empty(),
        "render_headline must not be called for empty days"
    );
    debug_assert!(total > 0);

    let top_count = counts[0].1;
    let leaders: Vec<SourceKind> = counts
        .iter()
        .take_while(|(_, c)| *c == top_count)
        .map(|(k, _)| *k)
        .collect();

    if counts.len() == 1 {
        let (kind, n) = counts[0];
        return format!(
            "Today was all **{}** — {} item{}.",
            kind.display_label(),
            n,
            plural_s(n)
        );
    }

    if leaders.len() >= 2 && top_count >= 2 {
        return format!(
            "{} led the day, {} item{} each.",
            join_bold_labels(&leaders),
            top_count,
            plural_s(top_count)
        );
    }

    let pct = whole_percent(top_count, total);
    if pct >= CLEAR_LEADER_PERCENT {
        let (kind, n) = counts[0];
        return format!(
            "Most of the day was **{}** work — {} item{}, {}% of today.",
            kind.display_label(),
            n,
            plural_s(n),
            pct
        );
    }

    let parts: Vec<String> = counts
        .iter()
        .map(|(k, n)| format!("**{}** ({})", k.display_label(), n))
        .collect();
    format!("Spread across {}.", oxford_join(&parts))
}

/// Render the inline SVG for the donut. Path geometry and palette
/// match the live preview; the **legend** (colour → source) is
/// included so Obsidian and other markdown readers show the same
/// mapping the desktop HTML legend provides.
///
/// Each slice is a single `<path>` with the slice's brand-accent
/// `fill` attribute and a `<title>` child carrying the tooltip
/// copy GitHub / Obsidian show on hover. A `<style>` block at the
/// top layers a `prefers-color-scheme: dark` override for kinds
/// whose default mark sits poorly on a dark canvas (notably the
/// GitHub black). Viewers that strip `<style>` (some sandboxed
/// pipelines do for safety) keep the inline fills and lose only
/// the dark-mode adaptation.
fn render_svg(counts: &[(SourceKind, u32)], total: u32) -> String {
    let vb_h = view_box_height_px(counts.len());
    let display_h = display_height_px(vb_h);

    let segments: Vec<u32> = counts.iter().map(|(_, n)| *n).collect();
    let paths = donut_paths(&segments, CX, CY, OUTER_RADIUS, INNER_RADIUS);
    let aria = build_aria_label(counts);

    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {vbw} {vbh}\" width=\"{dw}\" height=\"{dh}\" role=\"img\" aria-label=\"{a}\">\n",
        vbw = VIEWBOX_W,
        vbh = vb_h,
        dw = DISPLAY_W,
        dh = display_h,
        a = escape_xml_attr(&aria),
    ));
    out.push_str("  <style>\n");
    out.push_str("    @media (prefers-color-scheme: dark) {\n");
    for kind in counts.iter().map(|(k, _)| *k) {
        let dark = connector_accent_dark(kind);
        let light = connector_accent_light(kind);
        if dark != light {
            out.push_str(&format!(
                "      .ds-{} {{ fill: {}; }}\n",
                kind_css_class(kind),
                dark
            ));
        }
    }
    // Centre numeral and legend lines use the same dark-mode text
    // colour as the app shell.
    out.push_str("      .ds-centre-label,\n");
    out.push_str("      .ds-legend-text { fill: #E5E7EB; }\n");
    out.push_str("    }\n");
    out.push_str("  </style>\n");

    for ((kind, count), d) in counts.iter().zip(paths.iter()) {
        let pct = whole_percent(*count, total);
        let tooltip = format!(
            "{} — {} item{} ({}%)",
            kind.display_label(),
            count,
            plural_s(*count),
            pct
        );
        out.push_str(&format!(
            "  <path class=\"ds-{cls}\" d=\"{d}\" fill=\"{light}\"><title>{tt}</title></path>\n",
            cls = kind_css_class(*kind),
            d = d,
            light = connector_accent_light(*kind),
            tt = escape_xml_text(&tooltip)
        ));
    }

    // Centre label: the day's bullet total, kept in sync with the
    // live preview's centre numeral so the saved chart and the
    // app-side chart read identically at a glance. The font-size
    // and weight match the live preview's `text-[20px]
    // font-semibold` Tailwind classes; the system-font stack
    // matches the desktop app's default UI font so the rendered
    // markdown looks at home next to the prose around it.
    out.push_str(&format!(
        "  <text class=\"ds-centre-label\" x=\"{CX}\" y=\"{CY}\" text-anchor=\"middle\" dominant-baseline=\"central\" font-family=\"-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif\" font-size=\"20\" font-weight=\"600\" fill=\"#404040\">{total}</text>\n"
    ));

    out.push_str("  <g class=\"ds-legend\" aria-hidden=\"true\">\n");
    for (i, (kind, count)) in counts.iter().enumerate() {
        let line_y = LEGEND_FIRST_LINE_Y + i as u32 * LEGEND_LINE_SPACING;
        let rect_top = line_y.saturating_sub(3);
        let pct = whole_percent(*count, total);
        let caption = format!(
            "{} — {} item{} ({}%)",
            kind.display_label(),
            count,
            plural_s(*count),
            pct
        );
        out.push_str(&format!(
            "    <rect class=\"ds-{cls}\" x=\"4\" y=\"{rect_top}\" width=\"8\" height=\"8\" rx=\"2\" fill=\"{light}\"/>\n",
            cls = kind_css_class(*kind),
            rect_top = rect_top,
            light = connector_accent_light(*kind),
        ));
        out.push_str(&format!(
            "    <text class=\"ds-legend-text\" x=\"16\" y=\"{line_y}\" dominant-baseline=\"central\" font-family=\"-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif\" font-size=\"7\" fill=\"#404040\">{cap}</text>\n",
            line_y = line_y,
            cap = escape_xml_text(&caption),
        ));
    }
    out.push_str("  </g>\n");

    out.push_str("</svg>");
    out
}

/// `viewBox` height: donut uses the top `100` units; legend stacks below.
#[must_use]
fn view_box_height_px(slice_count: usize) -> u32 {
    if slice_count == 0 {
        return DONUT_VIEW;
    }
    let rows = slice_count as u32;
    let last_line_y =
        LEGEND_FIRST_LINE_Y + rows.saturating_sub(1).saturating_mul(LEGEND_LINE_SPACING);
    last_line_y + 8
}

#[must_use]
fn display_height_px(view_box_h: u32) -> u32 {
    ((DISPLAY_W as u64 * view_box_h as u64 + (VIEWBOX_W as u64 / 2)) / VIEWBOX_W as u64) as u32
}

/// Build one SVG `d` string per slice, in the same shape and byte
/// format the live-preview helper at
/// `apps/desktop/src/features/report/donutGeometry.ts` produces.
///
/// `segments` holds the raw count for each slice (the absolute scale
/// is collapsed by dividing by the sum, exactly like the TS port).
/// A zero-weight segment produces an empty string at its index so
/// the caller can `zip` the result with a parallel colour list and
/// keep alignment.
///
/// Three cases:
///
///   * No slices, or every slice zero → return an empty `Vec`
///     (NOT a vec of empty strings), matching the TS contract.
///   * Exactly one non-zero segment → emit the four-arc full-ring
///     d-string. SVG's `A` command can't express a 360° sweep
///     reliably (start==end is ambiguous to the renderer); the
///     ring is built as outer-clockwise then inner-counter-
///     clockwise so the `nonzero` fill rule paints a ring instead
///     of a disk.
///   * Mixed multi-segment → emit one donut wedge per non-zero
///     segment as `M outer-start A outer-arc L inner-start A
///     inner-arc Z`.
///
/// The `r` and `ri` arguments are kept as `u32` so the format
/// string interpolates them as bare integers (`A 48 48 …`),
/// matching the TS port's `${r}` template-literal output. Vertex
/// coordinates use [`fmt_vertex`] (two decimals) to match
/// `Number.prototype.toFixed(2)`.
pub(crate) fn donut_paths(segments: &[u32], cx: f64, cy: f64, r: u32, ri: u32) -> Vec<String> {
    if segments.is_empty() {
        return Vec::new();
    }
    let total: u32 = segments.iter().sum();
    if total == 0 {
        return Vec::new();
    }

    let r_f = f64::from(r);
    let ri_f = f64::from(ri);
    let mut paths: Vec<String> = Vec::with_capacity(segments.len());
    let mut angle = -FRAC_PI_2;

    for seg in segments {
        if *seg == 0 {
            paths.push(String::new());
            continue;
        }

        let sweep = (f64::from(*seg) / f64::from(total)) * TAU;
        let start = angle;
        let end = angle + sweep;
        angle = end;

        if (sweep - TAU).abs() < 1e-9 {
            paths.push(full_ring_path(cx, cy, r, ri));
            continue;
        }

        let outer_start_x = cx + r_f * start.cos();
        let outer_start_y = cy + r_f * start.sin();
        let outer_end_x = cx + r_f * end.cos();
        let outer_end_y = cy + r_f * end.sin();
        let inner_start_x = cx + ri_f * end.cos();
        let inner_start_y = cy + ri_f * end.sin();
        let inner_end_x = cx + ri_f * start.cos();
        let inner_end_y = cy + ri_f * start.sin();
        let large_arc = if sweep > PI { 1 } else { 0 };

        paths.push(format!(
            "M {osx} {osy} A {r} {r} 0 {la} 1 {oex} {oey} L {isx} {isy} A {ri} {ri} 0 {la} 0 {iex} {iey} Z",
            osx = fmt_vertex(outer_start_x),
            osy = fmt_vertex(outer_start_y),
            oex = fmt_vertex(outer_end_x),
            oey = fmt_vertex(outer_end_y),
            isx = fmt_vertex(inner_start_x),
            isy = fmt_vertex(inner_start_y),
            iex = fmt_vertex(inner_end_x),
            iey = fmt_vertex(inner_end_y),
            r = r,
            ri = ri,
            la = large_arc,
        ));
    }
    paths
}

/// Closed donut ring built as two outer semicircles joined to two
/// inner semicircles via a subpath transition (`M`). Mirrors
/// `fullRingPath` in `donutGeometry.ts`. The trailing `Z` closes
/// the inner subpath; the outer subpath stays open at its start
/// because the second outer arc returns the cursor exactly to
/// `(cx, cy - r)`, which `Z` would treat as a no-op anyway.
fn full_ring_path(cx: f64, cy: f64, r: u32, ri: u32) -> String {
    let r_f = f64::from(r);
    let ri_f = f64::from(ri);
    format!(
        "M {tx} {ty} A {r} {r} 0 1 1 {bx} {by} A {r} {r} 0 1 1 {tx} {ty} M {itx} {ity} A {ri} {ri} 0 1 0 {ibx} {iby} A {ri} {ri} 0 1 0 {itx} {ity} Z",
        tx = fmt_vertex(cx),
        ty = fmt_vertex(cy - r_f),
        bx = fmt_vertex(cx),
        by = fmt_vertex(cy + r_f),
        itx = fmt_vertex(cx),
        ity = fmt_vertex(cy - ri_f),
        ibx = fmt_vertex(cx),
        iby = fmt_vertex(cy + ri_f),
        r = r,
        ri = ri,
    )
}

/// Brand-accent hexes, light-mode default. Mirror of `MARKS[kind].accent.light`
/// in `apps/desktop/src/components/ConnectorLogo.tsx`. Documented
/// in DAY-170 (palette) and DAY-202 (Outlook addition); re-document
/// here so a future hex tweak in the frontend immediately surfaces
/// here under code review.
fn connector_accent_light(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::GitHub => "#24292F",
        SourceKind::GitLab => "#FC6D26",
        SourceKind::Jira => "#0052CC",
        SourceKind::Confluence => "#172B4D",
        SourceKind::LocalGit => "#F05032",
        SourceKind::Outlook => "#0078D4",
    }
}

/// Brand-accent hexes, dark-mode override. Mirror of
/// `MARKS[kind].accent.dark` in
/// `apps/desktop/src/components/ConnectorLogo.tsx`.
fn connector_accent_dark(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::GitHub => "#F0F6FC",
        SourceKind::GitLab => "#FC6D26",
        SourceKind::Jira => "#4C9AFF",
        SourceKind::Confluence => "#2684FF",
        SourceKind::LocalGit => "#F05032",
        SourceKind::Outlook => "#0078D4",
    }
}

/// Stable CSS class slug per kind. Lower-case so it interacts well
/// with the markdown viewers that lowercase tag names; ASCII-only
/// so it survives sandboxed `<style>`-stripping pipelines that
/// downcase or strip non-ASCII selectors.
fn kind_css_class(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::GitHub => "github",
        SourceKind::GitLab => "gitlab",
        SourceKind::Jira => "jira",
        SourceKind::Confluence => "confluence",
        SourceKind::LocalGit => "localgit",
        SourceKind::Outlook => "outlook",
    }
}

/// Format a float with exactly two decimals, matching JS's
/// `Number.prototype.toFixed(2)` so the Rust and TS donut paths are
/// byte-for-byte identical at the vertex level.
fn fmt_vertex(x: f64) -> String {
    format!("{x:.2}")
}

/// Round `numerator / total` to a whole percentage. Returns 0 if
/// `total` is 0, so a caller that forgot to short-circuit on the
/// empty-day case still gets a defensible number.
fn whole_percent(numerator: u32, total: u32) -> u32 {
    if total == 0 {
        return 0;
    }
    let pct = (f64::from(numerator) / f64::from(total)) * 100.0;
    pct.round() as u32
}

fn plural_s(n: u32) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Join a list of `SourceKind`s into a bolded English list:
///   `[GitHub]`            → `**GitHub**`
///   `[GitHub, Jira]`      → `**GitHub** and **Jira**`
///   `[GitHub, Jira, GitLab]` → `**GitHub**, **Jira**, and **GitLab**`
fn join_bold_labels(kinds: &[SourceKind]) -> String {
    let parts: Vec<String> = kinds
        .iter()
        .map(|k| format!("**{}**", k.display_label()))
        .collect();
    oxford_join(&parts)
}

/// Generic Oxford-comma join over already-formatted string parts.
fn oxford_join(parts: &[String]) -> String {
    match parts.len() {
        0 => String::new(),
        1 => parts[0].clone(),
        2 => format!("{} and {}", parts[0], parts[1]),
        _ => {
            let head = parts[..parts.len() - 1].join(", ");
            format!("{}, and {}", head, parts[parts.len() - 1])
        }
    }
}

/// Build the `aria-label` for the SVG. Drives screen readers in
/// Obsidian + GitHub's web reader, and gives the chart a non-visual
/// summary that survives any rendering downgrade. Format:
/// `Day summary: GitHub 4, GitLab 2, Jira 1.`
fn build_aria_label(counts: &[(SourceKind, u32)]) -> String {
    let parts: Vec<String> = counts
        .iter()
        .map(|(k, n)| format!("{} {}", k.display_label(), n))
        .collect();
    format!("Day summary: {}.", parts.join(", "))
}

/// Minimal XML-attribute escaping for the `aria-label` value. The
/// string can only legitimately contain characters from
/// `display_label()` plus digits and ASCII punctuation, so we only
/// need to handle the four characters that break XML attribute
/// parsing.
fn escape_xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Minimal XML-text escaping for the `<title>` body. Same character
/// set as `display_label()` — `&`, `<`, `>` need replacing.
fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};
    use dayseam_core::{RenderedBullet, RenderedSection, ReportDraft};
    use std::collections::HashMap;
    use uuid::Uuid;

    fn draft_with(sections: Vec<RenderedSection>) -> ReportDraft {
        ReportDraft {
            id: Uuid::nil(),
            date: NaiveDate::from_ymd_opt(2026, 4, 18).unwrap(),
            template_id: "dayseam.dev_eod".into(),
            template_version: "2026-04-18".into(),
            sections,
            evidence: Vec::new(),
            per_source_state: HashMap::new(),
            verbose_mode: false,
            generated_at: Utc::now(),
        }
    }

    fn bullet(id: &str, kind: Option<SourceKind>) -> RenderedBullet {
        RenderedBullet {
            id: id.to_string(),
            text: format!("bullet {id}"),
            source_kind: kind,
        }
    }

    fn section(id: &str, bullets: Vec<RenderedBullet>) -> RenderedSection {
        RenderedSection {
            id: id.to_string(),
            title: id.to_string(),
            bullets,
        }
    }

    // ---- aggregate_by_kind --------------------------------------

    #[test]
    fn aggregate_drops_legacy_none_kind_bullets() {
        let d = draft_with(vec![section(
            "s",
            vec![
                bullet("a", None),
                bullet("b", None),
                bullet("c", Some(SourceKind::GitHub)),
            ],
        )]);
        assert_eq!(aggregate_by_kind(&d), vec![(SourceKind::GitHub, 1)]);
    }

    #[test]
    fn aggregate_sorts_by_count_desc_with_render_order_tiebreak() {
        let d = draft_with(vec![section(
            "s",
            vec![
                // GitHub: 4
                bullet("a", Some(SourceKind::GitHub)),
                bullet("b", Some(SourceKind::GitHub)),
                bullet("c", Some(SourceKind::GitHub)),
                bullet("d", Some(SourceKind::GitHub)),
                // GitLab: 2
                bullet("e", Some(SourceKind::GitLab)),
                bullet("f", Some(SourceKind::GitLab)),
                // 1 each — render_order: LocalGit, Jira, Confluence, Outlook
                bullet("g", Some(SourceKind::Outlook)),
                bullet("h", Some(SourceKind::Confluence)),
                bullet("i", Some(SourceKind::Jira)),
                bullet("j", Some(SourceKind::LocalGit)),
            ],
        )]);
        assert_eq!(
            aggregate_by_kind(&d)
                .into_iter()
                .map(|(k, _)| k)
                .collect::<Vec<_>>(),
            vec![
                SourceKind::GitHub,
                SourceKind::GitLab,
                SourceKind::LocalGit,
                SourceKind::Jira,
                SourceKind::Confluence,
                SourceKind::Outlook,
            ]
        );
    }

    // ---- render_headline ----------------------------------------

    #[test]
    fn headline_single_kind_says_today_was_all() {
        let counts = vec![(SourceKind::GitHub, 5)];
        assert_eq!(
            render_headline(&counts, 5),
            "Today was all **GitHub** — 5 items."
        );
    }

    #[test]
    fn headline_single_kind_singular_when_count_is_one() {
        let counts = vec![(SourceKind::GitHub, 1)];
        assert_eq!(
            render_headline(&counts, 1),
            "Today was all **GitHub** — 1 item."
        );
    }

    #[test]
    fn headline_clear_leader_above_40_percent() {
        // 4 / 9 = 44%, top kind ≥ 40% threshold.
        let counts = vec![
            (SourceKind::GitHub, 4),
            (SourceKind::GitLab, 3),
            (SourceKind::Jira, 2),
        ];
        assert_eq!(
            render_headline(&counts, 9),
            "Most of the day was **GitHub** work — 4 items, 44% of today."
        );
    }

    #[test]
    fn headline_tied_leaders_lists_them_with_oxford_comma() {
        let counts = vec![
            (SourceKind::GitHub, 3),
            (SourceKind::Jira, 3),
            (SourceKind::GitLab, 1),
        ];
        // Two leaders → "X and Y led the day" (no Oxford comma needed).
        assert_eq!(
            render_headline(&counts, 7),
            "**GitHub** and **Jira** led the day, 3 items each.",
        );

        let three_way = vec![
            (SourceKind::GitHub, 2),
            (SourceKind::GitLab, 2),
            (SourceKind::Jira, 2),
        ];
        assert_eq!(
            render_headline(&three_way, 6),
            "**GitHub**, **GitLab**, and **Jira** led the day, 2 items each.",
        );
    }

    #[test]
    fn headline_falls_through_to_spread_when_every_kind_ties_at_one() {
        // Sparse day: 4 kinds each with exactly one bullet. The
        // tied-leaders branch would say "GitHub, GitLab, Jira, and
        // Confluence led the day, 1 item each." which reads as an
        // overclaim — nobody led, the day was just thin. The
        // top_count >= 2 floor sends this to the spread branch
        // where every kind is named with its absolute count.
        let counts = vec![
            (SourceKind::GitHub, 1),
            (SourceKind::GitLab, 1),
            (SourceKind::Jira, 1),
            (SourceKind::Confluence, 1),
        ];
        assert_eq!(
            render_headline(&counts, 4),
            "Spread across **GitHub** (1), **GitLab** (1), **Jira** (1), and **Confluence** (1).",
        );
    }

    #[test]
    fn headline_spread_when_no_leader_dominates() {
        // 3/10 = 30% — under 40% threshold, no tie, so falls through to spread.
        let counts = vec![
            (SourceKind::GitHub, 3),
            (SourceKind::GitLab, 2),
            (SourceKind::Jira, 2),
            (SourceKind::Confluence, 2),
            (SourceKind::Outlook, 1),
        ];
        assert_eq!(
            render_headline(&counts, 10),
            "Spread across **GitHub** (3), **GitLab** (2), **Jira** (2), **Confluence** (2), and **Outlook** (1).",
        );
    }

    // ---- donut_paths --------------------------------------------

    #[test]
    fn donut_paths_empty_for_empty_input() {
        let empty: Vec<String> = Vec::new();
        assert_eq!(donut_paths(&[], 50.0, 50.0, 48, 28), empty);
        assert_eq!(donut_paths(&[0, 0, 0], 50.0, 50.0, 48, 28), empty);
    }

    /// Lock the `d` strings byte-for-byte against the canonical
    /// values in `apps/desktop/src/features/report/__tests__/donutGeometry.test.ts`.
    /// If this test fails, either:
    ///   * the Rust port has drifted from the TS port — fix the
    ///     Rust geometry, or
    ///   * the TS port has changed and the saved markdown will
    ///     diverge from the live preview — update the TS test
    ///     first, then mirror the new canonical strings here.
    ///
    /// The strings below were copied verbatim from the TS test on
    /// 2026-04-29; they exercise empty input, all-zero input,
    /// single-segment full ring, zero-weight slice in mixed input,
    /// and the realistic four-segment day (7, 3, 2, 1).
    #[test]
    fn donut_paths_match_typescript_reference_byte_for_byte() {
        // Full-circle / single non-zero segment.
        assert_eq!(
            donut_paths(&[7], 50.0, 50.0, 48, 28),
            vec![
                "M 50.00 2.00 A 48 48 0 1 1 50.00 98.00 A 48 48 0 1 1 50.00 2.00 \
                 M 50.00 22.00 A 28 28 0 1 0 50.00 78.00 A 28 28 0 1 0 50.00 22.00 Z",
            ],
        );

        // Zero-weight slice in mixed input — preserves array length,
        // emits "" at the zero index.
        let mixed = donut_paths(&[5, 0, 5], 50.0, 50.0, 48, 28);
        assert_eq!(mixed.len(), 3);
        assert_eq!(mixed[1], "");
        assert!(!mixed[0].is_empty());
        assert!(!mixed[2].is_empty());

        // Realistic four-segment day: 7 GitHub, 3 Jira, 2 Confluence,
        // 1 Outlook (total 13). Strings copied verbatim from
        // donutGeometry.test.ts so a drift in either port fails here.
        assert_eq!(
            donut_paths(&[7, 3, 2, 1], 50.0, 50.0, 48, 28),
            vec![
                "M 50.00 2.00 A 48 48 0 1 1 38.51 96.61 \
                 L 43.30 77.19 A 28 28 0 1 0 50.00 22.00 Z",
                "M 38.51 96.61 A 48 48 0 0 1 2.35 44.21 \
                 L 22.20 46.62 A 28 28 0 0 0 43.30 77.19 Z",
                "M 2.35 44.21 A 48 48 0 0 1 27.69 7.50 \
                 L 36.99 25.21 A 28 28 0 0 0 22.20 46.62 Z",
                "M 27.69 7.50 A 48 48 0 0 1 50.00 2.00 \
                 L 50.00 22.00 A 28 28 0 0 0 36.99 25.21 Z",
            ],
        );
    }

    // ---- render_svg ---------------------------------------------

    #[test]
    fn svg_includes_one_path_per_slice_with_inline_fill_and_title() {
        let counts = vec![(SourceKind::GitHub, 4), (SourceKind::GitLab, 2)];
        let svg = render_svg(&counts, 6);

        // Static fills present.
        assert!(svg.contains("fill=\"#24292F\""), "{svg}");
        assert!(svg.contains("fill=\"#FC6D26\""), "{svg}");

        // <title> tooltip copy matches the live chart.
        assert!(
            svg.contains("<title>GitHub — 4 items (67%)</title>"),
            "{svg}"
        );
        assert!(
            svg.contains("<title>GitLab — 2 items (33%)</title>"),
            "{svg}"
        );

        // Centre label echoes the day's bullet total.
        assert!(svg.contains(">6</text>"), "centre numeral missing: {svg}");

        // Taller viewBox: legend under donut (2 slices → height 128, display height 92).
        assert!(svg.contains("viewBox=\"0 0 100 128\""), "{svg}");
        assert!(svg.contains("width=\"72\" height=\"92\""), "{svg}");

        assert!(svg.contains("class=\"ds-legend\""), "{svg}");
        assert!(svg.contains(">GitHub — 4 items (67%)</text>"), "{svg}");
        assert!(svg.contains(">GitLab — 2 items (33%)</text>"), "{svg}");

        // aria-label summarises the day.
        assert!(
            svg.contains("aria-label=\"Day summary: GitHub 4, GitLab 2.\""),
            "{svg}"
        );

        // Dark-mode override emitted only for kinds whose dark hex differs.
        // GitHub's light=#24292F vs dark=#F0F6FC → present.
        // GitLab's light=#FC6D26 vs dark=#FC6D26 → omitted.
        assert!(
            svg.contains(".ds-github { fill: #F0F6FC; }"),
            "GitHub dark override missing: {svg}"
        );
        assert!(
            !svg.contains(".ds-gitlab { fill: #FC6D26; }"),
            "GitLab override should be elided when light==dark: {svg}"
        );
        // Centre label gets a dark-mode override so it stays
        // legible against either viewer background.
        assert!(
            svg.contains(".ds-centre-label,\n      .ds-legend-text { fill: #E5E7EB; }"),
            "centre/legend dark override missing: {svg}"
        );
    }

    // ---- render_block (end-to-end) ------------------------------

    #[test]
    fn block_is_empty_when_no_chart_eligible_bullets() {
        let d = draft_with(vec![section("s", vec![bullet("a", None)])]);
        assert_eq!(render_block(&d), "");
    }

    #[test]
    fn block_prepends_headline_then_delimited_svg() {
        let d = draft_with(vec![section(
            "s",
            vec![
                bullet("a", Some(SourceKind::GitHub)),
                bullet("b", Some(SourceKind::GitHub)),
                bullet("c", Some(SourceKind::GitLab)),
            ],
        )]);
        let block = render_block(&d);

        // Headline first, ending with two newlines before the marker.
        assert!(
            block.starts_with("Most of the day was **GitHub** work — 2 items, 67% of today.\n\n"),
            "{block}"
        );

        // Begin / end markers wrap the SVG.
        assert!(
            block.contains(
                "<!-- dayseam:chart-begin (delete this block to drop the summary chart) -->\n<svg "
            ),
            "{block}"
        );
        assert!(
            block.ends_with("</svg>\n<!-- dayseam:chart-end -->\n\n"),
            "{block}"
        );
    }
}
