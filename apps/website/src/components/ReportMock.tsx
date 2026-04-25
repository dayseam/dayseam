import {
  motion,
  useMotionValue,
  useTransform,
  type MotionValue,
} from "framer-motion";
import { SHIPPING, type Connector } from "~/data/connectors";

/**
 * DAY-166. A stylised mock of the actual Dayseam desktop app — the
 * "order" that assembles on the far side of the black hole.
 *
 * Earlier revisions rendered a generic "daily report card": a
 * rounded container with some evidence bullets and a sparkline. It
 * looked pretty, but a visitor landing after the hero saw something
 * that did not match the app they would then download. The current
 * version mirrors the real desktop shell:
 *
 *   - Mac window chrome (traffic lights + frame)
 *   - `TitleBar`: "Dayseam" + subtitle copy
 *   - Left sidebar: "Sources" — the configured-sources strip from
 *     `apps/desktop/src/features/sources/SourcesSidebar.tsx`, with
 *     a connector logo + label + green health dot per source
 *   - Action row: date picker + Generate button
 *   - Report header: the date + template id
 *   - Kind-grouped bullet sections mirroring
 *     `apps/desktop/src/features/report/StreamingPreview.tsx`:
 *     each section has an ALL-CAPS tracked title, each source-kind
 *     group inside it has a `<emoji> <Label>` subheader, and each
 *     bullet has the small grey dot + text
 *   - A small "at a glance" donut chart summarising the day by
 *     source kind (this is a deliberate near-future product hint;
 *     the real app does not render one today, but the five-segment
 *     donut using real connector accents reinforces "five tools,
 *     one day, one picture")
 *
 * The mock is NOT a screenshot (screenshots go stale the moment
 * the app chrome changes, and they look uncanny at hero scale).
 * It is a faithful CSS reconstruction that keeps the same visual
 * rhythm as the app — kind emoji, ALL-CAPS section heads, grey
 * dots, template-id tooltip-equivalent — so a user who downloads
 * after watching the hero does not feel bait-and-switched when
 * the desktop window opens.
 *
 * Progressive reveal:
 *
 *   When a `scrollYProgress` MotionValue is supplied, each element
 *   fades + slides in at its own scroll-progress band, so the user
 *   watches the app assemble itself piece by piece as they scroll
 *   through act 3 (frame → title bar → sidebar → action row → two
 *   sections, each with its kind-groups → donut chart). Stagger is
 *   deliberate and narrative: the app "boots up" from chrome
 *   inward, mirroring how a real user onboards (window opens,
 *   sources wire up, date picked, report generates).
 *
 *   When `scrollYProgress` is omitted (the `prefers-reduced-motion`
 *   static fallback), every element renders at full opacity with
 *   no transform — the same stage directions without the vestibular
 *   cost. We still call every `useTransform` hook with a constant
 *   `useMotionValue(1)` so the hook order is identical between the
 *   animated and static modes; this keeps React happy.
 */

const CONNECTOR_BY_ID: Record<string, Connector> = Object.fromEntries(
  SHIPPING.map((c) => [c.id, c]),
) as Record<string, Connector>;

// Kind emoji mirrors `apps/desktop/src/features/report/
// StreamingPreview.tsx` → `SOURCE_KIND_EMOJI`. Keys are the
// PascalCase `SourceKind` values used in the app (NOT our
// lowercase `connector.id`) specifically so a grep on "GitHub"
// or a rename of a `SourceKind` in the desktop codebase surfaces
// this file as well. The marketing site's `Connector.kind`
// field carries the same PascalCase string; we key the lookup
// off `connector.kind` below.
//
// If you add or rename a `SourceKind` in the desktop app, update
// both this map and the `kind` field on the matching connector
// in `~/data/connectors.ts`. Hoisting these tokens into a
// shared package (`@dayseam/ui`) is the eventual fix and is
// tracked as a follow-up.
const KIND_EMOJI: Record<string, string> = {
  GitHub: "🐙",
  GitLab: "🦊",
  Jira: "📋",
  Confluence: "📄",
  LocalGit: "💻",
};

interface Bullet {
  readonly ref: string;
  readonly text: string;
}

interface KindGroup {
  readonly connectorId: string;
  readonly bullets: readonly Bullet[];
}

interface Section {
  readonly title: string;
  readonly groups: readonly KindGroup[];
}

// Copy here reads like real engineering standup output, not
// "Completed task X". Same philosophy as the pre-polish draft —
// users should recognise their day in the words — but now the
// bullets are distributed across sections + kind groups the way
// the real app renders them.
const COMMITS_SECTION: Section = {
  title: "COMMITS",
  groups: [
    {
      connectorId: "github",
      bullets: [
        { ref: "dayseam/dayseam@9fb3e21", text: "Polished the hero, Doppler boosted disk, larger falling icons" },
        { ref: "dayseam/dayseam@e32ee75", text: "Launched public marketing site with three act scroll animation" },
      ],
    },
    {
      connectorId: "localgit",
      bullets: [
        { ref: "spike/report-motion · 7 commits", text: "Prototyped the progressive report reveal before moving it to apps/website" },
      ],
    },
  ],
};

const PRS_SECTION: Section = {
  title: "PULL REQUESTS",
  groups: [
    {
      connectorId: "gitlab",
      bullets: [
        { ref: "platform!2411", text: "Reviewed the staging pipeline rollout, requested two changes on the retry policy" },
      ],
    },
    {
      connectorId: "jira",
      bullets: [
        { ref: "DAY-164", text: "Closed the back to back release race, master history stays linear" },
      ],
    },
    {
      connectorId: "confluence",
      bullets: [
        { ref: "ENG / Release runbook", text: "Updated the release playbook to cover the new force push guardrail" },
      ],
    },
  ],
};

// Donut values must sum to 100. Numbers are deliberately uneven
// (not 20/20/20/20/20) so the chart reads as "real data from your
// actual day" rather than a placeholder divided into fifths.
const DONUT: readonly { readonly connectorId: string; readonly pct: number }[] = [
  { connectorId: "github", pct: 34 },
  { connectorId: "gitlab", pct: 22 },
  { connectorId: "jira", pct: 18 },
  { connectorId: "confluence", pct: 14 },
  { connectorId: "localgit", pct: 12 },
];

/**
 * Convert a list of `(connectorId, pct)` entries into SVG arc
 * path-data strings for a donut chart centred at (cx, cy) with
 * outer radius `r` and inner radius `ri`. Returns one path per
 * segment, in input order, so the caller can paint each with its
 * connector's accent colour.
 *
 * Uses the standard SVG arc-flag trick: `large-arc-flag` is 1 when
 * the segment sweeps more than half the circle, 0 otherwise; the
 * `sweep-flag` for the outer arc is 1 (clockwise) and 0 for the
 * inner arc (counter-clockwise) so the two arcs enclose the
 * correct region. Pure function; no side effects.
 */
function donutPaths(
  segments: readonly { readonly pct: number }[],
  cx: number,
  cy: number,
  r: number,
  ri: number,
): string[] {
  const total = segments.reduce((acc, s) => acc + s.pct, 0) || 1;
  let angle = -Math.PI / 2;
  const paths: string[] = [];
  for (const seg of segments) {
    const sweep = (seg.pct / total) * Math.PI * 2;
    const start = angle;
    const end = angle + sweep;
    angle = end;
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

const DONUT_PATHS = donutPaths(DONUT, 50, 50, 48, 28);

export interface ReportMockProps {
  /** Optional scroll progress driving the per-element reveal. When
   *  omitted (e.g. in the reduced-motion static fallback) every
   *  element renders fully visible with no transform. */
  readonly scrollYProgress?: MotionValue<number>;
  readonly className?: string;
}

/**
 * The post-singularity reveal — Dayseam desktop app assembling
 * piece by piece. See module-level docstring for why this shape.
 */
export function ReportMock({
  scrollYProgress,
  className = "",
}: ReportMockProps): JSX.Element {
  // Fallback MotionValue so the useTransform hooks below always
  // have a real subscriber in the static-mode case. Value is fixed
  // at 1, which puts every scroll-driven ramp past its end stop
  // (e.g. `[0.62, 0.68] → [0, 1]` evaluates to 1), so every
  // element renders fully visible in the reduced-motion fallback
  // without us branching on mode inside every motion.div.
  const staticProgress = useMotionValue(1);
  const progress = scrollYProgress ?? staticProgress;

  // Progressive-reveal bands. Each element gets its own opacity +
  // y-offset curve tied to a scroll sub-range. Overlapping the
  // bands slightly (next band starts before previous finishes)
  // keeps the reveal feeling continuous rather than staged.
  //
  // Timing is coordinated with `Reveal` in `Hero.tsx`:
  //
  //   0.62-0.74  outer container scales 0.04 → 1 (the "app
  //              comes out of the black hole" beat — driven by
  //              Reveal, not here). The frame's own opacity
  //              fades in inside this range so the window
  //              materialises translucent out of the burst.
  //   0.74+      internals start appearing *inside the
  //              already-full-size window* — this is the
  //              explicit user ask: expand first, then fill.
  //
  // Exact boundaries tuned by manual scroll-through:
  //
  //   0.62-0.68  frame opacity  (window materialises out of burst
  //                              while Reveal's container scales
  //                              it up to full size)
  //   0.74-0.78  title bar      (app identity registers after
  //                              the frame is full-size)
  //   0.76-0.80  sidebar        (sources wire up)
  //   0.78-0.83  action row     (user input lands)
  //   0.82-0.87  commits section (evidence fills in)
  //   0.86-0.91  pull-request section (second kind group)
  //   0.90-0.97  donut chart    (summary resolves last)
  //
  // `frameY` is gone — earlier drafts translated the outer frame
  // 24px vertically in the same band as the fade, but with
  // Reveal's new scale-from-point (0.04 → 1) added on top, the
  // frame would jerk a visible amount at rendered-scale ≈ 0.08,
  // which reads as a glitch. Scale alone now carries the motion.
  const frameOpacity = useTransform(progress, [0.62, 0.68], [0, 1]);
  const titleOpacity = useTransform(progress, [0.74, 0.78], [0, 1]);
  const titleY = useTransform(progress, [0.74, 0.78], [12, 0]);
  const sidebarOpacity = useTransform(progress, [0.76, 0.8], [0, 1]);
  const sidebarX = useTransform(progress, [0.76, 0.8], [-16, 0]);
  const actionRowOpacity = useTransform(progress, [0.78, 0.83], [0, 1]);
  const actionRowY = useTransform(progress, [0.78, 0.83], [10, 0]);
  const commitsOpacity = useTransform(progress, [0.82, 0.87], [0, 1]);
  const commitsY = useTransform(progress, [0.82, 0.87], [14, 0]);
  const prOpacity = useTransform(progress, [0.86, 0.91], [0, 1]);
  const prY = useTransform(progress, [0.86, 0.91], [14, 0]);
  const donutOpacity = useTransform(progress, [0.9, 0.97], [0, 1]);
  const donutY = useTransform(progress, [0.9, 0.97], [14, 0]);

  return (
    <motion.div
      aria-label="Example Dayseam desktop window"
      className={
        "relative w-[min(92vw,720px)] overflow-hidden rounded-2xl " +
        "border border-white/10 bg-charcoal-900/95 " +
        "shadow-[0_40px_120px_-20px_rgba(0,0,0,0.8),0_0_0_1px_rgba(255,255,255,0.04)_inset] " +
        "backdrop-blur-sm " +
        className
      }
      style={{ opacity: frameOpacity }}
    >
      <MacTitleChrome />

      <motion.div style={{ opacity: titleOpacity, y: titleY }}>
        <AppTitleBar />
      </motion.div>

      {/* Mobile layout note: below the `sm` breakpoint (< 640px) we
       * hide the `Sources` sidebar entirely. At phone widths the
       * 160px sidebar + the main column is too squeezed to read —
       * the bullet copy is the payoff and the sidebar is decorative
       * reinforcement of "look, these connectors are configured",
       * which the hero's falling-icon rain already sells. The
       * sidebar returns on `sm+` where the chrome has room to
       * breathe. `hidden sm:block` wins over the motion `opacity`
       * style (display:none trumps transform), which is what we
       * want: on mobile the sidebar is gone, not just transparent. */}
      <div className="flex">
        <motion.div
          style={{ opacity: sidebarOpacity, x: sidebarX }}
          className="hidden w-[160px] shrink-0 border-r border-white/5 bg-charcoal-900/80 px-3 py-4 sm:block"
        >
          <SourcesSidebar />
        </motion.div>

        <div className="flex min-w-0 flex-1 flex-col px-4 py-4 sm:px-5">
          <motion.div style={{ opacity: actionRowOpacity, y: actionRowY }}>
            <ActionRow />
            <ReportHeader />
          </motion.div>

          <motion.div style={{ opacity: commitsOpacity, y: commitsY }}>
            <SectionBlock section={COMMITS_SECTION} />
          </motion.div>

          <motion.div style={{ opacity: prOpacity, y: prY }}>
            <SectionBlock section={PRS_SECTION} />
          </motion.div>

          <motion.div
            style={{ opacity: donutOpacity, y: donutY }}
            className="mt-5 rounded-lg border border-white/5 bg-charcoal-800/50 p-4"
          >
            <DonutSummary />
          </motion.div>
        </div>
      </div>
    </motion.div>
  );
}

// -----------------------------------------------------------------
// Sub-components
// -----------------------------------------------------------------

function MacTitleChrome(): JSX.Element {
  return (
    <div className="flex items-center gap-1.5 border-b border-white/5 bg-charcoal-800/70 px-3 py-2">
      <span className="h-3 w-3 rounded-full bg-[#ff5f57]" />
      <span className="h-3 w-3 rounded-full bg-[#febc2e]" />
      <span className="h-3 w-3 rounded-full bg-[#28c840]" />
    </div>
  );
}

function AppTitleBar(): JSX.Element {
  return (
    <header className="flex items-center justify-between border-b border-white/5 px-5 py-3">
      <div className="flex flex-col gap-0.5">
        <h3 className="text-[15px] font-semibold tracking-tight text-zinc-50">
          Dayseam
        </h3>
        <p className="text-[10px] text-zinc-500">
          Local first automated work reporting
        </p>
      </div>
      <div className="flex items-center gap-3 text-[10px] text-zinc-500">
        <span className="flex items-center gap-1">
          <span className="inline-block h-1.5 w-1.5 rounded-full bg-emerald-400" />
          Synced
        </span>
      </div>
    </header>
  );
}

function SourcesSidebar(): JSX.Element {
  return (
    <>
      <p className="mb-2 px-1 text-[9px] font-semibold uppercase tracking-[0.18em] text-zinc-500">
        Sources
      </p>
      <ul className="flex flex-col gap-1">
        {SHIPPING.map((connector) => (
          <li
            key={connector.id}
            className="flex items-center gap-2 rounded-md border border-dashed border-white/10 px-2 py-1.5"
          >
            <svg
              viewBox="0 0 24 24"
              width={14}
              height={14}
              fill={connector.accent}
              aria-hidden="true"
            >
              <path d={connector.path} />
            </svg>
            <span className="flex-1 truncate text-[11px] text-zinc-200">
              {connector.name}
            </span>
            <span
              className="inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-400"
              aria-hidden="true"
            />
          </li>
        ))}
        <li className="mt-1 flex items-center gap-1.5 px-2 py-1 text-[11px] text-zinc-500">
          <span aria-hidden="true">+</span>
          <span>Add source</span>
        </li>
      </ul>
    </>
  );
}

function ActionRow(): JSX.Element {
  return (
    <div className="mb-3 flex items-center justify-between gap-3 rounded-md border border-white/5 bg-charcoal-800/60 px-3 py-2">
      <div className="flex items-center gap-2">
        <span className="text-[10px] uppercase tracking-[0.14em] text-zinc-500">
          Date
        </span>
        <span className="rounded-sm bg-charcoal-700/80 px-2 py-0.5 font-mono text-[11px] text-zinc-200">
          2026/04/24
        </span>
        {/* "all sources" is a nice-to-have context tag — trim on
         * mobile where the action row would otherwise crowd the
         * Generate button off the right edge on <375px screens. */}
        <span className="hidden text-[10px] text-zinc-500 sm:inline">
          all sources
        </span>
      </div>
      <button
        type="button"
        disabled
        className="rounded-sm bg-cream px-3 py-1 text-[11px] font-semibold text-charcoal-900 shadow-sm"
      >
        Generate
      </button>
    </div>
  );
}

function ReportHeader(): JSX.Element {
  return (
    <div className="mb-4 flex items-center justify-between border-b border-white/5 pb-3">
      <h4 className="text-[15px] font-semibold text-zinc-50">
        April 24, 2026
      </h4>
      <span className="font-mono text-[10px] text-zinc-500">dev_eod_v1</span>
    </div>
  );
}

function SectionBlock({ section }: { section: Section }): JSX.Element {
  return (
    <section className="mb-4 flex flex-col gap-2">
      <h5 className="text-[11px] font-semibold uppercase tracking-[0.2em] text-zinc-400">
        {section.title}
      </h5>
      {section.groups.map((group) => (
        <KindGroupRow key={group.connectorId} group={group} />
      ))}
    </section>
  );
}

function KindGroupRow({ group }: { group: KindGroup }): JSX.Element {
  const connector = CONNECTOR_BY_ID[group.connectorId];
  if (!connector) return <></>;
  // Emoji lookup keys off the PascalCase `kind` (the app's
  // canonical `SourceKind`), not the lowercase `connector.id`,
  // so the map stays grep-aligned with the desktop app's
  // `SOURCE_KIND_EMOJI`. Fallback dot is only reachable for
  // connectors without a `kind` set (coming-soon placeholders
  // that shouldn't appear in the mock anyway).
  const emoji = connector.kind ? KIND_EMOJI[connector.kind] ?? "•" : "•";
  return (
    <div className="flex flex-col gap-1">
      <h6 className="flex items-center gap-1.5 text-[11px] font-medium text-zinc-400">
        <span aria-hidden="true">{emoji}</span>
        <span>{connector.name}</span>
      </h6>
      <ul className="flex flex-col gap-1 pl-1">
        {group.bullets.map((bullet) => (
          <li
            key={bullet.ref}
            className="flex items-start gap-2 text-[11.5px] leading-snug text-zinc-200"
          >
            <span
              aria-hidden="true"
              className="mt-1.5 inline-block h-1 w-1 shrink-0 rounded-full bg-zinc-500"
            />
            <div className="min-w-0 flex-1">
              <span>{bullet.text}</span>
              <span className="ml-2 font-mono text-[10px] text-zinc-500">
                {bullet.ref}
              </span>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}

function DonutSummary(): JSX.Element {
  // Stack donut above list on very narrow screens (below `sm`), so
  // on a 320-375px viewport the donut stays legible rather than
  // squishing the `AT A GLANCE` labels down a 130px-wide column.
  // On `sm+` the donut sits inline with the list (classic
  // chart + legend layout) which is what the app looks like.
  return (
    <div className="flex flex-col items-center gap-4 sm:flex-row sm:items-center">
      <svg
        viewBox="0 0 100 100"
        width={96}
        height={96}
        aria-hidden="true"
        className="shrink-0 drop-shadow-[0_0_20px_rgba(255,180,80,0.2)]"
      >
        {DONUT.map((seg, i) => {
          const connector = CONNECTOR_BY_ID[seg.connectorId];
          if (!connector) return null;
          return (
            <path
              key={seg.connectorId}
              d={DONUT_PATHS[i]}
              fill={connector.accent}
              opacity={0.92}
            />
          );
        })}
        <circle cx="50" cy="50" r="26" fill="#0E0E10" />
        <text
          x="50"
          y="47"
          textAnchor="middle"
          fontSize="10"
          fontWeight="600"
          fill="#F6F1E6"
        >
          Day
        </text>
        <text
          x="50"
          y="60"
          textAnchor="middle"
          fontSize="7"
          fill="#9ca3af"
          letterSpacing="1"
        >
          AT A GLANCE
        </text>
      </svg>
      <ul className="flex w-full flex-1 flex-col gap-1">
        {DONUT.map((seg) => {
          const connector = CONNECTOR_BY_ID[seg.connectorId];
          if (!connector) return null;
          return (
            <li
              key={seg.connectorId}
              className="flex items-center gap-2 text-[11px] text-zinc-300"
            >
              <span
                aria-hidden="true"
                className="inline-block h-2 w-2 rounded-full"
                style={{ backgroundColor: connector.accent }}
              />
              <span className="flex-1">{connector.name}</span>
              <span className="font-mono text-zinc-500">{seg.pct}%</span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
