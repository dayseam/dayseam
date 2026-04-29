import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  LogEvent,
  ProgressEvent,
  ReportCompletedEvent,
  ReportDraft,
  WriteReceipt,
} from "@dayseam/ipc-types";
import { useReport, REPORT_COMPLETED_EVENT } from "../ipc/useReport";
import {
  emitEvent,
  getCreatedChannels,
  mockInvoke,
  registerInvokeHandler,
  resetTauriMocks,
} from "./tauri-mock";

const RUN_ID = "rrrrrrrr-rrrr-rrrr-rrrr-rrrrrrrrrrrr";
const DRAFT_ID = "dddddddd-dddd-dddd-dddd-dddddddddddd";

const DRAFT: ReportDraft = {
  id: DRAFT_ID,
  date: "2026-04-17",
  template_id: "eod",
  template_version: "1.0.0",
  sections: [],
  evidence: [],
  per_source_state: {},
  verbose_mode: false,
  generated_at: "2026-04-17T12:00:00Z",
};

describe("useReport", () => {
  beforeEach(() => {
    resetTauriMocks();
  });

  afterEach(() => {
    resetTauriMocks();
  });

  it("starts idle with no draft or runId", () => {
    const { result } = renderHook(() => useReport());
    expect(result.current.status).toBe("idle");
    expect(result.current.runId).toBeNull();
    expect(result.current.draft).toBeNull();
  });

  it("invokes `report_generate` and exposes the returned runId", async () => {
    registerInvokeHandler("report_generate", async () => RUN_ID);

    const { result } = renderHook(() => useReport());

    await act(async () => {
      await result.current.generate("2026-04-17", ["src-1"]);
    });

    expect(result.current.runId).toBe(RUN_ID);
    expect(mockInvoke).toHaveBeenCalledWith(
      "report_generate",
      expect.objectContaining({
        date: "2026-04-17",
        sourceIds: ["src-1"],
        templateId: null,
      }),
    );
  });

  it("accumulates progress + log events from the channels", async () => {
    registerInvokeHandler("report_generate", async () => RUN_ID);

    const { result } = renderHook(() => useReport());

    await act(async () => {
      await result.current.generate("2026-04-17", ["src-1"]);
    });

    // The `generate` call constructs progress + logs channels in that
    // order, so those are the two most-recent mock channels.
    const channels = getCreatedChannels();
    const progressCh = channels[channels.length - 2];
    const logsCh = channels[channels.length - 1];

    const progressEvent = {
      run_id: RUN_ID,
      phase: { name: "fetch", status: "in_progress" },
    } as unknown as ProgressEvent;
    const logEvent = {
      run_id: RUN_ID,
      level: "info",
      message: "hello",
    } as unknown as LogEvent;

    act(() => {
      progressCh?.deliver(progressEvent);
      logsCh?.deliver(logEvent);
    });

    expect(result.current.progress).toHaveLength(1);
    expect(result.current.logs).toHaveLength(1);
  });

  it("fetches the draft when `report:completed` fires with a draft id", async () => {
    registerInvokeHandler("report_generate", async () => RUN_ID);
    registerInvokeHandler("report_get", async () => DRAFT);

    const { result } = renderHook(() => useReport());

    await act(async () => {
      await result.current.generate("2026-04-17", ["src-1"]);
    });

    const completed: ReportCompletedEvent = {
      run_id: RUN_ID,
      status: "Completed",
      draft_id: DRAFT_ID,
      cancel_reason: null,
    };

    await act(async () => {
      emitEvent(REPORT_COMPLETED_EVENT, completed);
      // Allow the micro-task queued by `invoke("report_get")` to flush.
      await Promise.resolve();
      await Promise.resolve();
    });

    await waitFor(() => expect(result.current.draft).not.toBeNull());
    expect(result.current.status).toBe("completed");
    expect(result.current.draft?.id).toBe(DRAFT_ID);
  });

  it("`save` refuses when no draft has completed yet", async () => {
    const { result } = renderHook(() => useReport());
    await expect(result.current.save("sink-1")).rejects.toThrow(
      /no draft to save/,
    );
  });

  it("`save` forwards draftId and sinkId after completion", async () => {
    registerInvokeHandler("report_generate", async () => RUN_ID);
    registerInvokeHandler("report_get", async () => DRAFT);
    const receipts: WriteReceipt[] = [];
    registerInvokeHandler("report_save", async (args) => {
      expect(args).toEqual({ draftId: DRAFT_ID, sinkId: "sink-1" });
      return receipts;
    });

    const { result } = renderHook(() => useReport());
    await act(async () => {
      await result.current.generate("2026-04-17", ["src-1"]);
    });

    await act(async () => {
      emitEvent(REPORT_COMPLETED_EVENT, {
        run_id: RUN_ID,
        status: "Completed",
        draft_id: DRAFT_ID,
        cancel_reason: null,
      } satisfies ReportCompletedEvent);
      await Promise.resolve();
      await Promise.resolve();
    });
    await waitFor(() => expect(result.current.draft).not.toBeNull());

    await act(async () => {
      const out = await result.current.save("sink-1");
      expect(out).toBe(receipts);
    });
  });

  it("`cancel` is a no-op while no runId has been assigned", async () => {
    const { result } = renderHook(() => useReport());
    await act(async () => {
      await result.current.cancel();
    });
    expect(mockInvoke).not.toHaveBeenCalledWith(
      "report_cancel",
      expect.anything(),
    );
  });

  it("`cancel` calls `report_cancel` with the active runId", async () => {
    registerInvokeHandler("report_generate", async () => RUN_ID);
    registerInvokeHandler("report_cancel", async () => undefined);

    const { result } = renderHook(() => useReport());
    await act(async () => {
      await result.current.generate("2026-04-17", ["src-1"]);
    });
    await act(async () => {
      await result.current.cancel();
    });
    expect(mockInvoke).toHaveBeenCalledWith("report_cancel", { runId: RUN_ID });
  });

  // DAY-128 #2: connectors emit `ProgressPhase::Completed` when a
  // per-source walk finishes. Before this fix, `deriveStatusFromProgress`
  // turned those into the top-level `"completed"` status, which made
  // the Cancel button flip back to "Generate report" mid-run between
  // sources — the "generate button glitch" the user reported. The
  // status must only settle to a terminal value on a run-level
  // progress event (`source_id === null`) *or* on
  // `report:completed`; per-source terminals stay "running".
  it("keeps status running when a per-source Completed event fires mid-run", async () => {
    registerInvokeHandler("report_generate", async () => RUN_ID);

    const { result } = renderHook(() => useReport());

    await act(async () => {
      await result.current.generate("2026-04-17", ["src-1", "src-2"]);
    });

    const channels = getCreatedChannels();
    const progressCh = channels[channels.length - 2];

    const sourceACompleted = {
      run_id: RUN_ID,
      source_id: "src-1",
      phase: { status: "completed", message: "src-1 done" },
      emitted_at: "2026-04-17T12:00:00Z",
    } as unknown as ProgressEvent;

    act(() => {
      progressCh?.deliver(sourceACompleted);
    });

    expect(result.current.status).toBe("running");
  });

  it("settles to completed when a run-level (source_id=null) Completed fires", async () => {
    registerInvokeHandler("report_generate", async () => RUN_ID);

    const { result } = renderHook(() => useReport());

    await act(async () => {
      await result.current.generate("2026-04-17", ["src-1"]);
    });

    const channels = getCreatedChannels();
    const progressCh = channels[channels.length - 2];

    const runLevelCompleted = {
      run_id: RUN_ID,
      source_id: null,
      phase: { status: "completed", message: "run done" },
      emitted_at: "2026-04-17T12:00:00Z",
    } as unknown as ProgressEvent;

    act(() => {
      progressCh?.deliver(runLevelCompleted);
    });

    expect(result.current.status).toBe("completed");
  });

  it("`reset` clears the accumulated state", async () => {
    registerInvokeHandler("report_generate", async () => RUN_ID);

    const { result } = renderHook(() => useReport());
    await act(async () => {
      await result.current.generate("2026-04-17", ["src-1"]);
    });

    act(() => {
      result.current.reset();
    });

    expect(result.current.runId).toBeNull();
    expect(result.current.status).toBe("idle");
    expect(result.current.progress).toEqual([]);
  });

  // DAY-185 #1: a stale `report:completed` from a superseded prior
  // run that arrives during the *new* run's handshake gap used to
  // slip through when `runId` was still null. We stash completions
  // until invoke returns and only apply when `pending.run_id ===
  // runId`, so a stale prior id is discarded at flush time.
  it("ignores a stale report:completed during the handshake gap of a new run", async () => {
    let resolveGenerate: ((id: string) => void) | undefined;
    const generatePromise = new Promise<string>((resolve) => {
      resolveGenerate = resolve;
    });
    registerInvokeHandler("report_generate", () => generatePromise);
    const reportGetSpy = vi.fn(async () => DRAFT);
    registerInvokeHandler("report_get", reportGetSpy);

    const { result } = renderHook(() => useReport());

    let generatePending: Promise<unknown> | undefined;
    act(() => {
      generatePending = result.current.generate("2026-04-17", ["src-1"]);
    });

    // We are now in the handshake gap: `currentRef.runId === null`.
    // A late `report:completed` arrives for the *prior* run (the
    // stale one the supersede path is terminating).
    const stalePriorRun = "ssssssss-ssss-ssss-ssss-ssssssssssss";
    act(() => {
      emitEvent(REPORT_COMPLETED_EVENT, {
        run_id: stalePriorRun,
        status: "Cancelled",
        draft_id: null,
        cancel_reason: { kind: "superseded_by", run_id: RUN_ID },
      } satisfies ReportCompletedEvent);
    });

    // The hook must NOT have flipped to "cancelled" — the prior
    // run's terminal must not leak into the new run's state.
    expect(result.current.status).not.toBe("cancelled");
    // And it must NOT have invoked `report_get` for the prior run.
    expect(reportGetSpy).not.toHaveBeenCalled();

    // Now finish the handshake; the new run's runId arrives.
    await act(async () => {
      resolveGenerate?.(RUN_ID);
      await generatePending;
    });
    expect(result.current.runId).toBe(RUN_ID);
  });

  // DAY-185 #1 (companion): a `report:completed` event that fires
  // while the hook has no foreground run at all (background
  // scheduler catch-up) must not surface a draft into the
  // foreground UI. Without the strict identity check, the listener
  // would write a foreign draft into `state` and flip `status`
  // even though the user never clicked Generate.
  it("ignores report:completed events when no foreground run is active", async () => {
    const reportGetSpy = vi.fn(async () => DRAFT);
    registerInvokeHandler("report_get", reportGetSpy);

    const { result } = renderHook(() => useReport());
    expect(result.current.status).toBe("idle");

    act(() => {
      emitEvent(REPORT_COMPLETED_EVENT, {
        run_id: "background-run",
        status: "Completed",
        draft_id: DRAFT_ID,
        cancel_reason: null,
      } satisfies ReportCompletedEvent);
    });

    expect(result.current.status).toBe("idle");
    expect(result.current.draft).toBeNull();
    expect(reportGetSpy).not.toHaveBeenCalled();
  });

  // DAY-185 #3: when `report_generate` rejects, the Rust side may
  // already have emitted progress events through the channel. The
  // catch block must null `currentRef.current` so subsequent
  // `progress.onmessage` deliveries fail their identity check and
  // do not flip `status: "failed"` back to `"running"`.
  it("a tail progress event after a failed generate does not flip status back to running", async () => {
    registerInvokeHandler("report_generate", async () => {
      throw new Error("orchestrator validation failed");
    });

    const { result } = renderHook(() => useReport());

    // Capture the channels created by this generate so we can
    // deliver a tail event into them after the catch.
    const channelsBefore = getCreatedChannels().length;
    await act(async () => {
      await expect(
        result.current.generate("2026-04-17", ["src-1"]),
      ).rejects.toThrow();
    });
    expect(result.current.status).toBe("failed");

    const channelsAfter = getCreatedChannels();
    const progressCh = channelsAfter[channelsBefore]; // the progress channel created by generate()
    expect(progressCh).toBeDefined();

    act(() => {
      progressCh?.deliver({
        run_id: RUN_ID,
        source_id: null,
        phase: { status: "in_progress", message: "tail event" },
        emitted_at: "2026-04-17T12:00:00Z",
      } as unknown as ProgressEvent);
    });

    expect(result.current.status).toBe("failed");
  });

  // DAY-185 #4: starting a fresh generate should silence the
  // previous run's channels so a stragglers tail event from a
  // prior run cannot reach React state. We verify this by
  // pre-creating one run, swapping a fresh one in, and delivering
  // a tail event into the *first* run's progress channel.
  it("a tail event from a prior run's channel is no-op after a fresh generate", async () => {
    registerInvokeHandler("report_generate", async () => RUN_ID);

    const { result } = renderHook(() => useReport());

    await act(async () => {
      await result.current.generate("2026-04-17", ["src-1"]);
    });
    const firstChannels = getCreatedChannels();
    const firstProgressCh = firstChannels[firstChannels.length - 2];

    // Start a second generate, which swaps the channel pair.
    await act(async () => {
      await result.current.generate("2026-04-18", ["src-1"]);
    });

    const progressBefore = result.current.progress.length;

    // Now drive a tail event into the *first* run's channel. The
    // prior `onmessage` should be silenced; state must not grow.
    act(() => {
      firstProgressCh?.deliver({
        run_id: RUN_ID,
        source_id: null,
        phase: { status: "in_progress", message: "stale tail" },
        emitted_at: "2026-04-17T12:00:00Z",
      } as unknown as ProgressEvent);
    });

    expect(result.current.progress.length).toBe(progressBefore);
  });

  // DAY-185 #5: if the user clicks Cancel during the handshake
  // gap (before `report_generate` returns the runId), the intent
  // must be honoured the moment the runId arrives. Previously the
  // Cancel was a silent no-op and the user had to click again
  // after the run started.
  it("queues a Cancel clicked during the handshake and fires it once the runId arrives", async () => {
    let resolveGenerate: ((id: string) => void) | undefined;
    const generatePromise = new Promise<string>((resolve) => {
      resolveGenerate = resolve;
    });
    registerInvokeHandler("report_generate", () => generatePromise);
    const cancelSpy = vi.fn(async () => undefined);
    registerInvokeHandler("report_cancel", cancelSpy);

    const { result } = renderHook(() => useReport());

    let generatePending: Promise<unknown> | undefined;
    act(() => {
      generatePending = result.current.generate("2026-04-17", ["src-1"]);
    });

    // User clicks Cancel during the handshake gap.
    await act(async () => {
      await result.current.cancel();
    });
    // `report_cancel` should NOT have been invoked yet (no runId
    // available).
    expect(cancelSpy).not.toHaveBeenCalled();

    // Resolve the handshake.
    await act(async () => {
      resolveGenerate?.(RUN_ID);
      await generatePending;
      // Give the queued cancel a microtask to fire.
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(cancelSpy).toHaveBeenCalledWith({ runId: RUN_ID });
  });
});
