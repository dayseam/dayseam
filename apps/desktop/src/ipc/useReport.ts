// React binding for the `report_*` IPC surface.
//
// The hook drives the full generate-report lifecycle from a single
// hook instance: call `generate(date, sourceIds)`, receive the
// typed `Channel<ProgressEvent>` / `Channel<LogEvent>` streams as
// React state, listen for the `report:completed` Tauri window event
// to pick up the final `draft_id`, and expose `cancel()` and
// `save(sinkId)` helpers. The shape deliberately mirrors the
// Phase-1 `useRunStreams` so a future refactor can merge them if
// that ends up being the right abstraction.
//
// Stale-run protection (DAY-185): every `generate()` swaps a fresh
// pair of channels into `currentRef`. The `onmessage` callbacks gate
// on **channel-object identity** (`currentRef.current?.progress !==
// progress`) so a tail event from a prior run cannot reach state.
// The `report:completed` listener matches payloads with
// `payload.run_id === ref.runId` once the handshake has supplied an
// id. During the handshake gap (`ref.runId === null`) completions are
// **stashed** on `pendingCompletionRef` and applied **after** invoke
// returns only when `pending.run_id === runId`, so (a) fast runs that
// finish before IPC returns still hydrate the draft, and (b) a stale
// superseded run id never applies once the new id is known. Background
// scheduler runs can't leak because `ref` is `null` while idle.
//
// Failure path (DAY-185 #3): when the `report_generate` invoke
// itself rejects, the channel may already have queued progress
// events on the Rust side. The catch block nulls `currentRef.current`
// so subsequent message handlers' identity check fails and tail
// events don't flip `failed` back to `running`.
//
// Cancel-during-handshake (DAY-185 #5): if the user clicks Cancel
// before `report_generate` has returned the run id, we record the
// intent on `cancelPendingRef`. When the run id arrives we honour
// it by firing `report_cancel` immediately. Without this, the
// Cancel button is a silent no-op during the handshake gap.

import { useCallback, useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  LogEvent,
  ProgressEvent,
  ReportCompletedEvent,
  ReportDraft,
  RunId,
  SyncRunStatus,
  WriteReceipt,
} from "@dayseam/ipc-types";
import { Channel, invoke } from "./invoke";

/** Name of the window event fired once a run reaches a terminal
 *  [`SyncRunStatus`]. Matches `REPORT_COMPLETED_EVENT` on the Rust
 *  side of `ipc/commands.rs`. */
export const REPORT_COMPLETED_EVENT = "report:completed";

export type ReportStatus =
  | "idle"
  | "starting"
  | "running"
  | "completed"
  | "cancelled"
  | "failed";

export interface ReportState {
  runId: RunId | null;
  status: ReportStatus;
  progress: ProgressEvent[];
  logs: LogEvent[];
  /** The draft fetched after the run completes. `null` while the
   *  run is still in-flight, during cancel/fail, and before the
   *  first `generate()` call. */
  draft: ReportDraft | null;
  error: string | null;
}

const INITIAL: ReportState = {
  runId: null,
  status: "idle",
  progress: [],
  logs: [],
  draft: null,
  error: null,
};

function deriveStatusFromProgress(events: ProgressEvent[]): ReportStatus {
  // DAY-128 #2: connectors emit `ProgressPhase::Completed` when
  // their per-source walk finishes (see e.g. `connector-gitlab`,
  // `connector-jira`, `connector-github`). If we propagate every
  // terminal phase to the top-level run status the Cancel button
  // flips back to Generate the instant one source finishes and
  // then back to Cancel as the next source starts — which is the
  // "Generate button glitch mid-run" the user is reporting on a
  // multi-source selection. Only the orchestrator's run-level
  // completion event carries `source_id === null`; that is the
  // canonical run terminal on the progress stream. All other
  // terminals are per-source and must not change the button.
  const last = events[events.length - 1];
  if (!last) return "starting";
  const phase = last.phase;
  const isRunLevel = last.source_id === null;
  switch (phase.status) {
    case "starting":
      return "starting";
    case "in_progress":
      return "running";
    case "completed":
      return isRunLevel ? "completed" : "running";
    case "cancelled":
      return isRunLevel ? "cancelled" : "running";
    case "failed":
      return isRunLevel ? "failed" : "running";
    default:
      return "running";
  }
}

function statusFromSyncRunStatus(status: SyncRunStatus): ReportStatus {
  switch (status) {
    case "Completed":
      return "completed";
    case "Cancelled":
      return "cancelled";
    case "Failed":
      return "failed";
    default:
      return "running";
  }
}

export interface UseReportState extends ReportState {
  generate: (
    date: string,
    sourceIds: string[],
    templateId?: string | null,
  ) => Promise<RunId>;
  cancel: () => Promise<void>;
  save: (sinkId: string) => Promise<WriteReceipt[]>;
  reset: () => void;
}

export function useReport(): UseReportState {
  const [state, setState] = useState<ReportState>(INITIAL);
  const currentRef = useRef<{
    runId: RunId | null;
    progress: Channel<ProgressEvent>;
    logs: Channel<LogEvent>;
  } | null>(null);
  // `save()` needs the latest draft id synchronously without
  // blocking on a React render cycle; ref mirrors whatever the
  // completion listener last wrote into state.draft.
  const draftIdRef = useRef<string | null>(null);
  // DAY-185 #5: if the user clicks Cancel while `report_generate`
  // is still mid-handshake (no runId yet), record the intent so
  // the awaiting `generate()` call can fire `report_cancel` as
  // soon as it has the run id.
  const cancelPendingRef = useRef<boolean>(false);
  // DAY-185 #2: any in-flight `report_get` triggered by a
  // `report:completed` event must not write into state after the
  // hook unmounts. The outer `useEffect` flips this to `true` in
  // its cleanup; the listener's `.then` checks before calling
  // `setState`. Tracked as a ref because the check happens inside
  // a closure registered on the Tauri event bus.
  const unmountedRef = useRef<boolean>(false);
  // While `ref.runId === null`, stash at most one terminal payload;
  // `generate()` flushes it after invoke returns if `run_id` matches.
  const pendingCompletionRef = useRef<ReportCompletedEvent | null>(null);

  const applyReportCompletion = useCallback((payload: ReportCompletedEvent) => {
    setState((prev) => ({
      ...prev,
      status: statusFromSyncRunStatus(payload.status),
    }));
    if (payload.draft_id) {
      const draftId = payload.draft_id;
      draftIdRef.current = draftId;
      void invoke("report_get", { draftId })
        .then((draft) => {
          if (unmountedRef.current) return;
          setState((prev) => ({ ...prev, draft }));
        })
        .catch((err) => {
          if (unmountedRef.current) return;
          setState((prev) => ({
            ...prev,
            error: err instanceof Error ? err.message : JSON.stringify(err),
          }));
        });
    }
  }, []);

  // One listener per mount pipes `report:completed` into the hook.
  // We keep it here rather than in `generate()` so we don't miss
  // the completion of a run that finishes between the invoke
  // returning and the listener being attached.
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;
    listen<ReportCompletedEvent>(REPORT_COMPLETED_EVENT, (evt) => {
      const payload = evt.payload;
      const ref = currentRef.current;
      // DAY-185 #1: no foreground generate — background completions only.
      if (!ref) return;
      // Handshake gap: stash until invoke returns; flush compares ids.
      if (ref.runId === null) {
        pendingCompletionRef.current = payload;
        return;
      }
      if (payload.run_id !== ref.runId) return;
      applyReportCompletion(payload);
    })
      .then((fn) => {
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch((err) => {
        console.warn("dayseam report listener failed to attach", err);
      });
    return () => {
      cancelled = true;
      unmountedRef.current = true;
      unlisten?.();
    };
  }, [applyReportCompletion]);

  const reset = useCallback(() => {
    currentRef.current = null;
    draftIdRef.current = null;
    cancelPendingRef.current = false;
    pendingCompletionRef.current = null;
    setState(INITIAL);
  }, []);

  const generate = useCallback(
    async (
      date: string,
      sourceIds: string[],
      templateId: string | null = null,
    ) => {
      // DAY-185 #4: any prior run's channels are about to be
      // dropped. Tauri 2's `Channel` has no public unregister
      // API, so the bridge entry stays in `transformCallback`
      // until the page reloads — but we *can* short-circuit any
      // late delivery by replacing the `onmessage` handler with
      // a no-op. That keeps the closure-capture footprint
      // bounded (no setState, no React reconciliation work) even
      // if the Rust side emits one final tail event after we
      // swap.
      const previous = currentRef.current;
      if (previous) {
        previous.progress.onmessage = () => {};
        previous.logs.onmessage = () => {};
      }

      const progress = new Channel<ProgressEvent>();
      const logs = new Channel<LogEvent>();
      currentRef.current = { runId: null, progress, logs };
      draftIdRef.current = null;
      cancelPendingRef.current = false;
      pendingCompletionRef.current = null;

      setState({ ...INITIAL, status: "starting" });

      progress.onmessage = (event) => {
        if (currentRef.current?.progress !== progress) return;
        setState((prev) => {
          const next = [...prev.progress, event];
          return { ...prev, progress: next, status: deriveStatusFromProgress(next) };
        });
      };
      logs.onmessage = (event) => {
        if (currentRef.current?.logs !== logs) return;
        setState((prev) => ({ ...prev, logs: [...prev.logs, event] }));
      };

      try {
        const runId = await invoke("report_generate", {
          date,
          sourceIds,
          templateId,
          progress,
          logs,
        });
        if (currentRef.current?.progress === progress) {
          currentRef.current.runId = runId;
        }
        setState((prev) => ({ ...prev, runId }));
        // `pendingCompletionRef.current` is populated during the awaited
        // `invoke` by the `report:completed` listener. TS 5's control flow
        // for `Ref['current']` after `await` does not preserve
        // `ReportCompletedEvent | null` (it collapses for this read site), so
        // re-assert the nominal shape before comparing to `runId`.
        const stashedTerminal = pendingCompletionRef.current as unknown as
          | ReportCompletedEvent
          | null;
        pendingCompletionRef.current = null;
        const stillOnThisGeneration =
          currentRef.current?.progress === progress;
        if (
          stillOnThisGeneration &&
          stashedTerminal !== null &&
          stashedTerminal.run_id === runId
        ) {
          applyReportCompletion(stashedTerminal);
        }
        // DAY-185 #5: honour a Cancel click that landed during
        // the handshake gap. Read-and-clear under the same
        // identity check so a Cancel intent recorded *before*
        // this generate() ran (impossible today but defensive
        // against future refactors) doesn't bleed into a fresh
        // run.
        if (cancelPendingRef.current && currentRef.current?.progress === progress) {
          cancelPendingRef.current = false;
          void invoke("report_cancel", { runId }).catch((err) => {
            console.warn("dayseam pending-cancel failed", err);
          });
        }
        return runId;
      } catch (err) {
        // DAY-185 #3: null the ref so the channel `onmessage`
        // identity checks fail for any tail progress / log
        // events the Rust side may already have queued. Without
        // this, the channel handler still matches and would
        // flip `failed` back to `running` via
        // `deriveStatusFromProgress`.
        if (currentRef.current?.progress === progress) {
          currentRef.current = null;
        }
        pendingCompletionRef.current = null;
        const message = err instanceof Error ? err.message : JSON.stringify(err);
        setState((prev) => ({ ...prev, status: "failed", error: message }));
        throw err;
      }
    },
    [applyReportCompletion],
  );

  const cancel = useCallback(async () => {
    const runId = currentRef.current?.runId;
    // DAY-185 #5: if there is an active foreground generate but
    // the runId hasn't arrived yet, queue a pending-cancel intent
    // so the awaiting `generate()` call can honour it the moment
    // the runId resolves. Without this, the Cancel button looks
    // wired up (the sidebar flips into the Cancel state on
    // `status: "starting"`) but does nothing during the
    // handshake.
    if (!runId) {
      if (currentRef.current) {
        cancelPendingRef.current = true;
      }
      return;
    }
    await invoke("report_cancel", { runId });
  }, []);

  const save = useCallback(async (sinkId: string) => {
    const draftId = draftIdRef.current;
    if (!draftId) {
      throw new Error("no draft to save; generate a report first");
    }
    return await invoke("report_save", { draftId, sinkId });
  }, []);

  return { ...state, generate, cancel, save, reset };
}
