import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useRunStreams } from "../ipc/useRunStreams";
import {
  getCreatedChannels,
  registerInvokeHandler,
  resetTauriMocks,
} from "./tauri-mock";
import type {
  LogEvent,
  ProgressEvent,
  RunId,
} from "@dayseam/ipc-types";

const RUN_ID = "11111111-2222-3333-4444-555555555555" as RunId;

function progress(
  idx: number,
  status: "starting" | "in_progress" | "completed" | "failed" = "in_progress",
): ProgressEvent {
  return {
    run_id: RUN_ID,
    source_id: null,
    phase:
      status === "starting"
        ? { status: "starting", message: `step ${idx}` }
        : status === "completed"
          ? { status: "completed", message: `step ${idx}` }
          : status === "failed"
            ? { status: "failed", code: "unknown", message: `step ${idx}` }
            : {
                status: "in_progress",
                completed: idx,
                total: null,
                message: `step ${idx}`,
              },
    emitted_at: new Date().toISOString(),
  };
}

function log(idx: number): LogEvent {
  return {
    run_id: RUN_ID,
    source_id: null,
    level: "Info",
    message: `log ${idx}`,
    context: {},
    emitted_at: new Date().toISOString(),
  };
}

describe("useRunStreams", () => {
  beforeEach(() => {
    resetTauriMocks();
    registerInvokeHandler("dev_start_demo_run", async () => RUN_ID);
  });

  afterEach(() => {
    resetTauriMocks();
  });

  it("reports `idle` before start() is called", () => {
    const { result } = renderHook(() => useRunStreams());
    expect(result.current.status).toBe("idle");
    expect(result.current.progress).toEqual([]);
    expect(result.current.logs).toEqual([]);
    expect(result.current.runId).toBeNull();
  });

  it("preserves the order of progress events as they stream in", async () => {
    const { result } = renderHook(() => useRunStreams());

    await act(async () => {
      await result.current.start();
    });

    const channels = getCreatedChannels();
    expect(channels).toHaveLength(2);
    const [progressCh, logsCh] = channels;

    await act(async () => {
      (progressCh as unknown as { deliver: (e: ProgressEvent) => void }).deliver(
        progress(1, "starting"),
      );
      (progressCh as unknown as { deliver: (e: ProgressEvent) => void }).deliver(
        progress(2),
      );
      (progressCh as unknown as { deliver: (e: ProgressEvent) => void }).deliver(
        progress(3),
      );
      (logsCh as unknown as { deliver: (e: LogEvent) => void }).deliver(log(1));
      (logsCh as unknown as { deliver: (e: LogEvent) => void }).deliver(log(2));
    });

    expect(result.current.runId).toBe(RUN_ID);
    expect(result.current.progress.map((p) => p.phase.message)).toEqual([
      "step 1",
      "step 2",
      "step 3",
    ]);
    expect(result.current.logs.map((l) => l.message)).toEqual(["log 1", "log 2"]);
    expect(result.current.status).toBe("running");
  });

  it("transitions to `completed` when a completed phase arrives", async () => {
    const { result } = renderHook(() => useRunStreams());
    await act(async () => {
      await result.current.start();
    });
    const [progressCh] = getCreatedChannels();
    await act(async () => {
      (progressCh as unknown as { deliver: (e: ProgressEvent) => void }).deliver(
        progress(1, "completed"),
      );
    });
    expect(result.current.status).toBe("completed");
  });

  it("surfaces failures from start() as error state", async () => {
    resetTauriMocks();
    registerInvokeHandler("dev_start_demo_run", async () => {
      throw new Error("boom");
    });

    const { result } = renderHook(() => useRunStreams());

    let caught: unknown = null;
    await act(async () => {
      try {
        await result.current.start();
      } catch (err) {
        caught = err;
      }
    });

    expect(caught).toBeInstanceOf(Error);
    expect((caught as Error).message).toMatch(/boom/);

    await waitFor(() => expect(result.current.status).toBe("failed"));
    expect(result.current.error).toMatch(/boom/);
  });

  it("drops events from a previous run when start() is called again", async () => {
    const { result } = renderHook(() => useRunStreams());
    await act(async () => {
      await result.current.start();
    });
    const firstChannels = getCreatedChannels().slice();

    await act(async () => {
      await result.current.start();
    });

    // Deliver an old progress event on the first, now-orphaned channel.
    await act(async () => {
      (firstChannels[0] as unknown as {
        deliver: (e: ProgressEvent) => void;
      }).deliver(progress(99));
    });

    expect(result.current.progress).toEqual([]);
  });
});
