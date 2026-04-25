/** @type {import('tailwindcss').Config} */
// DAY-166. Tailwind config for the marketing site.
//
// The brand palette below mirrors the locked DAY-161 Convergence
// identity at `assets/brand/dayseam-mark.svg`. Do NOT introduce new
// brand hexes here without updating the SVG source too — the SVG is
// the canonical source of truth and the site rasterises off it for
// every mark (favicon, og:image, nav logo). Keeping the hexes in one
// place means a future palette refresh touches exactly two files.
export default {
  content: ["./src/**/*.{astro,html,js,jsx,ts,tsx,md,mdx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        // Matches `Background charcoal` in dayseam-mark.svg. Used as
        // the page background so the Convergence mark sits flush
        // against its own canvas colour when placed in the nav.
        charcoal: {
          DEFAULT: "#17171A",
          900: "#0E0E10",
          800: "#17171A",
          700: "#1F1F24",
          600: "#2A2A30",
          500: "#3A3A42",
          400: "#4E4E57",
        },
        // The five strand hues from the Convergence mark. Each
        // connector's accent-of-origin comes from here, so the
        // landing reads as a visual echo of the logo.
        strand: {
          gold: "#E89A2C",
          teal: "#2B8AA0",
          coral: "#D94F6E",
          sage: "#5BA567",
          indigo: "#4D6DD0",
        },
        // The "seam" cream used for accent text + running-stitch
        // dividers. The single chromatic warm in an otherwise cool
        // UI — use sparingly, this is the "resolved report" colour.
        cream: "#F6F1E6",
      },
      fontFamily: {
        sans: [
          "InterVariable",
          "Inter",
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "Roboto",
          "Helvetica Neue",
          "system-ui",
          "sans-serif",
        ],
        mono: [
          "JetBrains Mono",
          "SF Mono",
          "Menlo",
          "Consolas",
          "Liberation Mono",
          "monospace",
        ],
      },
      boxShadow: {
        // Long, soft, colour-aware glow — sits behind the report
        // card at the post-singularity reveal. Specifically cream-
        // tinted so it reads as "the seam is warm" rather than a
        // generic drop-shadow.
        seam: "0 30px 120px -20px rgba(246, 241, 230, 0.08)",
      },
      keyframes: {
        // Subtle idle motion on the accretion disk while act 2 is
        // on-screen. Driven by a CSS keyframe (not Framer Motion)
        // because the spin is a purely cosmetic loop and should
        // keep turning even when scroll progress is momentarily 0.
        "accretion-spin": {
          "0%": { transform: "rotate(0deg)" },
          "100%": { transform: "rotate(360deg)" },
        },
      },
      animation: {
        "accretion-spin": "accretion-spin 24s linear infinite",
      },
    },
  },
  plugins: [],
};
