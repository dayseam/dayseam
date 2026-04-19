import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { SourceIdentity } from "@dayseam/ipc-types";
import { useIdentities } from "../ipc/useIdentities";
import { registerInvokeHandler, resetTauriMocks } from "./tauri-mock";

const PERSON_ID = "11111111-1111-1111-1111-111111111111";
const IDENTITY: SourceIdentity = {
  id: "22222222-2222-2222-2222-222222222222",
  person_id: PERSON_ID,
  source_id: "33333333-3333-3333-3333-333333333333",
  kind: "GitEmail",
  external_actor_id: "me@example.com",
};

describe("useIdentities", () => {
  beforeEach(() => {
    resetTauriMocks();
    registerInvokeHandler("identities_list_for", async () => [IDENTITY]);
  });

  afterEach(() => {
    resetTauriMocks();
  });

  it("stays idle when personId is null", async () => {
    const { result } = renderHook(() => useIdentities(null));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.identities).toEqual([]);
  });

  it("loads identities for the given person", async () => {
    const { result } = renderHook(() => useIdentities(PERSON_ID));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.identities).toEqual([IDENTITY]);
  });

  it("re-fetches after upsert", async () => {
    let calls = 0;
    registerInvokeHandler("identities_list_for", async () => {
      calls += 1;
      return calls === 1 ? [] : [IDENTITY];
    });
    registerInvokeHandler("identities_upsert", async () => IDENTITY);

    const { result } = renderHook(() => useIdentities(PERSON_ID));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.identities).toEqual([]);

    await act(async () => {
      await result.current.upsert(IDENTITY);
    });
    expect(result.current.identities).toEqual([IDENTITY]);
  });
});
