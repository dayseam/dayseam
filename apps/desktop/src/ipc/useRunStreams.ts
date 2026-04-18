// React hook that starts a demo run on the Rust side and exposes the
// per-run progress / log streams as ordered React state.
//
// Phase 1 only has the `dev_start_demo_run` command to exercise the
// per-run streaming model end-to-end — Task 6 of Phase 2 will swap
// that for the real `run_start`. The hook's shape is deliberately
// stable across both so wiring a real run into the UI is a one-line
// change.

import { useCallback, useRef, useState } from "react";
import type { LogEvent, ProgressEvent, RunId } from "@dayseam/ipc-types";
import { Channel, invoke } from "./invoke";

export type RunStreamStatus =
  | "idle"
  | "starting"
  | "running"
  | "completed"
  | "failed";

export interface RunStreamsState {
  runId: RunId | null;
  status: RunStreamStatus;
  progress: ProgressEvent[];
  logs: LogEvent[];
  error: string | null;
}

const INITIAL: RunStreamsState = {
  runId: null,
  status: "idle",
  progress: [],
  logs: [],
  error: null,
};

function deriveStatus(events: ProgressEvent[]): RunStreamStatus {
  const last = events[events.length - 1]?.phase;
  if (!last) return "starting";
  switch (last.status) {
    case "starting":
      return "starting";
    case "in_progress":
      return "running";
    case "completed":
      return "completed";
    case "failed":
      return "failed";
    default:
      return "running";
  }
}

export function useRunStreams() {
  const [state, setState] = useState<RunStreamsState>(INITIAL);
  // We keep the live channels on a ref so a second `start()` call
  // can tear the previous run's channels down before starting a new
  // one, preventing stale events from leaking into the UI.
  const currentRef = useRef<{
    progress: Channel<ProgressEvent>;
    logs: Channel<LogEvent>;
  } | null>(null);

  const reset = useCallback(() => {
    currentRef.current = null;
    setState(INITIAL);
  }, []);

  const start = useCallback(async () => {
    const progress = new Channel<ProgressEvent>();
    const logs = new Channel<LogEvent>();
    currentRef.current = { progress, logs };

    setState({ ...INITIAL, status: "starting" });

    progress.onmessage = (event) => {
      if (currentRef.current?.progress !== progress) return;
      setState((prev) => {
        const next = [...prev.progress, event];
        return { ...prev, progress: next, status: deriveStatus(next) };
      });
    };
    logs.onmessage = (event) => {
      if (currentRef.current?.logs !== logs) return;
      setState((prev) => ({ ...prev, logs: [...prev.logs, event] }));
    };

    try {
      const runId = await invoke("dev_start_demo_run", { progress, logs });
      setState((prev) => ({ ...prev, runId }));
      return runId;
    } catch (err) {
      const message = err instanceof Error ? err.message : JSON.stringify(err);
      setState((prev) => ({ ...prev, status: "failed", error: message }));
      throw err;
    }
  }, []);

  return { ...state, start, reset };
}
