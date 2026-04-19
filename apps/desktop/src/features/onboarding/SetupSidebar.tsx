// Setup checklist panel — shared between the full-screen
// `FirstRunEmptyState` and any later "progress chip in the corner"
// UI. Owns the dialog-open state so its host doesn't have to know
// about `PickNameDialog`, `AddLocalGitSourceDialog`, et al.
//
// Invariant (plan §Task 7 #2): once a dialog closes, the corresponding
// checklist item flips to `done` without a full refresh. We lean on
// each dialog's existing mutation → refetch flow: `useSources`,
// `useSinks`, and `useIdentities` already re-query after every
// successful write. For the name step we call back through the hook
// with the `Person` returned by `persons_update_self`.
//
// The sidebar never mounts a dialog until the user explicitly clicks
// an action, so a mostly-completed first run isn't paying the
// per-dialog render cost while the user reads the welcome copy.

import { useCallback, useState } from "react";
import type { Source } from "@dayseam/ipc-types";
import { AddLocalGitSourceDialog } from "../sources/AddLocalGitSourceDialog";
import { ApproveReposDialog } from "../sources/ApproveReposDialog";
import { IdentityManagerDialog } from "../identities/IdentityManagerDialog";
import { SinksDialog } from "../sinks/SinksDialog";
import { PickNameDialog } from "./PickNameDialog";
import { SetupChecklistItemRow } from "./SetupChecklistItem";
import type { SetupChecklistItemId } from "./state";
import type { UseSetupChecklistState } from "./useSetupChecklist";

interface SetupSidebarProps {
  /** Usually the return value of `useSetupChecklist()` — hoisted into
   *  the parent so the same hook instance drives the gate decision
   *  and the sidebar rendering. */
  checklist: UseSetupChecklistState;
}

const ACTION_LABELS: Record<SetupChecklistItemId, string> = {
  name: "Pick a name",
  source: "Add source",
  identity: "Add mappings",
  sink: "Add sink",
};

export function SetupSidebar({ checklist }: SetupSidebarProps) {
  const { items, person, setPerson, refresh, loading, error } = checklist;
  const [openDialog, setOpenDialog] = useState<SetupChecklistItemId | null>(
    null,
  );
  // When `AddLocalGitSourceDialog` resolves successfully we pop the
  // approve-repos dialog on top (mirrors `SourcesSidebar`). Keeping
  // this lifted into the sidebar keeps the two dialogs coordinated
  // without a callback chain between siblings.
  const [approving, setApproving] = useState<Source | null>(null);

  const close = useCallback(() => setOpenDialog(null), []);

  return (
    <div className="flex flex-col gap-3" data-testid="setup-sidebar">
      {error ? (
        <span role="alert" className="text-xs text-red-600 dark:text-red-400">
          Setup status unavailable: {error}
        </span>
      ) : null}

      <ul className="flex flex-col gap-2">
        {items.map((item) => (
          <SetupChecklistItemRow
            key={item.id}
            item={item}
            actionLabel={ACTION_LABELS[item.id]}
            onAction={() => setOpenDialog(item.id)}
            disabled={loading && person === null}
          />
        ))}
      </ul>

      <PickNameDialog
        open={openDialog === "name"}
        initialName={person?.display_name ?? ""}
        onClose={close}
        onSaved={(updated) => setPerson(updated)}
      />

      <AddLocalGitSourceDialog
        open={openDialog === "source"}
        onClose={close}
        onAdded={(source) => {
          close();
          setApproving(source);
          void refresh();
        }}
      />

      {approving ? (
        <ApproveReposDialog
          source={approving}
          onClose={() => {
            setApproving(null);
            void refresh();
          }}
        />
      ) : null}

      <IdentityManagerDialog
        open={openDialog === "identity"}
        onClose={() => {
          close();
          void refresh();
        }}
      />

      <SinksDialog
        open={openDialog === "sink"}
        onClose={() => {
          close();
          void refresh();
        }}
      />
    </div>
  );
}
