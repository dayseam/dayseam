import { describe, expect, it } from "vitest";
import { ATLASSIAN_ERROR_CODES } from "@dayseam/ipc-types";
import { atlassianErrorCopy } from "../atlassianErrorCopy";

describe("atlassianErrorCopy parity", () => {
  it("every atlassian error code has copy", () => {
    // `ATLASSIAN_ERROR_CODES` is regenerated from
    // `dayseam_core::error_codes::ALL` by the Rust
    // `ts_types_generated` test, so this test fails whenever a new
    // `atlassian.*` / `jira.*` / `confluence.*` code lands in the
    // Rust source of truth without a matching entry here.
    for (const code of ATLASSIAN_ERROR_CODES) {
      expect(
        atlassianErrorCopy[code],
        `missing atlassianErrorCopy entry for ${code}`,
      ).toBeDefined();
      expect(atlassianErrorCopy[code]?.title).toBeTruthy();
      expect(atlassianErrorCopy[code]?.body).toBeTruthy();
    }
  });

  it("does not carry stale entries for retired codes", () => {
    const known = new Set<string>(ATLASSIAN_ERROR_CODES);
    for (const key of Object.keys(atlassianErrorCopy)) {
      expect(
        known.has(key),
        `stale atlassianErrorCopy entry: ${key}`,
      ).toBe(true);
    }
  });
});
