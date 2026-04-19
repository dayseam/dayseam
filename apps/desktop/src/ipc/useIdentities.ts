// React binding for the `identities_*` IPC surface.
//
// Identities are scoped to a [`Person`] — v0.1 always uses the
// canonical self-person — and the hook takes the `personId` as an
// argument so a future multi-person UI can fan out over as many
// people as it wants without a rewrite. Passing `null` keeps the
// hook idle (useful when the parent component is still loading the
// person row).

import { useCallback, useEffect, useState } from "react";
import type { SourceIdentity } from "@dayseam/ipc-types";
import { invoke } from "./invoke";

export interface UseIdentitiesState {
  identities: SourceIdentity[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  upsert: (identity: SourceIdentity) => Promise<SourceIdentity>;
  remove: (id: string) => Promise<void>;
}

export function useIdentities(personId: string | null): UseIdentitiesState {
  const [identities, setIdentities] = useState<SourceIdentity[]>([]);
  const [loading, setLoading] = useState(personId !== null);
  const [error, setError] = useState<string | null>(null);
  // Track the `personId` we last observed so we can synchronously
  // reset `loading` when the caller swaps people. Without this, a
  // parent that goes from `personId=null` to `personId=<uuid>` gets
  // one render where `loading=false` and `identities=[]`, which the
  // first-run checklist gate misreads as "no identities yet, empty
  // state should show". Resetting state during render is a
  // documented React 18 pattern for deriving state from props (see
  // https://react.dev/reference/react/useState#storing-information-from-previous-renders).
  const [trackedPersonId, setTrackedPersonId] = useState<string | null>(personId);
  if (trackedPersonId !== personId) {
    setTrackedPersonId(personId);
    setLoading(personId !== null);
    setError(null);
    setIdentities([]);
  }

  const refresh = useCallback(async () => {
    if (personId === null) {
      setIdentities([]);
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const rows = await invoke("identities_list_for", { personId });
      setIdentities(rows);
    } catch (err) {
      setError(err instanceof Error ? err.message : JSON.stringify(err));
    } finally {
      setLoading(false);
    }
  }, [personId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const upsert = useCallback(
    async (identity: SourceIdentity) => {
      const result = await invoke("identities_upsert", { identity });
      await refresh();
      return result;
    },
    [refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      await invoke("identities_delete", { id });
      await refresh();
    },
    [refresh],
  );

  return { identities, loading, error, refresh, upsert, remove };
}
