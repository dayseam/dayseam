/**
 * DAY-166. Connector metadata for the marketing site.
 *
 * Two populations live here:
 *
 *   1. `SHIPPING` — the five connectors that land in today's
 *      release. Brand paths are duplicated (intentionally) from
 *      `apps/desktop/src/components/ConnectorLogo.tsx` rather than
 *      hoisted into `@dayseam/ui` as a shared package; the hoist
 *      is tracked as a follow-up, and duplicating seven lines of
 *      SVG path data is vastly cheaper than reshaping two
 *      workspaces in the same PR. If a connector's brand mark
 *      changes, update BOTH files — CI has no gate for this drift
 *      yet, it is a process note.
 *
 *   2. `COMING_SOON` — five placeholder connectors we show on the
 *      landing to communicate the "connector architecture leaves
 *      room to grow" narrative. These are NOT in the app yet;
 *      they render with a `coming soon` ribbon on the grid. The
 *      list (Slack, Teams, Linear, Word, Excel) was chosen during
 *      DAY-166 scoping because each maps to a clear work-evidence
 *      source — chat (Slack/Teams), project-tracking (Linear),
 *      and doc-editing (Word/Excel) — and collectively they tell
 *      the "every surface where work happens" story.
 *
 * Colour policy:
 *   Each connector's `accent` is the **real brand colour** of the
 *   service (adjusted only where the canonical hex would be
 *   invisible on a charcoal background — GitHub's near-black
 *   `#181717` becomes pure white, Confluence's very-dark navy
 *   `#172B4D` becomes the Atlassian sky `#2684FF`). The goal is
 *   instant recognition: a visitor should clock "that's GitHub,
 *   that's GitLab, that's Linear" in the hero icon-rain without
 *   having to read the chip label. The page's visual echo of the
 *   Convergence logo lives on in the accretion-disk conic
 *   gradient, the sparkline header, the narrative-copy accents,
 *   and the trust-strip labels — none of which are tied to the
 *   per-connector accent. Earlier drafts used the five strand
 *   hues as connector accents to reinforce the logo rhyme at the
 *   cost of recognisability; DAY-166 review flipped that
 *   trade-off.
 *
 * Every path in both populations is sourced from Simple Icons
 * (https://simpleicons.org), which is CC0-licensed. The marks
 * themselves remain the trademarks of their respective owners and
 * are used here in the classic "connected to X" nominative-fair-
 * use sense. See CREDITS.md for attribution.
 */

export interface Connector {
  /** Stable id used as React key and in data-* selectors. */
  readonly id: string;
  /** Brand name as it appears visibly on the marketing site.
   *  For shipping connectors this deliberately matches the app's
   *  `SOURCE_KIND_LABEL` (e.g. "Local git" with a lowercase "g",
   *  not "Local Git") so a user who sees the name on the landing
   *  and then opens the desktop app doesn't see a casing flip. */
  readonly name: string;
  /** Canonical `SourceKind` value from the desktop app
   *  (`apps/desktop/src/features/report/StreamingPreview.tsx` →
   *  `SOURCE_KIND_EMOJI` / `SOURCE_KIND_LABEL`). Populated for
   *  shipping connectors; undefined for coming-soon ones that
   *  don't yet correspond to a real `SourceKind`. Exposing this
   *  lets the marketing-site `ReportMock` key into the same
   *  PascalCase dictionary the app uses — a grep on "GitHub" in
   *  the desktop codebase now surfaces both files, so an app-side
   *  rename of a `SourceKind` can't silently drift from this
   *  mirror. */
  readonly kind?: string;
  /** Short one-line pitch for what this source contributes. */
  readonly pitch: string;
  /** Hex colour — drives the chip accent + hero trail colour.
   *  See the module docstring for the colour policy (real brand
   *  hex, with named substitutions where the canonical value is
   *  invisible on charcoal). */
  readonly accent: string;
  /** Simple Icons path data, 24x24 viewBox, `fill="currentColor"`. */
  readonly path: string;
}

// ---------------------------------------------------------------------------
// Shipping connectors (five strand hues from the Convergence mark).
// ---------------------------------------------------------------------------

const GITHUB_PATH =
  "M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12";

const GITLAB_PATH =
  "m23.6004 9.5927-.0337-.0862L20.3.9814a.851.851 0 0 0-.3362-.405.8748.8748 0 0 0-.9997.0539.8748.8748 0 0 0-.29.4399l-2.2055 6.748H7.5375l-2.2057-6.748a.8573.8573 0 0 0-.29-.4412.8748.8748 0 0 0-.9997-.0537.8585.8585 0 0 0-.3362.4049L.4332 9.5015l-.0325.0862a6.0657 6.0657 0 0 0 2.0119 7.0105l.0113.0087.03.0213 4.976 3.7264 2.462 1.8633 1.4995 1.1321a1.0085 1.0085 0 0 0 1.2197 0l1.4995-1.1321 2.4619-1.8633 5.006-3.7489.0125-.01a6.0682 6.0682 0 0 0 2.0094-7.003z";

const JIRA_PATH =
  "M12.004 0c-2.35 2.395-2.365 6.185.133 8.585l3.412 3.413-3.197 3.198a6.501 6.501 0 0 1 1.412 7.04l9.566-9.566a.95.95 0 0 0 0-1.344L12.004 0zm-1.748 1.74L.67 11.327a.95.95 0 0 0 0 1.344C4.45 16.44 8.22 20.244 12 24c2.295-2.298 2.395-6.096-.08-8.533l-3.47-3.469 3.2-3.2c-1.918-1.955-2.363-4.725-1.394-7.057z";

const CONFLUENCE_PATH =
  "M.87 18.257c-.248.382-.53.875-.763 1.245a.764.764 0 0 0 .255 1.04l4.965 3.054a.764.764 0 0 0 1.058-.26c.199-.332.454-.763.733-1.221 1.967-3.247 3.945-2.853 7.508-1.146l4.957 2.337a.764.764 0 0 0 1.028-.382l2.364-5.346a.764.764 0 0 0-.382-1 599.851 599.851 0 0 1-4.965-2.361C10.911 10.97 5.224 11.185.87 18.257zM23.131 5.743c.249-.405.531-.875.764-1.25a.764.764 0 0 0-.256-1.034L18.675.404a.764.764 0 0 0-1.058.26c-.195.335-.451.763-.734 1.225-1.966 3.246-3.945 2.85-7.508 1.146L4.437.694a.764.764 0 0 0-1.027.382L1.046 6.422a.764.764 0 0 0 .382 1c1.039.49 3.105 1.467 4.965 2.361 6.698 3.246 12.392 3.029 16.738-4.04z";

const GIT_PATH =
  "M23.546 10.93L13.067.452c-.604-.603-1.582-.603-2.188 0L8.708 2.627l2.76 2.76c.645-.215 1.379-.07 1.889.441.516.515.658 1.258.438 1.9l2.658 2.66c.645-.223 1.387-.078 1.9.435.721.72.721 1.884 0 2.604-.719.719-1.881.719-2.6 0-.539-.541-.674-1.337-.404-1.996L12.86 8.955v6.525c.176.086.342.203.488.348.713.721.713 1.883 0 2.6-.719.721-1.889.721-2.609 0-.719-.719-.719-1.879 0-2.598.182-.18.387-.316.605-.406V8.835c-.217-.091-.424-.222-.6-.401-.545-.545-.676-1.342-.396-2.009L7.636 3.7.45 10.881c-.6.605-.6 1.584 0 2.189l10.43 10.477c.604.604 1.582.604 2.186 0l10.48-10.43c.605-.603.605-1.582 0-2.187";

// ---------------------------------------------------------------------------
// Coming-soon connectors (Simple Icons paths, CC0).
// ---------------------------------------------------------------------------

// Slack — Simple Icons `slack` (CC0).
const SLACK_PATH =
  "M5.042 15.165a2.528 2.528 0 0 1-2.52 2.523A2.528 2.528 0 0 1 0 15.165a2.527 2.527 0 0 1 2.522-2.52h2.52v2.52zM6.313 15.165a2.527 2.527 0 0 1 2.521-2.52 2.527 2.527 0 0 1 2.521 2.52v6.313A2.528 2.528 0 0 1 8.834 24a2.528 2.528 0 0 1-2.521-2.522v-6.313zM8.834 5.042a2.528 2.528 0 0 1-2.521-2.52A2.528 2.528 0 0 1 8.834 0a2.528 2.528 0 0 1 2.521 2.522v2.52H8.834zM8.834 6.313a2.528 2.528 0 0 1 2.521 2.521 2.528 2.528 0 0 1-2.521 2.521H2.522A2.528 2.528 0 0 1 0 8.834a2.528 2.528 0 0 1 2.522-2.521h6.312zM18.956 8.834a2.528 2.528 0 0 1 2.522-2.521A2.528 2.528 0 0 1 24 8.834a2.528 2.528 0 0 1-2.522 2.521h-2.522V8.834zM17.688 8.834a2.528 2.528 0 0 1-2.523 2.521 2.527 2.527 0 0 1-2.52-2.521V2.522A2.527 2.527 0 0 1 15.165 0a2.528 2.528 0 0 1 2.523 2.522v6.312zM15.165 18.956a2.528 2.528 0 0 1 2.523 2.522A2.528 2.528 0 0 1 15.165 24a2.527 2.527 0 0 1-2.52-2.522v-2.522h2.52zM15.165 17.688a2.527 2.527 0 0 1-2.52-2.523 2.526 2.526 0 0 1 2.52-2.52h6.313A2.527 2.527 0 0 1 24 15.165a2.528 2.528 0 0 1-2.522 2.523h-6.313z";

// Microsoft Teams — Simple Icons `microsoftteams` (CC0).
const TEAMS_PATH =
  "M20.625 8.127c1.524 0 2.758-1.235 2.758-2.759C23.383 3.844 22.149 2.61 20.625 2.61c-1.523 0-2.757 1.234-2.757 2.758 0 1.524 1.234 2.759 2.757 2.759zm-.93 1.035h-5.53a.77.77 0 0 0-.77.77v6.275a4.363 4.363 0 0 0 4.134 4.351 4.252 4.252 0 0 0 4.365-4.25v-5.81a1.335 1.335 0 0 0-1.336-1.336h-.863zm-9.188-3.506c1.923 0 3.481-1.558 3.481-3.481C13.988 1.16 12.43 0 10.508 0 8.585 0 7.027 1.558 7.027 3.481c0 1.923 1.558 3.481 3.48 3.481zM14.2 8.13H3.156C1.77 8.13.63 9.27.63 10.655v7.345c0 3.045 2.476 5.52 5.52 5.52 3.045 0 5.52-2.475 5.52-5.52v-7.345c0-1.385-1.14-2.525-2.526-2.525z";

// Linear — Simple Icons `linear` (CC0).
const LINEAR_PATH =
  "M.403 13.265C.143 12.207 0 11.12 0 10C0 4.477 4.477 0 10 0c1.12 0 2.207.143 3.265.403L.403 13.265zm1.232 3.273A10.01 10.01 0 0 0 5.08 21.983L21.983 5.08A10.01 10.01 0 0 0 15.735 1.635L1.635 15.735 1.635 16.538zm4.965 5.822L22.36 8.6A10.01 10.01 0 0 1 23.597 15.735L15.735 23.597a10.01 10.01 0 0 1-7.135-1.236zm8.005 1.598L23.336 15.167a10 10 0 0 1-8.731 8.731zM10 20a10 10 0 0 1-6-2l8-8 6 6a10 10 0 0 1-8 4z";

// Microsoft Word — Simple Icons `microsoftword` (CC0).
// Slightly simplified (outer + stroke) mark so it stays legible at
// 24px in the hero grid even with antialiasing.
const WORD_PATH =
  "M21.6 0H7.2A2.4 2.4 0 0 0 4.8 2.4v2.4H2.4A2.4 2.4 0 0 0 0 7.2v9.6a2.4 2.4 0 0 0 2.4 2.4h2.4v2.4A2.4 2.4 0 0 0 7.2 24h14.4a2.4 2.4 0 0 0 2.4-2.4V2.4A2.4 2.4 0 0 0 21.6 0zM4.8 17.999L3.2 12l-1.2 5.999h-.8L0 10.8h.8l.8 5.2 1.2-5.2h.8l1.2 5.2.8-5.2h.8l-1.2 7.199h-.6zm16.8 3.601H7.2v-2.4h7.2A2.4 2.4 0 0 0 16.8 16.8V7.2A2.4 2.4 0 0 0 14.4 4.8H7.2V2.4h14.4v19.2z";

// Microsoft Excel — Simple Icons `microsoftexcel` (CC0).
const EXCEL_PATH =
  "M21.53 4.306v15.363q0 .807-.472 1.433-.472.627-1.253.85l-9.888 2.75v-4.023H4.23q-.812 0-1.39-.578-.579-.579-.579-1.39V6.25q0-.811.579-1.39.578-.579 1.39-.579h5.687V1.258L20.635.139q.823-.114 1.398.347.577.461.577 1.284v2.536zM9.917 15.39l1.79 3.56h1.945L10.85 13.57l2.688-5.217h-1.859L9.945 12.056 8.328 8.353h-1.89l2.589 5.2-2.788 5.397h1.895l1.782-3.562z";

// Real brand accent hexes. Where the canonical Simple Icons value
// disappears on a charcoal background, we substitute a visible
// in-family alternative (noted per-row). The goal is instant
// "that's Service X" recognition in the hero icon-rain.
export const SHIPPING: readonly Connector[] = [
  {
    id: "github",
    kind: "GitHub",
    // GitHub's canonical mark is #181717 (near-black); rendered on
    // charcoal it would vanish entirely. Use pure white — which
    // is how GitHub itself renders the octocat on dark surfaces.
    name: "GitHub",
    pitch: "Pull requests, reviews, commits across all your repos",
    accent: "#FFFFFF",
    path: GITHUB_PATH,
  },
  {
    id: "gitlab",
    kind: "GitLab",
    name: "GitLab",
    pitch: "Merge requests, pipelines, issues, self hosted or cloud",
    // Simple Icons canonical GitLab orange.
    accent: "#FC6D26",
    path: GITLAB_PATH,
  },
  {
    id: "jira",
    kind: "Jira",
    name: "Jira",
    pitch: "The tickets you moved, the ones blocking you, the ones you closed",
    // Simple Icons canonical Atlassian blue for Jira.
    accent: "#0052CC",
    path: JIRA_PATH,
  },
  {
    id: "confluence",
    kind: "Confluence",
    name: "Confluence",
    pitch: "Docs you wrote, comments you left, pages you touched",
    // Simple Icons canonical Confluence hex is #172B4D (very dark
    // navy), which merges with the charcoal background. Use the
    // Atlassian sky `#2684FF` — a same-family blue with enough
    // lightness to pop against #17171A while also differentiating
    // visually from Jira's deeper #0052CC.
    accent: "#2684FF",
    path: CONFLUENCE_PATH,
  },
  {
    id: "localgit",
    kind: "LocalGit",
    // Lowercase "git" deliberately — matches the desktop app's
    // `SOURCE_KIND_LABEL.LocalGit = "Local git"`. An earlier draft
    // used "Local Git" (proper-noun casing) and was flagged on
    // review as a bait-and-switch vs what the downloaded app
    // shows. Keep these two in sync.
    name: "Local git",
    pitch: "Commits on laptops that never left the laptop",
    // Simple Icons canonical Git mark red-orange.
    accent: "#F05032",
    path: GIT_PATH,
  },
];

export const COMING_SOON: readonly Connector[] = [
  {
    id: "slack",
    name: "Slack",
    pitch: "Threads you drove, decisions you surfaced, standups you wrote",
    // Slack's primary mark is a four-colour cube; we can only
    // render one hex per single-path SVG. The pink accent #E01E5A
    // is the most recognisable-as-Slack single-colour option on a
    // dark background (the aubergine #4A154B reads muddy). Users
    // still clock the cube silhouette + pink → "Slack".
    accent: "#E01E5A",
    path: SLACK_PATH,
  },
  {
    id: "teams",
    name: "Microsoft Teams",
    pitch: "The back and forth that ends up driving half your week",
    // Simple Icons canonical Teams purple.
    accent: "#6264A7",
    path: TEAMS_PATH,
  },
  {
    id: "linear",
    name: "Linear",
    pitch: "Issues you shipped, cycles you closed, specs you wrote",
    // Simple Icons canonical Linear indigo.
    accent: "#5E6AD2",
    path: LINEAR_PATH,
  },
  {
    id: "word",
    name: "Microsoft Word",
    pitch: "Documents you authored or reviewed across SharePoint",
    // Simple Icons canonical Word blue.
    accent: "#2B579A",
    path: WORD_PATH,
  },
  {
    id: "excel",
    name: "Microsoft Excel",
    pitch: "Spreadsheets you updated, where half of ops actually lives",
    // Simple Icons canonical Excel green.
    accent: "#21A366",
    path: EXCEL_PATH,
  },
];

/** Convenience list used by the hero animation — every connector
 *  the site renders, in the order they cascade down the viewport
 *  during act 1. Shipping connectors come first so the visual
 *  density centres them, coming-soon drift in at the edges. */
export const ALL_CONNECTORS: readonly Connector[] = [
  ...SHIPPING,
  ...COMING_SOON,
];
