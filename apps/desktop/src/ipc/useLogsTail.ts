// Imperative fetch + lightweight polling for the persisted log tail.
// The drawer calls `refresh()` on mount and when the user clicks the
// "Refresh" button; no background polling yet — we add one in Phase 2
// once there's enough log churn to warrant it.

import { useCallback, useEffect, useState } from "react";
import type { LogEntry } from "@dayseam/ipc-types";
import { invoke } from "./invoke";

export interface UseLogsTailOptions {
  /** Maximum rows to request. Defaults to the Rust side's default (100). */
  limit?: number;
  /** Auto-fetch on mount. Defaults to `true`. */
  autoLoad?: boolean;
}

export interface UseLogsTailState {
  entries: LogEntry[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

export function useLogsTail(options: UseLogsTailOptions = {}): UseLogsTailState {
  const { limit, autoLoad = true } = options;
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const rows = await invoke("logs_tail", {
        since: null,
        limit: limit ?? null,
      });
      setEntries(rows);
    } catch (err) {
      setError(err instanceof Error ? err.message : JSON.stringify(err));
    } finally {
      setLoading(false);
    }
  }, [limit]);

  useEffect(() => {
    if (!autoLoad) return;
    void refresh();
  }, [autoLoad, refresh]);

  return { entries, loading, error, refresh };
}
