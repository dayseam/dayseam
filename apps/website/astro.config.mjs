// @ts-check
import { defineConfig } from "astro/config";
import react from "@astrojs/react";
import tailwind from "@astrojs/tailwind";

// DAY-166. Marketing site for Dayseam.
//
// Output: static. The site has no backend, no user accounts, no
// database — the whole point of Dayseam is that the product itself
// is local-first, so the marketing site that pitches it should not
// quietly require a server. Every page renders at build time; the
// only client-side JavaScript is the hero animation island, which
// is mounted with `client:only="react"` (not `client:visible`)
// because `useScroll` / `useMotionValue` read `window` synchronously
// at setup — SSR would blow up. `client:only` is the correct
// directive for browser-only islands, and it still defers loading
// until after HTML paint, so time-to-first-content is unaffected.
export default defineConfig({
  site: "https://dayseam.com",
  output: "static",
  compressHTML: true,
  // Astro's dev toolbar is a floating chrome strip at the bottom of
  // dev builds — useful when actively iterating, noisy when sharing
  // the site with stakeholders or running a smoke test. Disabling
  // it keeps the preview visually identical to production. The flag
  // only affects `astro dev`; `astro build` never ships the toolbar.
  devToolbar: {
    enabled: false,
  },
  integrations: [
    react(),
    tailwind({
      // Let us own `src/styles/global.css` so we can layer custom
      // brand tokens (the Convergence strand colours) on top of
      // Tailwind's base without the integration injecting a
      // competing `base.css`.
      applyBaseStyles: false,
    }),
  ],
  vite: {
    ssr: {
      // framer-motion ships a client-only `sync` export path that
      // trips Astro's SSR resolver on static-build; marking it
      // external makes Astro defer it to the client island rather
      // than trying to evaluate it during build.
      noExternal: [],
    },
  },
});
