// React binding for the `sources_*` IPC surface.
//
// The hook owns the list of configured [`Source`]s plus the four
// mutating operations the Phase 2 UI needs — add, update, delete,
// and healthcheck. Every successful mutation re-fetches the list so
// consumers never have to manually reconcile the optimistic state
// with what the Rust side stored. A re-fetch is one round-trip and
// happens off the critical path (after the Rust call resolves); the
// list is small (< a few dozen entries in practice) so there's no
// real latency cost and we avoid the two-state-machines bug trap.
//
// Mutations surface their errors by rethrowing so the caller can
// show them inline (e.g. on the form that initiated the call) while
// keeping the list state itself free of transient mutation errors.

import { useCallback, useEffect, useState } from "react";
import type {
  Source,
  SourceConfig,
  SourceHealth,
  SourceKind,
  SourcePatch,
} from "@dayseam/ipc-types";
import { invoke } from "./invoke";

export interface UseSourcesState {
  sources: Source[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  add: (
    kind: SourceKind,
    label: string,
    config: SourceConfig,
  ) => Promise<Source>;
  update: (id: string, patch: SourcePatch) => Promise<Source>;
  remove: (id: string) => Promise<void>;
  healthcheck: (id: string) => Promise<SourceHealth>;
}

export function useSources(): UseSourcesState {
  const [sources, setSources] = useState<Source[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const rows = await invoke("sources_list", {});
      setSources(rows);
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
    async (kind: SourceKind, label: string, config: SourceConfig) => {
      const source = await invoke("sources_add", { kind, label, config });
      await refresh();
      return source;
    },
    [refresh],
  );

  const update = useCallback(
    async (id: string, patch: SourcePatch) => {
      const source = await invoke("sources_update", { id, patch });
      await refresh();
      return source;
    },
    [refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      await invoke("sources_delete", { id });
      await refresh();
    },
    [refresh],
  );

  const healthcheck = useCallback(
    async (id: string) => {
      const health = await invoke("sources_healthcheck", { id });
      await refresh();
      return health;
    },
    [refresh],
  );

  return { sources, loading, error, refresh, add, update, remove, healthcheck };
}
