import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Person } from "@dayseam/ipc-types";
import { useSetupChecklist } from "../features/onboarding/useSetupChecklist";
import {
  registerInvokeHandler,
  registerOnboardingComplete,
  resetTauriMocks,
} from "./tauri-mock";

const DEFAULT_PERSON: Person = {
  id: "11111111-1111-1111-1111-111111111111",
  display_name: "Me",
  is_self: true,
};

const NAMED_PERSON: Person = {
  id: "11111111-1111-1111-1111-111111111111",
  display_name: "Vedanth",
  is_self: true,
};

describe("useSetupChecklist", () => {
  beforeEach(() => {
    resetTauriMocks();
  });

  afterEach(() => {
    resetTauriMocks();
  });

  it("reports `complete: true` when all four inputs are satisfied", async () => {
    registerOnboardingComplete();
    const { result } = renderHook(() => useSetupChecklist());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.complete).toBe(true);
    expect(result.current.items.every((i) => i.done)).toBe(true);
  });

  it("keeps `complete: false` when the self-person still has the `Me` sentinel name", async () => {
    registerOnboardingComplete();
    // Override just the person so the other three checklist items stay
    // satisfied. This isolates the name-step derivation.
    registerInvokeHandler("persons_get_self", async () => DEFAULT_PERSON);
    const { result } = renderHook(() => useSetupChecklist());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.complete).toBe(false);
    const name = result.current.items.find((i) => i.id === "name");
    expect(name?.done).toBe(false);
  });

  it("flips the name step without a second fetch when `setPerson` is called", async () => {
    registerOnboardingComplete();
    registerInvokeHandler("persons_get_self", async () => DEFAULT_PERSON);
    const { result } = renderHook(() => useSetupChecklist());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.items.find((i) => i.id === "name")?.done).toBe(false);

    act(() => {
      result.current.setPerson(NAMED_PERSON);
    });
    expect(result.current.items.find((i) => i.id === "name")?.done).toBe(true);
    expect(result.current.complete).toBe(true);
  });

  it("surfaces a fetch error without hanging in the loading state", async () => {
    registerOnboardingComplete();
    registerInvokeHandler("persons_get_self", async () => {
      throw new Error("boom");
    });
    const { result } = renderHook(() => useSetupChecklist());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.error).toMatch(/boom/);
    expect(result.current.person).toBeNull();
    expect(result.current.complete).toBe(false);
  });
});
