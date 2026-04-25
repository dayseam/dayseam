/**
 * DAY-166. Regression guard for the hero-captions alignment
 * invariant.
 *
 * ## What this test defends
 *
 * The hero has three narrative captions — "Your day is scattered…",
 * "Dayseam pulls every fragment…", "And writes you the report…" —
 * that crossfade in place over the same viewport location as the
 * user scrolls. The only way that crossfade reads as a crossfade
 * (rather than three lines of text stacking or sliding past each
 * other) is if all three captions occupy exactly the same rendered
 * position.
 *
 * The bug this test exists to catch was a subtle one: captions 2
 * and 3 were marked `position: absolute` with `inset-x-0` but no
 * `top` or `bottom` offset. CSS treats absolute elements with no
 * vertical offsets as "render at your static-flow position, just
 * pulled out of the flow for stacking purposes". Which means
 * caption 2 rendered where it *would* have appeared if it were in
 * flow — i.e. below caption 1 — and caption 3 below caption 2.
 * On a 1440×900 viewport caption 1 was at y≈836 and caption 3
 * was at y≈900, a full caption-height below. Visible immediately
 * in the act-3 screenshot; invisible to anyone reading the CSS
 * unless they already knew the trap.
 *
 * The fix is `top-0` on captions 2 and 3 (and `px-6` moved onto
 * each caption instead of the container, so absolute captions
 * don't stretch 48px wider than the in-flow one). The fix is one
 * Tailwind class, which makes it trivial to lose during a
 * refactor — exactly the shape of regression a smoke test should
 * defend.
 *
 * ## What we measure
 *
 * Each caption carries a `data-caption="act1|act2|act3"` attribute.
 * We query all three, read `getBoundingClientRect().top`, and
 * assert they collapse to a single value at every viewport
 * width we care about.
 *
 * We accept a ±1px tolerance because browsers can subpixel-render
 * a text baseline differently between elements when the computed
 * font metrics round off — Chromium has historically reported
 * 836.0 vs 836.5 on the same nominal line. Anything beyond 2px
 * indicates a real alignment regression, not rounding noise.
 *
 * ## How to run
 *
 *   # from the monorepo root
 *   pnpm --filter @dayseam/website build
 *   pnpm --filter @dayseam/website preview &
 *   node apps/website/tests/caption-alignment.spec.mjs
 *
 * Exit code 0 means every caption is aligned on every viewport.
 * Any non-zero exit means a caption drifted; the failing message
 * prints the viewport, the three `top` values, and their spread.
 *
 * This test is intentionally standalone Node — not wired into
 * `@playwright/test` fixtures — because it checks one invariant
 * and does not justify the machinery of a full Playwright test
 * harness. If we grow a second browser-level test we should
 * upgrade to a proper test runner; until then this stays
 * minimal and unambiguous.
 */

import { chromium } from "playwright";

const PREVIEW_URL = process.env.WEBSITE_URL ?? "http://localhost:4321";

// Viewports chosen to cover the three bands the hero is tuned for:
// a desktop width, the phone width where `text-balance` line-wraps
// differ, and the 780-850 band where the old `inset-x-0` bug was
// most visible because captions 2/3 stretched 48px wider than
// caption 1.
const VIEWPORTS = [
  { width: 1440, height: 900, label: "desktop" },
  { width: 820, height: 900, label: "tablet-wrap-edge" },
  { width: 375, height: 812, label: "mobile" },
];

const TOLERANCE_PX = 2;

async function measureCaptions(page, viewport) {
  await page.setViewportSize(viewport);
  await page.goto(PREVIEW_URL, { waitUntil: "networkidle" });

  // The hero is a `client:only="react"` island. `networkidle` is
  // satisfied once the JS bundle has finished loading but before
  // React has hydrated the component, so the `[data-caption]`
  // elements won't exist yet. Wait for them explicitly.
  await page.waitForSelector("[data-caption='act1']", { timeout: 10_000 });

  return page.evaluate(() => {
    const captions = Array.from(document.querySelectorAll("[data-caption]"));
    return captions.map((el) => {
      const rect = el.getBoundingClientRect();
      return {
        caption: el.getAttribute("data-caption"),
        top: Math.round(rect.top * 100) / 100,
        left: Math.round(rect.left * 100) / 100,
        width: Math.round(rect.width * 100) / 100,
      };
    });
  });
}

function assertAligned(measurements, viewport) {
  if (measurements.length !== 3) {
    throw new Error(
      `[${viewport.label} ${viewport.width}×${viewport.height}] expected 3 captions, got ${measurements.length}: ${JSON.stringify(measurements)}`,
    );
  }

  const tops = measurements.map((m) => m.top);
  const spread = Math.max(...tops) - Math.min(...tops);

  if (spread > TOLERANCE_PX) {
    const detail = measurements
      .map((m) => `  ${m.caption}: top=${m.top}`)
      .join("\n");
    throw new Error(
      `[${viewport.label} ${viewport.width}×${viewport.height}] caption tops drifted by ${spread}px (tolerance ${TOLERANCE_PX}px):\n${detail}`,
    );
  }

  // Also assert left/width match — the `px-6` audit. If captions 2
  // and 3 were to regress to `inset-x-0` without the matching `px-6`
  // that caption 1 carries, their `left` would be 0 and caption 1's
  // would be 24 (the effective `px-6` on a flow element). That would
  // not trip the `top` check but would still break the crossfade feel.
  const lefts = measurements.map((m) => m.left);
  const leftSpread = Math.max(...lefts) - Math.min(...lefts);
  if (leftSpread > TOLERANCE_PX) {
    const detail = measurements
      .map((m) => `  ${m.caption}: left=${m.left}`)
      .join("\n");
    throw new Error(
      `[${viewport.label} ${viewport.width}×${viewport.height}] caption lefts drifted by ${leftSpread}px (tolerance ${TOLERANCE_PX}px):\n${detail}`,
    );
  }

  console.log(
    `[${viewport.label} ${viewport.width}×${viewport.height}] aligned at top=${tops[0]}, left=${lefts[0]} (spread ${spread}px / ${leftSpread}px)`,
  );
}

async function main() {
  const browser = await chromium.launch();
  const context = await browser.newContext({ reducedMotion: "no-preference" });
  const page = await context.newPage();

  let failures = 0;

  try {
    for (const viewport of VIEWPORTS) {
      try {
        const measurements = await measureCaptions(page, viewport);
        assertAligned(measurements, viewport);
      } catch (err) {
        console.error(err.message);
        failures += 1;
      }
    }
  } finally {
    await browser.close();
  }

  if (failures > 0) {
    console.error(`\n${failures} viewport(s) failed the caption-alignment invariant.`);
    process.exit(1);
  }

  console.log("\nCaption alignment invariant holds across all viewports.");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
