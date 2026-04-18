// Helpers for dismissing the inline splash screen defined in
// `apps/desktop/index.html`.
//
// The splash is plain HTML/CSS so it paints the instant the webview
// has the document — before Vite's JS bundle has even finished
// parsing. Once React's `App` mounts we flip the node's
// `data-hidden` attribute, which kicks off the CSS fade, and then
// remove the node from the tree once the transition completes so
// it can't capture focus or screen-reader attention.
//
// Everything is defensive by design: if the splash node isn't
// present (e.g. a Vitest jsdom run, or a future index.html refactor)
// the helpers are no-ops. React must never block on the splash.

const SPLASH_ID = "splash";

// Fallback only. The actual fade duration is read from the computed
// `transition-duration` of the splash element at dismissal time, so
// a stylesheet bump in `index.html` is picked up automatically.
// Kept in sync with the CSS default so jsdom (which returns `""`
// from `getComputedStyle` for unresolved transitions) still removes
// the node in a reasonable window.
const SPLASH_FADE_MS_FALLBACK = 220;

// Paranoia buffer beyond the computed transition, to absorb any
// paint-scheduling jitter between the CSS fade completing and the
// JS timer firing. Cheap at 20ms — the node is invisible by then.
const SPLASH_REMOVE_SLACK_MS = 20;

/**
 * Parse a CSS `transition-duration` value (e.g. `"220ms"`, `"0.22s"`,
 * `"220ms, 0s"`) into the longest component in milliseconds. Returns
 * the fallback when the value is empty or unparseable, which happens
 * in jsdom or when the element has no transition.
 */
function parseTransitionDuration(value: string): number {
  if (!value) return SPLASH_FADE_MS_FALLBACK;
  const parts = value.split(",");
  let longest = 0;
  for (const raw of parts) {
    const token = raw.trim();
    if (!token) continue;
    let ms = 0;
    if (token.endsWith("ms")) {
      ms = Number.parseFloat(token.slice(0, -2));
    } else if (token.endsWith("s")) {
      ms = Number.parseFloat(token.slice(0, -1)) * 1000;
    } else {
      ms = Number.parseFloat(token);
    }
    if (Number.isFinite(ms) && ms > longest) longest = ms;
  }
  return longest > 0 ? longest : SPLASH_FADE_MS_FALLBACK;
}

/**
 * Begin the splash fade and remove the node from the DOM once the
 * CSS transition has finished. Safe to call multiple times — the
 * second call (whether concurrent mid-fade or after removal) is a
 * no-op because the node's already being torn down.
 *
 * Separated from the React tree on purpose: it runs in a
 * `useEffect` after first paint, which guarantees the user sees
 * the rendered `App` at least one frame before the splash starts
 * fading. Without that ordering you can get a flicker where the
 * splash disappears before the app has laid out.
 */
export function dismissSplash(): void {
  if (typeof document === "undefined") return;
  const splash = document.getElementById(SPLASH_ID);
  if (!splash) return;

  // Re-entrancy guard: `App`'s effect runs twice under StrictMode,
  // and we also want mid-fade re-entry to be a cheap no-op so future
  // callers don't have to think about ordering. `splash.test.tsx`
  // empirically pins this guard — commenting it out makes the
  // StrictMode and mid-fade cases fail on the `data-hidden` write
  // count, not just on end-state.
  if (splash.getAttribute("data-hidden") === "true") return;

  splash.setAttribute("data-hidden", "true");

  // `matchMedia` can be absent in some test environments; guard the
  // reduced-motion branch so the helper still works there.
  const prefersReducedMotion =
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  const remove = () => {
    splash.parentNode?.removeChild(splash);
  };

  if (prefersReducedMotion) {
    remove();
    return;
  }

  // Drive the JS timeout from the CSS so a future stylesheet bump
  // (or a Tailwind refactor that swaps the transition entirely) is
  // picked up without a paired JS change.
  const computedDuration =
    typeof window !== "undefined" && typeof window.getComputedStyle === "function"
      ? parseTransitionDuration(window.getComputedStyle(splash).transitionDuration)
      : SPLASH_FADE_MS_FALLBACK;

  // `setTimeout` rather than `transitionend` because `transitionend`
  // doesn't fire on `visibility` changes in every webview and we
  // want a hard upper bound on how long the (now-invisible) node
  // lingers in the DOM.
  window.setTimeout(remove, computedDuration + SPLASH_REMOVE_SLACK_MS);
}
