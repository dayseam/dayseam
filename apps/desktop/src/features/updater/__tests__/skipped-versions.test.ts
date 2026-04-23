import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  __clearSkippedVersionsForTests,
  isSkipped,
  skipVersion,
} from "../skipped-versions";

describe("skipped-versions", () => {
  beforeEach(() => {
    __clearSkippedVersionsForTests();
  });
  afterEach(() => {
    __clearSkippedVersionsForTests();
  });

  it("reports unskipped versions as not skipped", () => {
    expect(isSkipped("0.6.0")).toBe(false);
  });

  it("persists a skipped version so a fresh reader sees it", () => {
    skipVersion("0.6.0");
    expect(isSkipped("0.6.0")).toBe(true);
    // Any other version is unaffected — "skip this version" is
    // per-release, never a global opt-out.
    expect(isSkipped("0.6.1")).toBe(false);
  });

  it("is idempotent — skipping the same version twice does not double-persist", () => {
    skipVersion("0.6.0");
    skipVersion("0.6.0");
    const raw = window.localStorage.getItem("dayseam.updater.skippedVersions");
    expect(JSON.parse(raw ?? "[]")).toEqual(["0.6.0"]);
  });

  it("survives corrupt localStorage by treating it as empty", () => {
    window.localStorage.setItem(
      "dayseam.updater.skippedVersions",
      "{not valid json",
    );
    // Would-have-caught: a prior round trip through the updater
    // that wrote an array and was later replaced by hand-edited
    // garbage should never throw on the next render pass — the
    // banner must simply re-appear, not crash the shell.
    expect(() => isSkipped("0.6.0")).not.toThrow();
    expect(isSkipped("0.6.0")).toBe(false);
  });

  it("tolerates non-string array entries in storage", () => {
    window.localStorage.setItem(
      "dayseam.updater.skippedVersions",
      JSON.stringify(["0.6.0", 42, null, "0.6.1"]),
    );
    expect(isSkipped("0.6.0")).toBe(true);
    expect(isSkipped("0.6.1")).toBe(true);
    // Would-have-caught: the filter drops the numeric/nullish
    // entries without throwing, which matters because a future
    // schema bump that shipped a richer value (e.g. `{ version,
    // skippedAt }`) and was then rolled back would otherwise wedge
    // the updater banner forever.
  });
});
