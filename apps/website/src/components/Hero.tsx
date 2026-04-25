import { useRef } from "react";
import {
  motion,
  useReducedMotion,
  useScroll,
  useTransform,
  type MotionValue,
} from "framer-motion";
import { ALL_CONNECTORS, type Connector } from "~/data/connectors";
import { ReportMock } from "./ReportMock";

/**
 * DAY-166. The scroll-driven "chaos → black hole → order" hero.
 *
 * Narrative (three acts, camera steady, viewport pinned):
 *
 *   Act 1  (scroll 0.00 → 0.35): work-evidence icons rain down
 *          from the top. Staggered, varied rotation, deliberately
 *          chaotic. This is "your day, scattered across ten tools".
 *
 *   Act 2  (scroll 0.25 → 0.65): a singularity forms at mid-frame.
 *          An accretion disk of the five strand colours swirls
 *          continuously. Each falling icon bends toward the disk,
 *          spirals inward, scales to zero and blurs as it crosses
 *          the event horizon.
 *
 *   Act 3  (scroll 0.55 → 1.00): light burst, then an ordered
 *          Dayseam report materialises in the same screen space —
 *          same icons, now reorganised into evidence-linked bullets
 *          with a sparkline header. "One report, sourced from
 *          everything."
 *
 * Implementation notes:
 *
 *   - Animation is powered by `useScroll` bound to the outer
 *     300vh section; `useTransform` drives per-element MotionValues
 *     from the 0→1 scroll progress. Every animated property is a
 *     `transform` or `opacity` — never `width`/`height`/`top`/`left`
 *     — so scroll stays on the compositor and does not thrash
 *     layout.
 *
 *   - Icon positions are **deterministic** (`buildTrajectory` is a
 *     pure function of index). No `Math.random()`; SSR and client
 *     render identical markup, so React hydration never warns.
 *
 *   - The pinned layout is a `sticky` child inside a tall parent;
 *     this is the cheapest cross-browser scroll-pin pattern and
 *     does not require `position: pin` CSS from the GSAP
 *     ScrollTrigger school. Works on iOS Safari, which is the
 *     platform most likely to surprise us.
 *
 *   - Reduced-motion users get `HeroStatic` — three stacked panels
 *     telling the same story without motion. This is required by
 *     the review brief; do not collapse it into "just slower
 *     animation", `prefers-reduced-motion` is about vestibular
 *     safety, not performance.
 */

// ---------------------------------------------------------------------------
// Act 1: individual connector icon, falling then spiralling in.
// ---------------------------------------------------------------------------

interface FallingIconProps {
  readonly connector: Connector;
  readonly index: number;
  readonly total: number;
  readonly scrollYProgress: MotionValue<number>;
}

interface Trajectory {
  readonly startX: number;
  readonly endX: number;
  readonly startY: number;
  readonly midY: number;
  readonly spinStart: number;
  readonly spinEnd: number;
  readonly scale: number;
  readonly entryDelay: number;
}

/**
 * Deterministic per-icon layout. Icons start spread horizontally
 * across the viewport, rain down to roughly the vertical midpoint,
 * then spiral inward to (0, 0) in act 2. Using a pure index-based
 * trajectory (no RNG) means SSR markup matches client markup.
 */
function buildTrajectory(index: number, total: number): Trajectory {
  const normalised = (index + 0.5) / total;
  const startX = -45 + normalised * 90;
  const xJitter = Math.sin(index * 2.39) * 5;
  const startY = -60 - (index % 4) * 12;
  const midY = -10 + Math.cos(index * 1.73) * 8;
  const spinStart = (index * 37) % 360;
  const spinDirection = index % 2 === 0 ? 1 : -1;
  const spinEnd = spinStart + spinDirection * 180;
  const scale = 0.9 + (index % 3) * 0.08;
  const entryDelay = (index / total) * 0.25;
  return {
    startX: startX + xJitter,
    endX: 0,
    startY,
    midY,
    spinStart,
    spinEnd,
    scale,
    entryDelay,
  };
}

function FallingIcon({
  connector,
  index,
  total,
  scrollYProgress,
}: FallingIconProps): JSX.Element {
  const trajectory = buildTrajectory(index, total);

  // Horizontal: drift toward centre (x=0) as the icon crosses the
  // event horizon in act 2.
  const x = useTransform(
    scrollYProgress,
    [0, 0.35, 0.6],
    [trajectory.startX, trajectory.startX * 0.55, trajectory.endX],
  );

  // Vertical: fall from above viewport into mid-frame during act 1,
  // then continue toward centre as it spirals inward.
  const y = useTransform(
    scrollYProgress,
    [0, 0.3, 0.55],
    [trajectory.startY, trajectory.midY, 0],
  );

  // Rotation: always turning; accelerates into act 2 to sell the
  // "falling into a well" spiral. Expressed in degrees.
  const rotate = useTransform(
    scrollYProgress,
    [0, 0.35, 0.6],
    [trajectory.spinStart, trajectory.spinStart + 90, trajectory.spinEnd + 360],
  );

  // Scale + opacity: icons appear during the entry ramp, stay
  // visible through act 1, then shrink + fade as they cross the
  // event horizon. Scale 0 at ~0.6 is the "gone" moment.
  const entryStart = trajectory.entryDelay;
  const entryEnd = trajectory.entryDelay + 0.12;
  const scale = useTransform(
    scrollYProgress,
    [entryStart, entryEnd, 0.5, 0.62],
    [0, trajectory.scale, trajectory.scale, 0],
  );
  const opacity = useTransform(
    scrollYProgress,
    [entryStart, entryEnd, 0.5, 0.6],
    [0, 1, 1, 0],
  );
  const blur = useTransform(scrollYProgress, [0.45, 0.6], [0, 6]);

  // Icon render strategy (DAY-166 revision):
  //
  // Earlier drafts wrapped each brand mark in a rounded tile with a
  // translucent accent-colour background + inner ring + outer glow.
  // That read as "chip of accent colour" rather than "the service
  // itself" — the tile was louder than the mark it contained. The
  // current render drops the tile entirely and shows just the raw
  // brand SVG at 80px (up from a 52px tile housing a 32px mark) with
  // a two-layer `drop-shadow` filter in the connector's accent
  // colour: a tight 4px halo for definition + a wide 18px glow so
  // the silhouette reads as emissive rather than flat. This is what
  // lets a viewer clock "that's GitHub falling in, that's GitLab"
  // in the half-second each icon is on-screen.
  //
  // `drop-shadow` (not `box-shadow`) is deliberate: drop-shadow
  // follows the alpha channel of the SVG path, so the glow hugs the
  // Octocat or fox silhouette rather than squaring around the
  // bounding box. box-shadow would have leaked as a rectangle.
  //
  // The `blur` filter (applied as the icon crosses the event horizon
  // in act 2) is composed into the same `filter` string because CSS
  // `filter` is a single property — multiple functions are allowed,
  // but multiple `filter` declarations would overwrite each other.
  const iconSize = 80;
  const filter = useTransform(blur, (b) => {
    const shadows =
      `drop-shadow(0 0 18px ${connector.accent}cc)` +
      ` drop-shadow(0 0 4px ${connector.accent}ff)`;
    return b > 0.01 ? `${shadows} blur(${b}px)` : shadows;
  });

  // Hoist the vw/vh string conversions out of the `style={{…}}`
  // literals below. Calling `useTransform(x, (v) => \`${v}vw\`)`
  // inline inside `style` would work — hooks run in render, so
  // the ordering is stable — but every re-render would allocate
  // a fresh transformer and subscribe a new listener to `x` /
  // `y`, leaving the previous one orphaned until GC. With one
  // `FallingIcon` per connector × however many scroll events
  // fire per second, that's a noisy allocation pattern for no
  // reason. Hoisting keeps the two transformers stable across
  // renders and makes the `style` object below pure `MotionValue`
  // references.
  const xVw = useTransform(x, (v) => `${v}vw`);
  const yVh = useTransform(y, (v) => `${v}vh`);

  return (
    <motion.div
      aria-hidden="true"
      className="absolute inset-0 flex items-center justify-center will-change-transform"
      style={{ x: xVw }}
    >
      <motion.svg
        viewBox="0 0 24 24"
        width={iconSize}
        height={iconSize}
        fill={connector.accent}
        style={{
          y: yVh,
          rotate,
          scale,
          opacity,
          filter,
        }}
      >
        <path d={connector.path} />
      </motion.svg>
    </motion.div>
  );
}

// ---------------------------------------------------------------------------
// Act 2: the singularity itself — event horizon + accretion disk.
// ---------------------------------------------------------------------------

interface BlackHoleProps {
  readonly scrollYProgress: MotionValue<number>;
}

function BlackHole({ scrollYProgress }: BlackHoleProps): JSX.Element {
  // Overall presence: the singularity swells from barely-there at
  // the end of act 1, peaks through act 2, then collapses to a
  // point as the report reveals in act 3. Peak scale bumped from
  // 1.1 to 1.22 during DAY-166 polish; the sphere now dominates
  // the viewport when it's on-screen, which is what the
  // "transformation from complexity to clarity" narrative needs.
  const horizonScale = useTransform(
    scrollYProgress,
    [0.15, 0.42, 0.58, 0.72, 0.82],
    [0.12, 1, 1.15, 1.22, 0],
  );
  const horizonOpacity = useTransform(
    scrollYProgress,
    [0.15, 0.33, 0.72, 0.8],
    [0, 1, 1, 0],
  );

  // Accretion-disk alpha. The disk is the brightest layer in
  // act 2, fading out to let the reveal burst take over.
  const diskOpacity = useTransform(
    scrollYProgress,
    [0.15, 0.4, 0.65, 0.78],
    [0, 1, 0.9, 0],
  );

  // Outer halo has its own curve — comes in slightly earlier (to
  // pre-announce the singularity before icons start arriving) and
  // lingers slightly longer (cushions the cut into the reveal).
  const haloOpacity = useTransform(
    scrollYProgress,
    [0.1, 0.38, 0.7, 0.82],
    [0, 1, 1, 0],
  );

  // Lensed-beam and Einstein-ring alphas — brightest right at
  // peak, because that is where the eye naturally lingers and is
  // where the "Interstellar-looking" payoff needs to land.
  const ringOpacity = useTransform(
    scrollYProgress,
    [0.2, 0.45, 0.7, 0.78],
    [0, 1, 0.95, 0],
  );

  return (
    <motion.div
      aria-hidden="true"
      className="absolute inset-0 flex items-center justify-center"
      style={{ opacity: horizonOpacity }}
    >
      <motion.div
        className="relative"
        style={{ scale: horizonScale, width: 300, height: 300 }}
      >
        {/*
         * Layer 1 — **Outer halo**. Diffuse, warm amber radial
         * gradient bleeding ~180px outside the rest of the sphere.
         * Earlier drafts used a multi-hue halo (amber → coral →
         * indigo) for a "logo echo", but that read as a pastel
         * disco rather than a singularity. Real (and film-real)
         * black holes only have one colour bleeding off the disk:
         * the incandescent orange of superheated matter. The halo
         * is now amber-dominant with a deep umber outer fade; the
         * Convergence-palette rhyme lives elsewhere on the page
         * (sparkline, trust strip, copy accents) and does not
         * need to fight realism here.
         */}
        <motion.div
          className="absolute inset-[-180px] rounded-full"
          style={{
            opacity: haloOpacity,
            background:
              "radial-gradient(circle," +
              " rgba(255,180,80,0.55) 0%," +
              " rgba(240,130,50,0.32) 20%," +
              " rgba(200,70,20,0.20) 40%," +
              " rgba(80,20,10,0.12) 62%," +
              " transparent 82%)",
            filter: "blur(28px)",
          }}
        />

        {/*
         * Layer 2 — **Accretion disk (spinning, Doppler-boosted)**.
         *
         * This is the realism pass. The disk is amber-dominant
         * with a very bright white-hot crescent on one side that
         * fades to a deep red-black on the other — the
         * Interstellar / Event-Horizon-Telescope signature "one
         * side brighter than the other" look. In physics that's
         * relativistic Doppler boosting: matter rotating toward
         * the viewer is blueshifted and brightened, matter
         * rotating away is redshifted and dimmed. In CSS we fake
         * it with an asymmetric conic gradient: the 0° stop is
         * the white-hot peak and the 180° stop is nearly black.
         * (In CSS, `conic-gradient(from 0deg, …)` anchors 0° at
         * the top — 12 o'clock — before any rotation; the exact
         * on-screen position of the bright crescent is whatever
         * the CSS keyframe has rotated the disk to at that frame.)
         *
         * The CSS keyframe still rotates the whole disk so the
         * bright crescent sweeps around the horizon at a steady
         * rate. That is technically inaccurate — real Doppler
         * brightness is anchored to the viewer's frame, not to
         * the disk — but cinematically correct: a static bright
         * crescent would read as a gradient background, and the
         * motion is what sells "this is a live thing pulling
         * matter in". The tradeoff is a conscious one; realism
         * is a means, and the end is "users recognise it as a
         * black hole at a glance".
         */}
        <motion.div
          className="absolute inset-[-120px] animate-accretion-spin rounded-full"
          style={{
            opacity: diskOpacity,
            background:
              "conic-gradient(from 0deg," +
              " rgba(255,245,200,1.0)   0deg," +
              " rgba(255,210,120,0.98) 40deg," +
              " rgba(240,160,60,0.90)  90deg," +
              " rgba(180,80,30,0.70) 135deg," +
              " rgba(90,30,10,0.45)  180deg," +
              " rgba(60,15,5,0.35)   200deg," +
              " rgba(120,45,15,0.55) 235deg," +
              " rgba(200,110,40,0.80) 275deg," +
              " rgba(250,190,90,0.95) 320deg," +
              " rgba(255,245,200,1.0) 360deg)",
            filter: "blur(14px)",
            mixBlendMode: "screen",
          }}
        />

        {/*
         * Layer 3 — **Einstein ring (elliptical photon ring)**.
         * Narrow vertically-stretched bright ring hugging the
         * horizon. The vertical stretch (ellipse 52% × 62%)
         * fakes the Interstellar visual — light from the back of
         * the disk bulging above and below the black sphere
         * because its photons were bent around the gravity well.
         * Tint shifted from near-cream to warmer gold so the ring
         * reads as a continuous piece of the same amber disk
         * rather than a separate white halo sitting on top.
         */}
        <motion.div
          className="absolute inset-[-30px] rounded-full"
          style={{
            opacity: ringOpacity,
            background:
              "radial-gradient(ellipse 52% 62% at center," +
              " rgba(255,230,160,0) 84%," +
              " rgba(255,230,160,0.65) 90%," +
              " rgba(255,245,200,1) 94%," +
              " rgba(240,160,60,0.9) 97%," +
              " transparent 100%)",
            filter: "blur(2.5px)",
          }}
        />

        {/*
         * Layer 4 — **Lensed equatorial beam**. Thin horizontal
         * bar of bright light crossing the sphere at the equator
         * with white-hot highlights on either side. Represents
         * the "near edge of the disk, seen edge-on from the
         * viewer" — the disk's closest face, brightest because
         * there is no gravitational redshift between it and you.
         * Extends 90px past the sphere on both sides to imply
         * the disk-plane continuing out into space. Warmer
         * orange tint to match the disk palette; stronger
         * boxShadow cascade so the beam sits in its own pool of
         * amber light rather than reading as a cream pencil line.
         */}
        <motion.div
          className="absolute left-[-90px] right-[-90px] top-1/2 h-[6px] -translate-y-1/2 rounded-full"
          style={{
            opacity: ringOpacity,
            background:
              "linear-gradient(90deg," +
              " transparent 0%," +
              " rgba(240,160,60,0) 8%," +
              " rgba(255,220,140,0.75) 28%," +
              " rgba(255,245,200,1) 45%," +
              " rgba(255,250,220,1) 50%," +
              " rgba(255,245,200,1) 55%," +
              " rgba(255,220,140,0.75) 72%," +
              " rgba(240,160,60,0) 92%," +
              " transparent 100%)",
            filter: "blur(1.5px)",
            boxShadow:
              "0 0 26px rgba(255,200,110,0.95)," +
              " 0 0 52px rgba(240,160,60,0.6)," +
              " 0 0 90px rgba(255,245,200,0.35)",
          }}
        />

        {/*
         * Layer 5 — **Event horizon**. Pure black core with a
         * sharper radial falloff than the previous draft (the
         * edge is now visible at 78% rather than 52%, which
         * makes the transition to the photon ring read cleaner)
         * and a deeper inset shadow so the void actually looks
         * like a void. Outer shadow also pushed so the sphere
         * appears to pull in the surrounding light.
         */}
        <div
          className="absolute inset-0 rounded-full"
          style={{
            background:
              "radial-gradient(circle," +
              " #000 70%," +
              " #030306 82%," +
              " #0A0A0E 92%," +
              " #17171A 100%)",
            boxShadow:
              "inset 0 0 70px rgba(0,0,0,0.95)," +
              " 0 0 120px rgba(0,0,0,0.9)," +
              " 0 0 60px rgba(0,0,0,1)",
          }}
        />
      </motion.div>
    </motion.div>
  );
}

// ---------------------------------------------------------------------------
// Act 3: the post-singularity reveal — Dayseam report card.
// ---------------------------------------------------------------------------

interface RevealProps {
  readonly scrollYProgress: MotionValue<number>;
}

function Reveal({ scrollYProgress }: RevealProps): JSX.Element {
  // The light burst is the singularity pulse that births the
  // window out of the black hole. It peaks at the *birth moment*
  // (scroll ~0.66 — halfway through the container's scale-up from
  // a point to full size) rather than after the window is already
  // full-size, which is how earlier drafts read: the burst used
  // to peak at 0.76 with the window sitting static at full scale
  // behind it, so the flash looked like a post-hoc flourish
  // rather than a cause-and-effect emergence. Pulled to an
  // earlier band so the sequence now reads as a single gesture:
  //
  //   black hole at peak → flash → window *emerges from the
  //   flash* → flash fades → internals fill in.
  //
  // Tint stays amber to match the accretion disk palette — the
  // colour story is continuous amber disk → amber burst →
  // neutral app chrome.
  const burstScale = useTransform(scrollYProgress, [0.58, 0.68, 0.8], [0.2, 2.6, 2.2]);
  const burstOpacity = useTransform(scrollYProgress, [0.6, 0.68, 0.76], [0, 0.95, 0]);

  // The outer container is now the primary driver of the "app
  // comes out of the black hole" beat: the ReportMock starts at
  // scale 0.04 (a near-invisible point anchored at the same
  // centre the singularity sat on) and expands to full size over
  // scroll [0.62 → 0.74]. Earlier drafts only ran [0.96 → 1] over
  // [0.6, 0.66], which is a settle motion — you barely noticed
  // the container moving. Users specifically asked for the
  // expansion to *happen, and finish*, before internals appear
  // inside. To honour that, the window's inner elements (title
  // bar, sidebar, action row, sections, donut) don't start
  // revealing until scroll 0.74+ — see the retimed bands in
  // `ReportMock.tsx`. Frame opacity (in ReportMock) also fades
  // 0 → 1 over [0.62, 0.68], so during the first half of the
  // expansion the frame is translucent and appears to materialise
  // out of the burst rather than popping in at full alpha.
  //
  // The Y translation is gone — when scaling from 0.04, any
  // vertical offset would multiply into a visually-distracting
  // jump of the tiny emerging dot. The scale alone carries all
  // the motion.
  const containerScale = useTransform(
    scrollYProgress,
    [0.62, 0.66, 0.7, 0.74],
    [0.04, 0.32, 0.78, 1],
  );

  return (
    // The outer wrapper is intentionally NOT aria-hidden — this
    // subtree contains the only place an assistive-tech user
    // should hear the final value proposition (the ReportMock's
    // `aria-label="Example Dayseam desktop window"` plus its
    // bullet copy). The decorative light burst below is the only
    // piece that should be muted, so the aria-hidden attribute
    // lives on that motion.div specifically. (ARIA has no way to
    // "un-hide" a descendant of an aria-hidden ancestor, so the
    // hiding has to be targeted at the decorative element rather
    // than blanket-applied here.)
    <div className="absolute inset-0 flex items-center justify-center">
      <motion.div
        aria-hidden="true"
        className="absolute h-[520px] w-[520px] rounded-full"
        style={{
          scale: burstScale,
          opacity: burstOpacity,
          background:
            "radial-gradient(circle," +
            " rgba(255,245,200,0.95) 0%," +
            " rgba(255,200,100,0.55) 30%," +
            " rgba(240,130,50,0.2) 55%," +
            " transparent 75%)",
          filter: "blur(14px)",
        }}
      />
      <motion.div
        className="relative flex w-full items-center justify-center px-6"
        style={{ scale: containerScale }}
      >
        <ReportMock scrollYProgress={scrollYProgress} />
      </motion.div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Landing intro — the card a visitor sees before any scroll happens.
// ---------------------------------------------------------------------------

interface IntroHeroProps {
  readonly scrollYProgress: MotionValue<number>;
}

/**
 * The first thing a visitor sees at scroll = 0. Without it the
 * pinned viewport starts visually empty — the black hole hasn't
 * formed, no icons have fallen yet, and the captions' act-1 line
 * hasn't faded in — which reads as "is the page broken?".
 *
 * Shows the product name + a one-line pitch, then fades out as
 * act 1 begins so it never competes with the falling-icon
 * choreography. The fade finishes by scroll ≈ 0.14 — that's roughly
 * the point where the first icon is fully on-screen (entryEnd for
 * index 0 is 0.12) and the user has committed to scrolling through
 * the three acts.
 *
 * `pointer-events-none` so it never intercepts the scroll-affordance
 * tap target below it. The `<h1>` here is the page's primary h1 in
 * the animated variant; HeroStatic has its own h1 for the reduced-
 * motion path, so exactly one h1 lands in the DOM regardless.
 */
function IntroHero({ scrollYProgress }: IntroHeroProps): JSX.Element {
  const introOpacity = useTransform(
    scrollYProgress,
    [0, 0.06, 0.14],
    [1, 0.55, 0],
  );
  const introY = useTransform(scrollYProgress, [0, 0.14], [0, -36]);

  return (
    <motion.div
      className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center px-6 text-center"
      style={{ opacity: introOpacity, y: introY }}
    >
      <h1 className="text-balance text-5xl font-semibold tracking-tight text-zinc-50 sm:text-7xl">
        Dayseam
      </h1>
      <p className="mt-4 max-w-xl text-balance text-base text-zinc-300 sm:mt-5 sm:text-lg">
        Automatic daily reports, pulled from every tool you already ship in
      </p>
      <p className="mt-6 text-xs uppercase tracking-[0.28em] text-zinc-500 sm:mt-8">
        Scroll to see how
      </p>
    </motion.div>
  );
}

// ---------------------------------------------------------------------------
// Narrative copy that scrolls with the acts.
// ---------------------------------------------------------------------------

interface CaptionsProps {
  readonly scrollYProgress: MotionValue<number>;
}

function Captions({ scrollYProgress }: CaptionsProps): JSX.Element {
  const act1Opacity = useTransform(scrollYProgress, [0, 0.1, 0.28, 0.38], [0, 1, 1, 0]);
  const act2Opacity = useTransform(scrollYProgress, [0.38, 0.48, 0.6, 0.7], [0, 1, 1, 0]);
  // Act 3 starts fading in slightly later than before (0.78 vs
  // 0.72) so it arrives once the report's frame + title bar have
  // materialised — the copy reads as a caption to what's on-screen
  // rather than an announcement before the thing it describes
  // exists.
  const act3Opacity = useTransform(scrollYProgress, [0.78, 0.9, 1], [0, 1, 1]);

  // Captions live at the very bottom of the pinned viewport
  // (bottom-6 / sm:bottom-8) instead of the previous bottom-24 /
  // sm:bottom-32. On viewports ~700-800px tall, the ReportMock
  // (~500px tall, vertically centred) ends around ~100-150px from
  // the viewport bottom; the earlier bottom-24 placement put the
  // act-3 caption directly on top of the report's donut-chart row,
  // which is exactly the moment the user is supposed to be
  // reading the report. Sitting the captions right at the edge
  // clears the report across every realistic viewport height.
  //
  // All three captions occupy the same line — only one is visible
  // at a time via the opacity transforms, and we want the crossfade
  // to happen *in place* rather than sliding down the viewport.
  // Caption 1 is in normal flow and defines the container's height;
  // captions 2 and 3 are absolutely positioned with `top-0` so they
  // render on top of caption 1. Without the explicit `top-0`, an
  // `absolute` element with no vertical offsets falls back to its
  // static-flow position, which for siblings means caption 2 sits
  // below caption 1 and caption 3 sits below caption 2 — the visual
  // "act-2 and act-3 copy sits lower than act-1" bug this fixes.
  //
  // Padding lives on the captions, not the container. An absolute
  // element's containing block is the *padding box* of its
  // positioned ancestor, so `inset-x-0` on captions 2 and 3 ignores
  // any `px-6` on the container. If we left `px-6` on the container,
  // caption 1 (in flow) would respect the padding — its text width
  // capped at `max-w-3xl - 48px` — while captions 2 and 3 (absolute,
  // inset-x-0) would stretch the full `max-w-3xl`, 48px wider. In
  // the 780-850px viewport band that 48px can flip caption 3 (54
  // chars) onto a different wrap break than caption 1 (38 chars),
  // which is exactly the kind of subtle "the crossfade feels
  // wobbly, but I can't say why" bug that survives multiple
  // review passes. Moving `px-6` onto each caption gives all three
  // an identical effective text width, independent of whether the
  // caption is in flow or absolutely positioned.
  //
  // `data-caption` lets the caption-alignment Playwright smoke
  // test (`apps/website/tests/caption-alignment.spec.mjs`) query
  // all three lines and assert their rendered `top` values
  // collapse to a single value — the regression guard for this
  // file's history of absolute-positioning pitfalls.
  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-6 mx-auto max-w-3xl text-center sm:bottom-8">
      <motion.p
        data-caption="act1"
        className="px-6 text-balance text-xl font-medium text-zinc-100 sm:text-2xl"
        style={{ opacity: act1Opacity }}
      >
        Your day is <span className="text-strand-coral">scattered</span> across ten tools
      </motion.p>
      <motion.p
        data-caption="act2"
        className="absolute inset-x-0 top-0 px-6 text-balance text-xl font-medium text-zinc-100 sm:text-2xl"
        style={{ opacity: act2Opacity }}
      >
        Dayseam pulls every fragment to <span className="text-strand-gold">one point</span>
      </motion.p>
      <motion.p
        data-caption="act3"
        className="absolute inset-x-0 top-0 px-6 text-balance text-xl font-medium text-zinc-100 sm:text-2xl"
        style={{ opacity: act3Opacity }}
      >
        And writes you the <span className="text-cream">report</span> of what you actually shipped
      </motion.p>
    </div>
  );
}

// ---------------------------------------------------------------------------
// The full animated hero — root exported component.
// ---------------------------------------------------------------------------

function HeroAnimated(): JSX.Element {
  const sectionRef = useRef<HTMLElement>(null);
  const { scrollYProgress } = useScroll({
    target: sectionRef,
    offset: ["start start", "end end"],
  });

  return (
    <section
      ref={sectionRef}
      className="relative h-[300vh]"
      aria-label="How Dayseam works, an animated walkthrough"
    >
      <div className="sticky top-0 flex h-screen items-center justify-center overflow-hidden">
        {/* Deep-space backdrop — subtle radial vignette so the hero
         * reads as a distinct "stage" on the page rather than
         * continuing the flat charcoal upward into the nav. */}
        <div
          aria-hidden="true"
          className="absolute inset-0"
          style={{
            background:
              "radial-gradient(circle at 50% 45%, #1f1f24 0%, #14141a 45%, #0a0a0e 100%)",
          }}
        />

        {/* Static star field — a handful of tiny pseudo-stars so
         * the sci-fi tone registers immediately, not just once the
         * user starts scrolling.
         *
         * Positions are deterministic by design: the file-wide
         * "no Math.random() at render" contract (see the
         * `buildTrajectory` note in the Hero docstring) means SSR
         * output and client-render output agree byte-for-byte,
         * so React hydration never warns and Astro's static HTML
         * never looks different from the hydrated island. The
         * `(i * 37) % 100` / `(i * 53 + 7) % 100` hashes are
         * "random-looking but deterministic" because 37 and 53
         * are coprime with 100, so walking `i` from 0..59 hits 60
         * distinct residues without clustering. Changing the
         * multipliers will shuffle the field; changing the star
         * count will require new coprime-with-count picks, or
         * the same residue will appear twice and two stars will
         * overlap. This is intentionally the last place in the
         * hero that uses an RNG-style pattern, so the contract
         * stays easy to audit. */}
        <div aria-hidden="true" className="pointer-events-none absolute inset-0">
          {Array.from({ length: 60 }).map((_, i) => {
            const left = (i * 37) % 100;
            const top = ((i * 53) + 7) % 100;
            const size = 1 + (i % 3) * 0.5;
            const opacity = 0.15 + ((i * 7) % 5) * 0.08;
            return (
              <span
                key={`star-${i}`}
                className="absolute rounded-full bg-white"
                style={{
                  left: `${left}%`,
                  top: `${top}%`,
                  width: `${size}px`,
                  height: `${size}px`,
                  opacity,
                }}
              />
            );
          })}
        </div>

        <BlackHole scrollYProgress={scrollYProgress} />

        {ALL_CONNECTORS.map((connector, index) => (
          <FallingIcon
            key={connector.id}
            connector={connector}
            index={index}
            total={ALL_CONNECTORS.length}
            scrollYProgress={scrollYProgress}
          />
        ))}

        <Reveal scrollYProgress={scrollYProgress} />

        {/* IntroHero sits after Reveal in DOM order so a mid-scroll
         * reload (scroll = 0, report also hidden) still paints the
         * landing card above any leftover stacking context. The
         * scroll prompt inside IntroHero replaces the previous
         * standalone "Scroll" affordance — one landing element,
         * not two, fading out together. */}
        <IntroHero scrollYProgress={scrollYProgress} />

        <Captions scrollYProgress={scrollYProgress} />
      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------
// Static fallback — vestibular-safe alternative.
// ---------------------------------------------------------------------------

function HeroStatic(): JSX.Element {
  // Reduced-motion fallback. Keeps the same three-beat pitch as the
  // animated hero (scattered → gathered → reported) but as a static
  // page so the layout has to do the work the animation does.
  //
  // Layout rule we enforce here: every step card holds *only* text +
  // a small decorative chip row. The full ReportMock (which is tall,
  // since it mirrors the desktop window) renders as a separate hero
  // visual *below* the steps, not inside the step-3 card. Embedding it
  // inside one card forces grid `items-stretch` to stretch cards 1
  // and 2 to ~1100px, dwarfing their two short paragraphs and breaking
  // every spacing relationship in the section. Pulling the visual out
  // means the three step cards stay equal-height and the report still
  // gets a marquee placement at full width.
  return (
    <section
      className="mx-auto max-w-6xl px-6 py-20"
      aria-label="How Dayseam works"
    >
      <div className="mx-auto max-w-3xl text-center">
        <h1 className="text-balance text-4xl font-semibold leading-tight text-zinc-50 sm:text-5xl">
          The report of your day,
          <br />
          written from evidence
        </h1>
        <p className="mt-4 text-balance text-lg text-zinc-400">
          Dayseam collects what you actually did across GitHub, GitLab, Jira,
          Confluence, and your local repos, then stitches it into an editable
          Markdown report, locally, with no account and no telemetry
        </p>
      </div>

      <ol className="mx-auto mt-14 grid max-w-5xl gap-6 md:grid-cols-3">
        <li className="rounded-2xl border border-white/5 bg-charcoal-700/40 p-6">
          <p className="mb-3 text-xs uppercase tracking-[0.18em] text-zinc-500">
            Step 1
          </p>
          <h2 className="mb-2 text-lg font-semibold text-zinc-100">
            Your day is scattered
          </h2>
          <p className="text-sm leading-relaxed text-zinc-400">
            Commits here, tickets there, docs somewhere else. Nobody has time to
            grep five tools every evening to write a standup
          </p>
          <div className="mt-5 flex flex-wrap gap-2">
            {ALL_CONNECTORS.map((c) => (
              <span
                key={`static-chip-${c.id}`}
                className="inline-flex h-7 w-7 items-center justify-center rounded-md"
                style={{
                  backgroundColor: `${c.accent}22`,
                  color: c.accent,
                }}
                aria-label={c.name}
              >
                <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
                  <path d={c.path} />
                </svg>
              </span>
            ))}
          </div>
        </li>

        <li className="rounded-2xl border border-white/5 bg-charcoal-700/40 p-6">
          <p className="mb-3 text-xs uppercase tracking-[0.18em] text-zinc-500">
            Step 2
          </p>
          <h2 className="mb-2 text-lg font-semibold text-zinc-100">
            Dayseam gathers everything
          </h2>
          <p className="text-sm leading-relaxed text-zinc-400">
            Connectors pull today's evidence from each source (PRs, merges,
            moved tickets, edited pages) into one local SQLite index
          </p>
        </li>

        <li className="rounded-2xl border border-white/5 bg-charcoal-700/40 p-6">
          <p className="mb-3 text-xs uppercase tracking-[0.18em] text-zinc-500">
            Step 3
          </p>
          <h2 className="mb-2 text-lg font-semibold text-zinc-100">
            One evidence linked report
          </h2>
          <p className="text-sm leading-relaxed text-zinc-400">
            An editable Markdown document, saved to your Obsidian vault or any
            folder, with every bullet linked back to the source it came from
          </p>
        </li>
      </ol>

      <div className="mx-auto mt-16 max-w-3xl">
        <ReportMock className="w-full" />
      </div>
    </section>
  );
}

/**
 * The exported hero. Branches on `prefers-reduced-motion` — which
 * is a hook, so the client island re-evaluates if the user flips
 * the OS-level setting mid-session.
 *
 * During SSR `useReducedMotion()` returns `null`, which we treat
 * as "assume motion is fine" and render the animated variant; the
 * client then re-hydrates with the real preference. This is the
 * standard Framer Motion pattern, and the animated variant
 * degrades gracefully on the motion-sensitive client because
 * every animated property returns to a safe steady-state at the
 * top/bottom of the pinned scroll range.
 */
export default function Hero(): JSX.Element {
  const prefersReducedMotion = useReducedMotion();
  if (prefersReducedMotion) {
    return <HeroStatic />;
  }
  return <HeroAnimated />;
}
