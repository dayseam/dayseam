# @dayseam/website

The public marketing site for Dayseam — **dayseam.com** (pre-launch).

Scaffolded in **DAY-166**, making progress on repo issue
[#141 — "Website: launch public Dayseam marketing site"](https://github.com/dayseam/dayseam/issues/141).

## Stack

- **[Astro 4](https://astro.build/)** for the static site shell — zero JS by
  default, one React island for the scroll-driven hero.
- **TypeScript** in strict mode (Astro's `strict` tsconfig preset).
- **Tailwind CSS 3** for styling, with a `tailwind.config.mjs` palette that
  mirrors the locked DAY-161 Convergence brand mark
  (`assets/brand/dayseam-mark.svg`).
- **Framer Motion 11** inside the single React island, for scroll-linked
  animation via `useScroll` / `useTransform`. Reduced-motion users get a
  static equivalent; the branch is in `src/components/Hero.tsx`.

No analytics library, no CMS, no backend. The site ships as pure HTML + CSS +
one JS bundle for the hero.

## Local development

From the repo root:

```bash
pnpm install
pnpm --filter @dayseam/website dev
# → http://localhost:4321
```

To produce a production build:

```bash
pnpm --filter @dayseam/website build
pnpm --filter @dayseam/website preview
```

The frontend CI job (`.github/workflows/ci.yml` → `frontend`) runs
`pnpm -r typecheck` across the workspace, which invokes `astro check` for
this package.

## Layout

```
src/
  components/
    Hero.tsx          # React island — scroll-driven black-hole animation
    HeroStatic ...    # (inside Hero.tsx) — reduced-motion fallback
    ReportMock.tsx    # Stylised Dayseam report card (post-singularity reveal)
    SiteNav.astro
    SiteFooter.astro
    BrandMark.astro   # Inline Convergence mark, inherits Tailwind sizing
    HowItWorks.astro
    ConnectorGrid.astro
    TrustStrip.astro
    DownloadCTA.astro
  data/
    connectors.ts     # Five SHIPPING + five COMING_SOON, with Simple-Icons paths
  layouts/
    Base.astro        # HTML shell, meta, favicon, global.css
  pages/
    index.astro       # Landing — composes all of the above
  styles/
    global.css        # Tailwind + charcoal defaults + prefers-reduced-motion
public/
  dayseam-mark.svg    # Copy of the canonical brand mark, used as favicon
```

## Adding a connector

When a new connector ships in the desktop app:

1. Open `src/data/connectors.ts`.
2. Move the connector from `COMING_SOON` to `SHIPPING`.
3. Set `accent` to the service's **real brand colour** — the page reads as
   "that's GitHub, that's GitLab, that's Linear" the moment a visitor looks
   at the hero icon-rain. Where the canonical hex is invisible on charcoal
   (GitHub's near-black, Confluence's very-dark navy), substitute a visible
   in-family alternative and leave a comment in `connectors.ts` explaining
   why. The Convergence logo-rhyme lives on in the accretion-disk conic
   gradient, the sparkline, and the trust-strip labels — none of which are
   tied to per-connector accents.
4. Update `apps/desktop/src/components/ConnectorLogo.tsx` in the same PR if
   the connector is new to the desktop app too — the brand-mark path data is
   intentionally duplicated between the two workspaces. (Hoisting into
   `@dayseam/ui` is a tracked follow-up; for now, CI has no gate for this
   drift, so the two-edit contract is a process note.)

## Brand tokens

All brand colours flow from the SVG source at
`../../assets/brand/dayseam-mark.svg`. When the locked palette changes
(which per the DAY-161 notes on that file should be approximately never),
update the SVG first, then mirror the new hexes in `tailwind.config.mjs`.

## Attribution

See [`CREDITS.md`](./CREDITS.md) for attribution of the Simple Icons brand
marks shown in the connector grid and hero animation.
