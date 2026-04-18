import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { ToastEvent } from "@dayseam/ipc-types";
import { ToastHost } from "../components/ToastHost";
import { TOAST_EVENT } from "../ipc";
import { emitEvent, resetTauriMocks } from "./tauri-mock";

function toast(overrides: Partial<ToastEvent> = {}): ToastEvent {
  return {
    id: crypto.randomUUID(),
    severity: "info",
    title: "hello",
    body: null,
    emitted_at: new Date().toISOString(),
    ...overrides,
  };
}

describe("ToastHost", () => {
  beforeEach(() => {
    resetTauriMocks();
  });
  afterEach(() => {
    resetTauriMocks();
  });

  it("renders nothing until a toast is broadcast", () => {
    render(<ToastHost />);
    expect(screen.queryByLabelText(/notifications/i)).toBeNull();
  });

  it("renders a broadcast toast with matching severity", async () => {
    render(<ToastHost />);
    await waitFor(() => {}); // let listen() attach

    act(() => {
      emitEvent(
        TOAST_EVENT,
        toast({ severity: "error", title: "Boom", body: "Something failed" }),
      );
    });

    const banner = await screen.findByTestId("toast-error");
    expect(banner).toHaveTextContent(/boom/i);
    expect(banner).toHaveTextContent(/something failed/i);
    expect(banner).toHaveAttribute("role", "alert");
  });

  it("deduplicates toasts with the same id", async () => {
    render(<ToastHost />);
    await waitFor(() => {});

    const t = toast({ severity: "warning", title: "Dup" });
    act(() => {
      emitEvent(TOAST_EVENT, t);
      emitEvent(TOAST_EVENT, t);
    });

    await waitFor(() => {
      expect(screen.getAllByTestId("toast-warning")).toHaveLength(1);
    });
  });
});
