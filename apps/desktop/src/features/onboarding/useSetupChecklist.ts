// First-run setup checklist — composition hook.
//
// This is the single source of truth the UI reads to decide "show the
// empty state" vs. "show the normal app", and which steps still need
// attention. It layers three existing list hooks (`useSources`,
// `useIdentities`, `useSinks`) plus a one-shot fetch of the canonical
// self-`Person` on top of the pure
// [`deriveSetupChecklist`](./state.ts) selector.
//
// Every list hook already re-fetches after a successful mutation, so
// the plan's invariant #2 (`checklist_item_completes_on_dialog_close`)
// holds for free — the dialogs satisfying each step either call through
// those hooks or call `refresh` directly. The one exception is the
// `"name"` step, which goes through `persons_update_self`; that
// command has no list hook to re-fetch, so the caller (the
// `PickNameDialog`) returns the updated `Person` and we drop it back
// into state here.

import { useCallback, useEffect, useMemo, useState } from "react";
import type { Person } from "@dayseam/ipc-types";
import { invoke, useIdentities, useSinks, useSources } from "../../ipc";
import {
  type SetupChecklistItem,
  type SetupChecklistStatus,
  deriveSetupChecklist,
} from "./state";

export interface UseSetupChecklistState {
  items: SetupChecklistItem[];
  complete: boolean;
  /** `true` while any of the underlying fetches has not yet resolved. */
  loading: boolean;
  /** First non-null error across the inputs, or `null`. */
  error: string | null;
  /** Canonical self-`Person`, or `null` before the first fetch resolves. */
  person: Person | null;
  /**
   * Swap in a new `Person` without a round-trip — the
   * `persons_update_self` command returns the updated row, and the
   * name-dialog calls this instead of a full refresh so the checklist
   * item flips instantly.
   */
  setPerson: (person: Person) => void;
  /** Force-refresh every input. Useful after out-of-band DB changes. */
  refresh: () => Promise<void>;
}

export function useSetupChecklist(): UseSetupChecklistState {
  const [person, setPersonState] = useState<Person | null>(null);
  const [personLoading, setPersonLoading] = useState(true);
  const [personError, setPersonError] = useState<string | null>(null);

  const sources = useSources();
  const identities = useIdentities(person?.id ?? null);
  const sinks = useSinks();

  const refreshPerson = useCallback(async () => {
    setPersonLoading(true);
    setPersonError(null);
    try {
      const row = await invoke("persons_get_self", {});
      setPersonState(row);
    } catch (err) {
      setPersonError(err instanceof Error ? err.message : JSON.stringify(err));
    } finally {
      setPersonLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshPerson();
  }, [refreshPerson]);

  const refresh = useCallback(async () => {
    await Promise.all([
      refreshPerson(),
      sources.refresh(),
      identities.refresh(),
      sinks.refresh(),
    ]);
  }, [refreshPerson, sources, identities, sinks]);

  const status: SetupChecklistStatus = useMemo(
    () =>
      deriveSetupChecklist({
        person,
        sources: sources.sources,
        identities: identities.identities,
        sinks: sinks.sinks,
      }),
    [person, sources.sources, identities.identities, sinks.sinks],
  );

  const loading =
    personLoading || sources.loading || identities.loading || sinks.loading;
  const error =
    personError ?? sources.error ?? identities.error ?? sinks.error ?? null;

  return {
    items: status.items,
    complete: status.complete,
    loading,
    error,
    person,
    setPerson: setPersonState,
    refresh,
  };
}
