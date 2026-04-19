// React binding for the `sinks_*` IPC surface.
//
// Phase-2 PR-A only needs `list` + `add`; delete lives in a later PR
// once the UI has a "manage sinks" view. Sinks are small, read
// infrequently, and don't stream state, so this mirrors the
// sources / identities hook shape rather than introducing its own
// caching layer.

import { useCallback, useEffect, useState } from "react";
import type { Sink, SinkConfig, SinkKind } from "@dayseam/ipc-types";
import { invoke } from "./invoke";

export interface UseSinksState {
  sinks: Sink[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  add: (kind: SinkKind, label: string, config: SinkConfig) => Promise<Sink>;
}

export function useSinks(): UseSinksState {
  const [sinks, setSinks] = useState<Sink[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const rows = await invoke("sinks_list", {});
      setSinks(rows);
    } catch (err) {
      setError(err instanceof Error ? err.message : JSON.stringify(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const add = useCallback(
    async (kind: SinkKind, label: string, config: SinkConfig) => {
      const sink = await invoke("sinks_add", { kind, label, config });
      await refresh();
      return sink;
    },
    [refresh],
  );

  return { sinks, loading, error, refresh, add };
}
