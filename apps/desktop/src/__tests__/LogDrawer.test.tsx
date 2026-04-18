import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { LogEntry } from "@dayseam/ipc-types";
import { LogDrawer } from "../components/LogDrawer";
import { registerInvokeHandler, resetTauriMocks } from "./tauri-mock";

const SAMPLE: LogEntry[] = [
  {
    timestamp: "2024-01-01T10:00:00Z",
    level: "Info",
    source_id: null,
    message: "app started",
  },
  {
    timestamp: "2024-01-01T10:00:05Z",
    level: "Error",
    source_id: null,
    message: "something failed",
  },
  {
    timestamp: "2024-01-01T10:00:06Z",
    level: "Debug",
    source_id: null,
    message: "trace msg",
  },
];

describe("LogDrawer", () => {
  beforeEach(() => {
    resetTauriMocks();
  });
  afterEach(() => {
    resetTauriMocks();
  });

  it("renders nothing when closed", () => {
    registerInvokeHandler("logs_tail", async () => SAMPLE);
    render(<LogDrawer open={false} onClose={() => {}} />);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("fetches and renders entries when opened", async () => {
    registerInvokeHandler("logs_tail", async () => SAMPLE);
    render(<LogDrawer open onClose={() => {}} />);

    await waitFor(() =>
      expect(screen.getByText(/app started/i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/something failed/i)).toBeInTheDocument();
    expect(screen.getByText(/trace msg/i)).toBeInTheDocument();
  });

  it("hides entries whose level is filtered out", async () => {
    registerInvokeHandler("logs_tail", async () => SAMPLE);
    render(<LogDrawer open onClose={() => {}} />);

    await waitFor(() =>
      expect(screen.getByText(/app started/i)).toBeInTheDocument(),
    );

    // Toggle Debug off.
    fireEvent.click(
      screen.getByRole("checkbox", { name: /debug/i }),
    );

    expect(screen.queryByText(/trace msg/i)).toBeNull();
    expect(screen.getByText(/something failed/i)).toBeInTheDocument();
  });

  it("renders the error state when logs_tail throws", async () => {
    registerInvokeHandler("logs_tail", async () => {
      throw new Error("db closed");
    });
    render(<LogDrawer open onClose={() => {}} />);

    await waitFor(() =>
      expect(screen.getByText(/failed to load logs/i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/db closed/i)).toBeInTheDocument();
  });

  it("calls onClose when Close is clicked", async () => {
    registerInvokeHandler("logs_tail", async () => SAMPLE);
    const onClose = vi.fn();
    render(<LogDrawer open onClose={onClose} />);

    await waitFor(() =>
      expect(screen.getByText(/app started/i)).toBeInTheDocument(),
    );

    fireEvent.click(screen.getByRole("button", { name: /close log drawer/i }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
