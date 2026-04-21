// Normalisation table for the Atlassian workspace-URL field. Holds
// the DAY-82 invariant `workspace_url_normalisation`: whatever the
// user types, the dialog's submit path stores exactly one of these
// shapes on the `SourceConfig::{Jira, Confluence}.workspace_url` row
// — never a silently-upgraded one, never a path-appended one.

import { describe, expect, it } from "vitest";
import {
  atlassianTokenPageUrl,
  normaliseWorkspaceUrl,
} from "../atlassian-workspace-url";

describe("normaliseWorkspaceUrl", () => {
  it("expands a bare slug to the canonical atlassian.net URL", () => {
    expect(normaliseWorkspaceUrl("modulrfinance")).toEqual({
      kind: "ok",
      url: "https://modulrfinance.atlassian.net",
    });
  });

  it("accepts the canonical shape as-is", () => {
    expect(
      normaliseWorkspaceUrl("https://modulrfinance.atlassian.net"),
    ).toEqual({ kind: "ok", url: "https://modulrfinance.atlassian.net" });
  });

  it("strips a trailing slash", () => {
    expect(
      normaliseWorkspaceUrl("https://modulrfinance.atlassian.net/"),
    ).toEqual({ kind: "ok", url: "https://modulrfinance.atlassian.net" });
  });

  it("rejects http:// (Atlassian Cloud is https-only)", () => {
    const result = normaliseWorkspaceUrl("http://modulrfinance.atlassian.net");
    expect(result.kind).toBe("invalid");
  });

  it("rejects input with a path segment", () => {
    const result = normaliseWorkspaceUrl(
      "https://modulrfinance.atlassian.net/wiki",
    );
    expect(result.kind).toBe("invalid");
  });

  it("rejects non-http(s) schemes", () => {
    expect(normaliseWorkspaceUrl("ftp://modulrfinance.atlassian.net").kind)
      .toBe("invalid");
    expect(normaliseWorkspaceUrl("javascript:alert(1)").kind).toBe("invalid");
  });

  it("rejects input with a query string or fragment", () => {
    expect(
      normaliseWorkspaceUrl("https://modulrfinance.atlassian.net?x=1").kind,
    ).toBe("invalid");
    expect(
      normaliseWorkspaceUrl("https://modulrfinance.atlassian.net#abc").kind,
    ).toBe("invalid");
  });

  it("treats empty input as empty (submit stays disabled)", () => {
    expect(normaliseWorkspaceUrl("")).toEqual({ kind: "empty" });
    expect(normaliseWorkspaceUrl("   ")).toEqual({ kind: "empty" });
  });

  it("rejects nonsense strings with special characters", () => {
    expect(normaliseWorkspaceUrl("my workspace").kind).toBe("invalid");
    expect(normaliseWorkspaceUrl("modulrfinance!").kind).toBe("invalid");
  });

  it("accepts a full custom-host shape like 'example.com'", () => {
    // A hostname with a dot and no scheme is expanded to https://
    // rather than treated as a slug — covers the user who pastes
    // the bare host of a custom Atlassian domain.
    expect(normaliseWorkspaceUrl("example.com")).toEqual({
      kind: "ok",
      url: "https://example.com",
    });
  });
});

describe("atlassianTokenPageUrl", () => {
  it("returns the canonical id.atlassian.com API-tokens page", () => {
    expect(atlassianTokenPageUrl()).toBe(
      "https://id.atlassian.com/manage-profile/security/api-tokens",
    );
  });
});
