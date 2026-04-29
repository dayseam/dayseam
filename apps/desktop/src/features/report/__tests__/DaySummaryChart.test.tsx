// DAY-211. RTL coverage for the DaySummaryChart streaming-preview
// header card. Three concerns are tested independently so a
// regression points at the right layer:
//
//   1. Aggregation purity (`aggregateByKind`) — the only place the
//      "drop legacy null source_kind", "tie-break by enum order"
//      and "sort by count descending" rules live. Tested without
//      rendering anything.
//   2. Render contract — the chart's DOM exposes the contract the
//      streaming preview and Playwright checks rely on:
//        - one path per kind (or the empty-state ring)
//        - data-attributes carrying kind, count, and accent hexes
//        - aria-label summarising the day
//        - legend rows matching slice order
//   3. Click-to-anchor — clicking a slice resolves the right
//      `[data-kind]` element and calls scrollIntoView on it.
//      jsdom doesn't ship scrollIntoView, so the test installs a
//      mock and asserts it was called with the correct selector.
//
// Snapshot test at the bottom freezes the SVG markup for a
// canonical multi-kind day so a stylistic regression (slice order,
// accent hex, layout class drift) shows up in CI as a deliberate
// snapshot review rather than a slow-burn UI bug.

import {
  render,
  screen,
  fireEvent,
  cleanup,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RenderedSection } from "@dayseam/ipc-types";
import { DaySummaryChart, aggregateByKind } from "../DaySummaryChart";

afterEach(() => {
  cleanup();
});

function makeBullet(
  id: string,
  text: string,
  source_kind: RenderedSection["bullets"][number]["source_kind"],
): RenderedSection["bullets"][number] {
  return { id, text, source_kind };
}

function makeSection(
  id: string,
  title: string,
  bullets: RenderedSection["bullets"],
): RenderedSection {
  return { id, title, bullets };
}

const COMMITS_SECTION = makeSection("commits", "COMMITS", [
  makeBullet("b-1", "Land sign-in flow", "GitHub"),
  makeBullet("b-2", "Tweak readme", "GitHub"),
  makeBullet("b-3", "Add filter", "GitLab"),
  makeBullet("b-4", "Triage tickets", "Jira"),
  makeBullet("b-5", "Edit runbook", "Confluence"),
  makeBullet("b-6", "Stand-up notes", "Outlook"),
  makeBullet("b-7", "Local fixup commit", "LocalGit"),
  makeBullet("b-legacy", "Pre-DAY-104 row", null),
]);

const PRS_SECTION = makeSection("prs", "PULL REQUESTS", [
  makeBullet("p-1", "Open PR for sign-in", "GitHub"),
  makeBullet("p-2", "Review request", "GitHub"),
  makeBullet("p-3", "Merge filter PR", "GitLab"),
]);

describe("aggregateByKind", () => {
  it("returns an empty array when there are no bullets", () => {
    expect(aggregateByKind([])).toEqual([]);
    expect(aggregateByKind([makeSection("empty", "EMPTY", [])])).toEqual([]);
  });

  it("excludes legacy null-source_kind bullets from the chart", () => {
    const sections = [
      makeSection("legacy-only", "LEGACY", [
        makeBullet("l-1", "no kind", null),
        makeBullet("l-2", "no kind", null),
      ]),
    ];
    expect(aggregateByKind(sections)).toEqual([]);
  });

  it("excludes bullets whose source_kind is undefined or unknown", () => {
    // Hard-cast through `unknown` to model what the e2e mock and
    // any pre-DAY-104 persisted draft can carry: the runtime data
    // doesn't always honour the ts-rs-generated `SourceKind | null`
    // type. Without this guard, `connectorAccent(undefined)` throws
    // inside the chart and unmounts `StreamingPreview` entirely
    // (the regression DAY-211's first CI run caught — see the
    // `playwright` job on https://github.com/dayseam/dayseam/pull/181).
    const undefinedKind = makeBullet(
      "u-1",
      "no kind in fixture",
      undefined as unknown as RenderedSection["bullets"][number]["source_kind"],
    );
    const unknownKind = makeBullet(
      "u-2",
      "future Rust enum addition",
      "Slack" as unknown as RenderedSection["bullets"][number]["source_kind"],
    );
    const sections = [
      makeSection("mixed", "MIXED", [
        undefinedKind,
        unknownKind,
        makeBullet("g-1", "real github bullet", "GitHub"),
      ]),
    ];
    expect(aggregateByKind(sections)).toEqual([{ kind: "GitHub", count: 1 }]);
  });

  it("sums counts across multiple sections", () => {
    const counts = aggregateByKind([COMMITS_SECTION, PRS_SECTION]);
    const byKind = Object.fromEntries(counts.map((c) => [c.kind, c.count]));
    expect(byKind).toEqual({
      GitHub: 4,
      GitLab: 2,
      Jira: 1,
      Confluence: 1,
      Outlook: 1,
      LocalGit: 1,
    });
  });

  it("sorts by count descending, breaking ties with the canonical kind order", () => {
    const counts = aggregateByKind([COMMITS_SECTION, PRS_SECTION]);
    expect(counts.map((c) => c.kind)).toEqual([
      "GitHub", // 4 — clear leader
      "GitLab", // 2
      // The remaining four all have count 1; SLICE_ORDER puts
      // LocalGit, Jira, Confluence, Outlook in that order.
      "LocalGit",
      "Jira",
      "Confluence",
      "Outlook",
    ]);
  });
});

describe("<DaySummaryChart />", () => {
  it("renders the empty-state ring and copy when no kinds have bullets", () => {
    render(<DaySummaryChart sections={[]} />);
    expect(screen.getByTestId("day-summary-chart-empty-ring")).toBeInTheDocument();
    expect(
      screen.getByText("No activity recorded today"),
    ).toBeInTheDocument();
    const svg = screen.getByRole("img");
    expect(svg).toHaveAttribute(
      "aria-label",
      "Day summary: no activity recorded today.",
    );
    // The centre count still renders as 0 so the chrome stays
    // identical between the empty and populated states.
    expect(screen.getByTestId("day-summary-chart-total")).toHaveTextContent("0");
  });

  it("renders one closed-ring slice for a single-kind day", () => {
    const sections: RenderedSection[] = [
      makeSection("commits", "COMMITS", [
        makeBullet("b-1", "Only github today", "GitHub"),
        makeBullet("b-2", "Another github thing", "GitHub"),
      ]),
    ];
    render(<DaySummaryChart sections={sections} />);
    const slices = document.querySelectorAll<SVGPathElement>(
      "[data-kind-slice]",
    );
    expect(slices).toHaveLength(1);
    expect(slices[0]).toHaveAttribute("data-kind-slice", "GitHub");
    expect(slices[0]).toHaveAttribute("data-count", "2");
    expect(screen.getByTestId("day-summary-chart-total")).toHaveTextContent("2");
    expect(screen.getByRole("img")).toHaveAttribute(
      "aria-label",
      "Day summary: GitHub 2 — 2 items today.",
    );
  });

  it("paints each slice in its connector accent and exposes the hex pair on the path", () => {
    render(<DaySummaryChart sections={[COMMITS_SECTION, PRS_SECTION]} />);
    const githubSlice = document.querySelector(
      '[data-kind-slice="GitHub"]',
    ) as SVGPathElement | null;
    expect(githubSlice).not.toBeNull();
    expect(githubSlice).toHaveAttribute("data-accent-light", "#24292F");
    expect(githubSlice).toHaveAttribute("data-accent-dark", "#F0F6FC");

    const outlookSlice = document.querySelector(
      '[data-kind-slice="Outlook"]',
    ) as SVGPathElement | null;
    expect(outlookSlice).not.toBeNull();
    expect(outlookSlice).toHaveAttribute("data-accent-light", "#0078D4");
    expect(outlookSlice).toHaveAttribute("data-accent-dark", "#0078D4");
  });

  it("renders a legend whose order matches the slice order", () => {
    render(<DaySummaryChart sections={[COMMITS_SECTION, PRS_SECTION]} />);
    const legendItems = document.querySelectorAll("[data-kind-legend]");
    expect(Array.from(legendItems).map((el) => el.getAttribute("data-kind-legend"))).toEqual([
      "GitHub",
      "GitLab",
      "LocalGit",
      "Jira",
      "Confluence",
      "Outlook",
    ]);
  });

  it("scrolls the matching kind-group into view when a slice is clicked", () => {
    // Render a target outside the chart so the chart's
    // `document.querySelector('[data-kind="..."]')` resolves.
    document.body.insertAdjacentHTML(
      "beforeend",
      '<div id="external-target" data-kind="GitHub" />',
    );
    const target = document.getElementById("external-target");
    expect(target).not.toBeNull();
    const scrollSpy = vi.fn();
    if (target) {
      // jsdom doesn't ship scrollIntoView; install a per-element
      // spy so the chart can call into it without throwing.
      Object.defineProperty(target, "scrollIntoView", {
        configurable: true,
        value: scrollSpy,
      });
    }

    render(<DaySummaryChart sections={[COMMITS_SECTION]} />);
    const githubSlice = document.querySelector(
      '[data-kind-slice="GitHub"]',
    ) as SVGPathElement | null;
    expect(githubSlice).not.toBeNull();

    fireEvent.click(githubSlice!);
    expect(scrollSpy).toHaveBeenCalledTimes(1);
    expect(scrollSpy).toHaveBeenCalledWith({
      behavior: "smooth",
      block: "start",
    });

    target?.remove();
  });

  it("renders a `<title>` per slice with the formatted tooltip copy", () => {
    render(<DaySummaryChart sections={[COMMITS_SECTION, PRS_SECTION]} />);
    const githubSlice = document.querySelector(
      '[data-kind-slice="GitHub"]',
    ) as SVGPathElement | null;
    expect(githubSlice).not.toBeNull();
    const githubTitle = githubSlice?.querySelector("title");
    expect(githubTitle?.textContent).toBe("GitHub — 4 items (40%)");

    const outlookSlice = document.querySelector(
      '[data-kind-slice="Outlook"]',
    ) as SVGPathElement | null;
    const outlookTitle = outlookSlice?.querySelector("title");
    // 1/10 = 10% — formatPercent uses the whole-percent branch
    // exactly at the 10% boundary, so this is the corner case for
    // the formatter.
    expect(outlookTitle?.textContent).toBe("Outlook — 1 item (10%)");
  });

  it("matches the snapshot for the canonical multi-kind day", () => {
    const { container } = render(
      <DaySummaryChart sections={[COMMITS_SECTION, PRS_SECTION]} />,
    );
    expect(container.firstChild).toMatchSnapshot();
  });
});
