//! Structured [`ReportDraft`] → markdown-body text.
//!
//! `dayseam-report` produces the structural draft (sections + bullets +
//! evidence) and deliberately leaves final markdown assembly to the sink.
//! Keeping the assembly local means each sink gets to pick its own dialect
//! (bullet glyph, heading level, blank-line convention) without coordinating
//! with the renderer, and the rendered bytes never leak out of the sink that
//! owns them.
//!
//! The dialect used by this sink is:
//!
//! ```text
//! Most of the day was **GitHub** work — 4 items, 67% of today.
//!
//! <!-- dayseam:chart-begin (delete this block to drop the summary chart) -->
//! <svg …>…</svg>
//! <!-- dayseam:chart-end -->
//!
//! ## Section title
//!
//! ### 💻 Local git
//!
//! - bullet one
//! - bullet two
//!
//! ### 🐙 GitHub
//!
//! - bullet three
//! ```
//!
//! The leading headline + delimited inline SVG is the *day-summary
//! block* (DAY-214); see [`crate::summary_chart`] for shape, theming,
//! and rationale. The block is omitted entirely when the day has no
//! chart-eligible bullets, so a quiet day's saved file starts at
//! `## Section title` exactly the way pre-DAY-214 files did.
//!
//! Every bullet renders under a `### <emoji> <Label>` subheading named
//! after its [`dayseam_core::SourceKind`] (DAY-104). The group order
//! follows [`SourceKind::render_order`] so a day with activity from two
//! forges renders deterministically; single-kind sections still emit
//! one `### <Kind>` group for layout parity (the user opted into
//! "always group" during design). Bullets whose `source_kind` is
//! `None` — the only realistic source is a pre-DAY-104 draft
//! deserialised from SQLite, see `RenderedBullet::source_kind` —
//! render at the bottom of the section under no subheading; the
//! degradation is visible but non-destructive.
//!
//! One blank line between the section heading and the first subheading,
//! one blank line between each subheading and its bullets, one blank
//! line between adjacent subgroups, one blank line between adjacent
//! sections, and a single trailing newline on the whole fragment so
//! the marker-block end delimiter lands on its own line.

use dayseam_core::{RenderedBullet, RenderedSection, ReportDraft, SourceKind};

use crate::summary_chart;

/// Substrings that, if they appeared verbatim on their own logical
/// line in the marker-block body, would be parsed as Dayseam's own
/// begin/end markers on the next read — turning a malicious
/// upstream commit message or MR title into a marker-block-injection
/// primitive. The bullet renderer always prefixes lines with `"- "`
/// (so the `<!-- dayseam:…-->` substring is never line-leading), but
/// upstream-controlled `text` fields can carry literal `\n`
/// characters and the line *after* the embedded newline lands
/// unprefixed in the body.
///
/// The fix is to neutralise these tokens at the sink boundary:
/// every literal `<!--` in a bullet's `text` is rewritten to the
/// HTML entity `&lt;!--` before the bullet hits the file. Markdown
/// readers render `&lt;` and `<` identically, so the user sees the
/// same characters they typed, but the marker parser's literal
/// substring match no longer fires.
///
/// We also collapse interior `\r` / `\n` to a single space so a
/// commit message containing `\n<!-- dayseam:end -->\n` cannot
/// land as a free-standing line in the rendered body even before
/// the entity rewrite kicks in. Together the two passes leave
/// zero degrees of freedom for an upstream attacker (or
/// well-meaning user with a `<!--` in their MR title) to break
/// the marker-block contract.
const MARKER_TOKEN: &str = "<!--";
const MARKER_TOKEN_NEUTRALISED: &str = "&lt;!--";

/// Render the body that lives between the begin and end markers:
/// the day-summary block (when there's anything to summarise)
/// followed by the section / per-kind / bullet structure. Does
/// *not* include the marker lines themselves — that is the
/// caller's responsibility.
///
/// The summary block is prepended unconditionally; the sub-helper
/// returns the empty string for a draft with no chart-eligible
/// bullets, so a "quiet" day's body still starts at the first
/// `## Section title` exactly the way pre-DAY-214 files did.
pub(crate) fn render_body(draft: &ReportDraft) -> String {
    let mut out = summary_chart::render_block(draft);
    out.push_str(&render_sections(draft));
    out
}

/// Render only the section / per-kind / bullet structure. Split
/// from [`render_body`] so the per-section unit tests can assert
/// on a stable string without having to re-thread the whole
/// summary block — the summary block is covered by its own
/// dedicated tests in [`crate::summary_chart`] and by the
/// integration test below.
pub(crate) fn render_sections(draft: &ReportDraft) -> String {
    let mut out = String::new();
    for (idx, section) in draft.sections.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        render_section(&mut out, section);
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn render_section(out: &mut String, section: &RenderedSection) {
    out.push_str("## ");
    out.push_str(&section.title);
    out.push_str("\n\n");

    if section.bullets.is_empty() {
        return;
    }

    let groups = group_bullets_by_kind(&section.bullets);
    for (group_idx, (kind, bullets)) in groups.iter().enumerate() {
        if group_idx > 0 {
            out.push('\n');
        }
        if let Some(kind) = kind {
            out.push_str("### ");
            out.push_str(&kind.display_with_emoji());
            out.push_str("\n\n");
        }
        for bullet in bullets {
            out.push_str("- ");
            out.push_str(&sanitise_bullet_text(&bullet.text));
            out.push('\n');
        }
    }
}

/// Neutralise marker-injection vectors in a bullet's user-facing
/// text. See [`MARKER_TOKEN`] for the rationale. Two passes:
///
/// 1. Replace interior `\r` / `\n` with a single space so an
///    upstream-controlled commit message like
///    `"fix: bug\n<!-- dayseam:end -->\n"` cannot break the bullet
///    into multiple body lines, the second of which would otherwise
///    render with no `- ` prefix and thus match the trim-equal
///    end-marker test in [`crate::markers::parse`].
/// 2. Rewrite any remaining `<!--` to `&lt;!--`. Markdown readers
///    render the entity identically to a literal `<`, so the user
///    still sees the original text, but the parser's literal
///    substring match no longer fires.
fn sanitise_bullet_text(text: &str) -> String {
    let mut buf = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\r' | '\n' => buf.push(' '),
            other => buf.push(other),
        }
    }
    if buf.contains(MARKER_TOKEN) {
        buf = buf.replace(MARKER_TOKEN, MARKER_TOKEN_NEUTRALISED);
    }
    buf
}

/// Group a section's bullets by [`SourceKind`], preserving per-group
/// bullet order and emitting groups in [`SourceKind::render_order`].
/// Returns `(None, bullets)` for bullets whose `source_kind` is
/// `None` — the only realistic source is a pre-DAY-104 draft,
/// which renders under the section heading without a `### <Kind>`
/// subheading (see module docs).
///
/// The function is intentionally allocation-heavy for a render-time
/// hot path (one `Vec<&RenderedBullet>` per kind) because the payoff
/// is two call sites — this sink and `StreamingPreview` — reading the
/// same structure; a zero-alloc iterator would save microseconds at
/// the cost of duplicating the bucketing logic on the frontend.
fn group_bullets_by_kind(
    bullets: &[RenderedBullet],
) -> Vec<(Option<SourceKind>, Vec<&RenderedBullet>)> {
    let order = SourceKind::render_order();
    let mut groups: Vec<(Option<SourceKind>, Vec<&RenderedBullet>)> =
        order.iter().map(|k| (Some(*k), Vec::new())).collect();
    let mut unattributed: Vec<&RenderedBullet> = Vec::new();

    for bullet in bullets {
        match bullet.source_kind {
            Some(kind) => {
                if let Some(bucket) = groups.iter_mut().find(|(k, _)| *k == Some(kind)) {
                    bucket.1.push(bullet);
                }
            }
            None => unattributed.push(bullet),
        }
    }

    groups.retain(|(_, v)| !v.is_empty());
    if !unattributed.is_empty() {
        groups.push((None, unattributed));
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};
    use dayseam_core::{RenderedBullet, RenderedSection};
    use std::collections::HashMap;
    use uuid::Uuid;

    fn draft_with_sections(sections: Vec<RenderedSection>) -> ReportDraft {
        ReportDraft {
            id: Uuid::nil(),
            date: NaiveDate::from_ymd_opt(2026, 4, 18).unwrap(),
            template_id: "dayseam.dev_eod".to_string(),
            template_version: "2026-04-18".to_string(),
            sections,
            evidence: Vec::new(),
            per_source_state: HashMap::new(),
            verbose_mode: false,
            generated_at: Utc::now(),
        }
    }

    fn bullet(id: &str, text: &str, kind: Option<SourceKind>) -> RenderedBullet {
        RenderedBullet {
            id: id.to_string(),
            text: text.to_string(),
            source_kind: kind,
        }
    }

    // Per-section tests scope to `render_sections` so they pin the
    // section / per-kind / bullet structure without re-asserting
    // the day-summary block on every case — the summary block has
    // its own focused tests in [`crate::summary_chart::tests`] and
    // the integration test at the bottom of this module.

    #[test]
    fn single_kind_section_renders_one_subheading_then_bullets() {
        let section = RenderedSection {
            id: "commits".to_string(),
            title: "Commits".to_string(),
            bullets: vec![
                bullet("b1", "first bullet", Some(SourceKind::LocalGit)),
                bullet("b2", "second bullet", Some(SourceKind::LocalGit)),
            ],
        };
        let out = render_sections(&draft_with_sections(vec![section]));
        assert_eq!(
            out,
            "## Commits\n\n### 💻 Local git\n\n- first bullet\n- second bullet\n"
        );
    }

    #[test]
    fn multi_kind_section_groups_bullets_in_render_order() {
        // LocalGit + GitHub + GitLab mixed in the same section; the
        // sink must emit them in [`SourceKind::render_order`] —
        // LocalGit → GitHub → GitLab — regardless of input order.
        let section = RenderedSection {
            id: "commits".to_string(),
            title: "Commits".to_string(),
            bullets: vec![
                bullet("b_gl", "gitlab commit", Some(SourceKind::GitLab)),
                bullet("b_lg", "local commit", Some(SourceKind::LocalGit)),
                bullet("b_gh", "github commit", Some(SourceKind::GitHub)),
            ],
        };
        let out = render_sections(&draft_with_sections(vec![section]));
        assert_eq!(
            out,
            "## Commits\n\n\
             ### 💻 Local git\n\n- local commit\n\n\
             ### 🐙 GitHub\n\n- github commit\n\n\
             ### 🦊 GitLab\n\n- gitlab commit\n"
        );
    }

    #[test]
    fn multiple_sections_are_separated_by_blank_line() {
        let a = RenderedSection {
            id: "a".to_string(),
            title: "Alpha".to_string(),
            bullets: vec![bullet("ba", "one", Some(SourceKind::LocalGit))],
        };
        let b = RenderedSection {
            id: "b".to_string(),
            title: "Beta".to_string(),
            bullets: vec![bullet("bb", "two", Some(SourceKind::Jira))],
        };
        let out = render_sections(&draft_with_sections(vec![a, b]));
        assert_eq!(
            out,
            "## Alpha\n\n### 💻 Local git\n\n- one\n\n## Beta\n\n### 📋 Jira\n\n- two\n"
        );
    }

    #[test]
    fn empty_section_still_renders_heading_without_subheading() {
        // The fully-empty-day fallback in `dayseam-report` goes
        // through `empty_section`, which always seeds one bullet;
        // this case (literally zero bullets) is more defensive than
        // exercised, but the `## <Title>\n\n` shape is pinned so a
        // future regression that does emit an empty section stays
        // layout-compatible with the old v0.4 output.
        let section = RenderedSection {
            id: "x".to_string(),
            title: "Nothing yet".to_string(),
            bullets: Vec::new(),
        };
        let out = render_sections(&draft_with_sections(vec![section]));
        assert_eq!(out, "## Nothing yet\n\n");
    }

    #[test]
    fn legacy_bullets_without_source_kind_render_below_any_attributed_groups() {
        // Upgrade case: an old draft (pre-DAY-104) stored in SQLite
        // has some bullets whose `source_kind` is `None`. The sink
        // renders attributed bullets first under their `### <Kind>`
        // subheading, then the unattributed tail without a
        // subheading. The degradation is visible but non-destructive.
        let section = RenderedSection {
            id: "commits".to_string(),
            title: "Commits".to_string(),
            bullets: vec![
                bullet("b_new", "new-render bullet", Some(SourceKind::LocalGit)),
                bullet("b_old", "pre-DAY-104 bullet", None),
            ],
        };
        let out = render_sections(&draft_with_sections(vec![section]));
        assert_eq!(
            out,
            "## Commits\n\n### 💻 Local git\n\n- new-render bullet\n\n- pre-DAY-104 bullet\n"
        );
    }

    // ---- render_body integration --------------------------------

    #[test]
    fn render_body_prepends_summary_block_above_sections_when_chart_eligible() {
        // A day with at least one attributed bullet must emit the
        // headline + delimited SVG ABOVE the first `## Section`
        // heading. We don't pin the SVG geometry here — that's
        // covered byte-for-byte by `summary_chart::tests` — but we
        // do assert on the structural ordering and the marker
        // shape so a refactor that moves the chart to the bottom
        // of the body, or strips the markers, fails here.
        let section = RenderedSection {
            id: "commits".to_string(),
            title: "Commits".to_string(),
            bullets: vec![bullet("b_lg", "local", Some(SourceKind::LocalGit))],
        };
        let out = render_body(&draft_with_sections(vec![section]));
        let begin_marker = out
            .find("<!-- dayseam:chart-begin")
            .expect("chart-begin marker");
        let end_marker = out
            .find("<!-- dayseam:chart-end")
            .expect("chart-end marker");
        let section_heading = out.find("## Commits").expect("section heading");
        assert!(
            begin_marker < end_marker,
            "begin marker must precede end marker"
        );
        assert!(
            end_marker < section_heading,
            "summary block must sit above the first section: {out}"
        );
        assert!(
            out.starts_with("Today was all **Local git**"),
            "body must lead with the headline paragraph: {out}"
        );
    }

    /// DAY-187 H4: an upstream-controlled bullet `text` that
    /// contains a literal Dayseam marker token (e.g. a commit
    /// message that quotes the marker syntax for documentation,
    /// or a malicious MR title from a co-tenant on a shared GitLab
    /// instance) must NOT round-trip back into the parser as a
    /// real begin/end marker on the next read. The sink rewrites
    /// `<!--` to `&lt;!--` at the body-render boundary; markdown
    /// renders both identically.
    #[test]
    fn bullet_text_with_marker_token_is_neutralised() {
        let section = RenderedSection {
            id: "commits".to_string(),
            title: "Commits".to_string(),
            bullets: vec![bullet(
                "b1",
                "fix: oops <!-- dayseam:end -->",
                Some(SourceKind::LocalGit),
            )],
        };
        let out = render_sections(&draft_with_sections(vec![section]));
        assert!(
            !out.contains("<!-- dayseam:end -->"),
            "bullet text must not allow marker injection, got: {out}"
        );
        assert!(
            out.contains("&lt;!-- dayseam:end --&gt;") || out.contains("&lt;!-- dayseam:end -->"),
            "neutralised form must be present, got: {out}"
        );
    }

    /// DAY-187 H4 (companion): an upstream-controlled bullet text
    /// that embeds a `\n` character cannot split into a second
    /// free-standing body line. The sink collapses interior
    /// newlines to a single space at the body-render boundary so
    /// the bullet always emits exactly one `- <text>\n` line.
    #[test]
    fn bullet_text_with_embedded_newline_collapses_to_one_line() {
        let section = RenderedSection {
            id: "commits".to_string(),
            title: "Commits".to_string(),
            bullets: vec![bullet(
                "b1",
                "fix: oops\nfollow-up note",
                Some(SourceKind::LocalGit),
            )],
        };
        let out = render_sections(&draft_with_sections(vec![section]));
        // Exactly one bullet line, joined by a single space.
        let bullet_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(bullet_lines, vec!["- fix: oops follow-up note"]);
    }

    #[test]
    fn render_body_omits_summary_block_for_chart_ineligible_drafts() {
        // A pre-DAY-104 draft (every bullet has `source_kind: None`)
        // has nothing chart-eligible to summarise; the body must
        // start at `## Section` exactly the way pre-DAY-214 files
        // did, so re-saving an old report doesn't insert an empty
        // chart marker pair into a file the user has been editing.
        let section = RenderedSection {
            id: "commits".to_string(),
            title: "Commits".to_string(),
            bullets: vec![bullet("b_old", "legacy", None)],
        };
        let out = render_body(&draft_with_sections(vec![section]));
        assert!(
            !out.contains("dayseam:chart-begin"),
            "chart marker must be elided for chart-ineligible drafts: {out}"
        );
        assert!(
            out.starts_with("## Commits"),
            "body must lead with the section heading when no chart: {out}"
        );
    }
}
