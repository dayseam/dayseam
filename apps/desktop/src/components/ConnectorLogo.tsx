import type { JSX, SVGProps } from "react";
import type { SourceKind } from "@dayseam/ipc-types";
import { MARKS } from "./connectorMarks";

/**
 * DAY-159. Small inline-SVG brand mark for each connector kind,
 * keyed on the canonical {@link SourceKind} enum so there is a
 * single place to extend when a new connector lands.
 *
 * Design rationale:
 * - `fill="currentColor"` so the mark inherits the chip's text
 *   colour and dark-mode classes automatically, the same way every
 *   other glyph in the app already themes.
 * - Path data is inlined (no HTTP, no asset pipeline, no SVGR, no
 *   runtime dep on `simple-icons`) because a small set of `d`
 *   strings is easier to audit than a toolchain, and the marks
 *   themselves change on the order of once-per-decade.
 * - Paths are sourced from Simple Icons (CC0), which
 *   tracks each service's official brand mark. See `CREDITS.md`
 *   additions in this PR for attribution; the marks themselves
 *   remain the trademarks of their respective owners and are used
 *   here in the classic "connected to X" nominative-fair-use sense.
 *
 * Accessibility:
 * - Default is **decorative** (`aria-hidden`, no `<title>`): most
 *   callers already render a visible text label next to the mark,
 *   so exposing the brand name to screen readers is redundant alt
 *   text.
 * - Callers that render the mark *without* a visible label should
 *   set `labelled={true}`; that turns on `role="img"` and a
 *   `<title>` so assistive tech announces the service.
 */

export interface ConnectorLogoProps
  extends Omit<SVGProps<SVGSVGElement>, "children" | "viewBox" | "fill"> {
  /** Canonical connector kind — keys into {@link MARKS}. */
  kind: SourceKind;
  /** Pixel size for both width and height. Defaults to 14, matching
   *  the text-xs metrics of the sources sidebar chip. */
  size?: number;
  /** When `true`, the logo is announced to assistive tech via
   *  `role="img"` plus a `<title>` holding the brand name. Leave
   *  `false` (the default) when a visible text label already names
   *  the service — otherwise the mark is redundant alt text. */
  labelled?: boolean;
  /**
   * When `true`, render the mark in its brand accent instead of the
   * chip's `currentColor`. DAY-170 wired this on so the sources
   * sidebar, the Add-source dropdown, and the identity manager can
   * all surface the one visual signal users already know — GitHub
   * is near-black in light mode and white in dark mode, GitLab is
   * tangerine, and so on — without any other glyph on the page
   * picking up bespoke colour. The flag exists as an opt-in rather
   * than a global switch because the same component still has to
   * serve monochrome callers (e.g. the log-drawer run rows) where a
   * colour flash would look noisy.
   *
   * The colour is switched at runtime via CSS's native
   * `light-dark()` function, relying on `color-scheme: light|dark`
   * being set on `<html>` by `applyResolvedTheme`. That means the
   * mark flips the instant the app's theme changes without the
   * component needing a React subscription to `useTheme()`, and it
   * inherits the user's system theme correctly when the preference
   * is `system`. Tauri 2 ships a recent-enough WebView on every
   * supported platform that `light-dark()` is safe to rely on; if
   * the function is ever unsupported, the browser falls through to
   * the inherited `currentColor` and the mark still renders — just
   * in a neutral tone rather than in brand accent.
   */
  colored?: boolean;
}

/** Inline-SVG brand mark for the given {@link SourceKind}. See the
 *  file-level doc for rationale and accessibility guidance. */
export function ConnectorLogo({
  kind,
  size = 14,
  labelled = false,
  colored = false,
  className,
  style,
  ...rest
}: ConnectorLogoProps): JSX.Element {
  const mark = MARKS[kind];
  const coloredStyle = colored
    ? // Using `color` (not `fill`) so the `fill="currentColor"`
      // below picks up the tint automatically; this keeps the rest
      // of the component (accessibility, sizing) identical whether
      // or not the caller wants the brand accent.
      {
        color: `light-dark(${mark.accent.light}, ${mark.accent.dark})`,
      }
    : undefined;
  return (
    <svg
      role={labelled ? "img" : undefined}
      aria-hidden={labelled ? undefined : true}
      aria-label={labelled ? mark.brandName : undefined}
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="currentColor"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      style={{ ...coloredStyle, ...style }}
      data-testid={`connector-logo-${kind}`}
      data-colored={colored ? "true" : undefined}
      // DAY-170: expose the resolved accent pair as data attributes
      // when `colored` is on. This exists specifically for two
      // consumers that cannot observe the inline `color:
      // light-dark(...)` cleanly:
      //   1. JSDOM-backed vitest runs, where `style.color` drops
      //      values it doesn't parse (including `light-dark()`), so
      //      tests would otherwise have to reach into CSSOM
      //      internals or diff snapshots just to assert the right
      //      brand hex landed.
      //   2. Playwright / a11y smoke checks that want to target
      //      "the GitHub-coloured chip" without parsing computed
      //      styles back into hexes.
      // The attributes are intentionally absent when `colored` is
      // false so the monochrome render stays clean.
      data-accent-light={colored ? mark.accent.light : undefined}
      data-accent-dark={colored ? mark.accent.dark : undefined}
      {...rest}
    >
      {labelled ? <title>{mark.brandName}</title> : null}
      <path d={mark.path} />
    </svg>
  );
}
