// Dialog: "pick your display name".
//
// Small single-field form that calls `persons_update_self` and hands
// the updated `Person` back to the caller (the setup checklist) so the
// checklist item flips to `done` without waiting on a second
// `persons_get_self` round-trip.
//
// The dialog does only frontend-side validation: trim the input,
// reject an empty string. The backend repeats the check
// (`IPC_INVALID_DISPLAY_NAME`); we rely on that for anything the form
// missed (e.g. a string with only zero-width whitespace characters).

import { useCallback, useEffect, useState } from "react";
import type { Person } from "@dayseam/ipc-types";
import { invoke } from "../../ipc";
import { Dialog, DialogButton } from "../../components/Dialog";

interface PickNameDialogProps {
  open: boolean;
  /** Current display name — prefilled when the dialog opens. */
  initialName: string;
  onClose: () => void;
  /** Invoked with the updated `Person` when the save succeeds. */
  onSaved: (person: Person) => void;
}

export function PickNameDialog({
  open,
  initialName,
  onClose,
  onSaved,
}: PickNameDialogProps) {
  const [name, setName] = useState(initialName);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reset the form every time the dialog re-opens so a cancelled edit
  // doesn't leak into the next open. We key on `open` rather than
  // `initialName` because an out-of-band name change while the dialog
  // is already open would be surprising to overwrite mid-edit.
  useEffect(() => {
    if (open) {
      setName(initialName);
      setError(null);
      setSubmitting(false);
    }
  }, [open, initialName]);

  const trimmed = name.trim();
  const canSubmit = trimmed.length > 0 && !submitting;

  const handleSubmit = useCallback(async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setError(null);
    try {
      const updated = await invoke("persons_update_self", {
        displayName: trimmed,
      });
      onSaved(updated);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : JSON.stringify(err));
      setSubmitting(false);
    }
  }, [canSubmit, trimmed, onSaved, onClose]);

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="Pick your name"
      description="How Dayseam labels every report and log line it writes on your behalf. Change it later from Settings."
      testId="pick-name-dialog"
      footer={
        <>
          <DialogButton kind="secondary" onClick={onClose} disabled={submitting}>
            Cancel
          </DialogButton>
          <DialogButton
            kind="primary"
            onClick={() => void handleSubmit()}
            disabled={!canSubmit}
          >
            Save
          </DialogButton>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void handleSubmit();
        }}
        className="flex flex-col gap-3"
      >
        <label
          htmlFor="pick-name-input"
          className="text-xs font-medium text-neutral-700 dark:text-neutral-300"
        >
          Display name
        </label>
        <input
          id="pick-name-input"
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Your name"
          className="rounded border border-neutral-300 bg-white px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
        />
        {error ? (
          <span
            role="alert"
            className="text-xs text-red-600 dark:text-red-400"
          >
            {error}
          </span>
        ) : null}
      </form>
    </Dialog>
  );
}
