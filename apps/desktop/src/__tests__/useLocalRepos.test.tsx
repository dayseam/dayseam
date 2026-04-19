import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { LocalRepo } from "@dayseam/ipc-types";
import { useLocalRepos } from "../ipc/useLocalRepos";
import { registerInvokeHandler, resetTauriMocks } from "./tauri-mock";

const SOURCE_ID = "ssssssss-ssss-ssss-ssss-ssssssssssss";
const REPO: LocalRepo = {
  path: "/Users/me/code/thing",
  label: "thing",
  is_private: false,
  discovered_at: "2026-04-17T12:00:00Z",
};

describe("useLocalRepos", () => {
  beforeEach(() => {
    resetTauriMocks();
    registerInvokeHandler("local_repos_list", async () => [REPO]);
  });

  afterEach(() => {
    resetTauriMocks();
  });

  it("stays idle when sourceId is null", async () => {
    const { result } = renderHook(() => useLocalRepos(null));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.repos).toEqual([]);
  });

  it("loads repos for the given source", async () => {
    const { result } = renderHook(() => useLocalRepos(SOURCE_ID));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.repos).toEqual([REPO]);
  });

  it("re-fetches after `setPrivate`", async () => {
    const updated: LocalRepo = { ...REPO, is_private: true };
    let calls = 0;
    registerInvokeHandler("local_repos_list", async () => {
      calls += 1;
      return calls === 1 ? [REPO] : [updated];
    });
    registerInvokeHandler("local_repos_set_private", async () => updated);

    const { result } = renderHook(() => useLocalRepos(SOURCE_ID));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.repos[0]?.is_private).toBe(false);

    await act(async () => {
      await result.current.setPrivate(REPO.path, true);
    });
    expect(result.current.repos[0]?.is_private).toBe(true);
  });
});
