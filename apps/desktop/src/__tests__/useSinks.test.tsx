import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Sink } from "@dayseam/ipc-types";
import { useSinks } from "../ipc/useSinks";
import { registerInvokeHandler, resetTauriMocks } from "./tauri-mock";

const SINK: Sink = {
  id: "kkkkkkkk-kkkk-kkkk-kkkk-kkkkkkkkkkkk",
  kind: "MarkdownFile",
  label: "Obsidian vault",
  config: {
    MarkdownFile: {
      config_version: 1,
      dest_dirs: ["/Users/me/vault"],
      frontmatter: true,
    },
  },
  created_at: "2026-04-17T12:00:00Z",
  last_write_at: null,
};

describe("useSinks", () => {
  beforeEach(() => {
    resetTauriMocks();
    registerInvokeHandler("sinks_list", async () => [SINK]);
  });

  afterEach(() => {
    resetTauriMocks();
  });

  it("lists sinks on mount", async () => {
    const { result } = renderHook(() => useSinks());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.sinks).toEqual([SINK]);
  });

  it("re-fetches after `add`", async () => {
    let calls = 0;
    registerInvokeHandler("sinks_list", async () => {
      calls += 1;
      return calls === 1 ? [] : [SINK];
    });
    registerInvokeHandler("sinks_add", async () => SINK);

    const { result } = renderHook(() => useSinks());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.sinks).toEqual([]);

    await act(async () => {
      await result.current.add("MarkdownFile", SINK.label, SINK.config);
    });
    expect(result.current.sinks).toEqual([SINK]);
  });
});
