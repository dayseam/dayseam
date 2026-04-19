// Dialog: capture a label + one or more scan roots for a new
// `LocalGit` source and call `sources_add`. On success the parent
// (SourcesSidebar) takes the returned `Source` and opens
// `ApproveReposDialog` so the user can toggle `is_private` on the
// freshly-discovered repos before they're ever scanned.
//
// v0.1 deliberately keeps the UI textual: the user types or pastes
// an absolute path, one per line. A native directory picker
// (`tauri-plugin-dialog`) shows up in Phase 3; until then we avoid
// the extra capability grant and the cross-platform quirks of
// webview-driven folder pickers.

import { useCallback, useMemo, useRef, useState } from "react";
import type { Source } from "@dayseam/ipc-types";
import { useSources } from "../../ipc";
import { Dialog, DialogButton } from "../../components/Dialog";

interface AddLocalGitSourceDialogProps {
  open: boolean;
  onClose: () => void;
  onAdded: (source: Source) => void;
}

function parseScanRoots(raw: string): string[] {
  return raw
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

export function AddLocalGitSourceDialog({
  open,
  onClose,
  onAdded,
}: AddLocalGitSourceDialogProps) {
  const { add } = useSources();
  const [label, setLabel] = useState("");
  const [rootsRaw, setRootsRaw] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const labelRef = useRef<HTMLInputElement>(null);

  const scanRoots = useMemo(() => parseScanRoots(rootsRaw), [rootsRaw]);
  const canSubmit = label.trim().length > 0 && scanRoots.length > 0 && !submitting;

  const reset = useCallback(() => {
    setLabel("");
    setRootsRaw("");
    setError(null);
    setSubmitting(false);
  }, []);

  const handleClose = useCallback(() => {
    if (submitting) return;
    reset();
    onClose();
  }, [submitting, reset, onClose]);

  const handleSubmit = useCallback(async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setError(null);
    try {
      const source = await add("LocalGit", label.trim(), {
        LocalGit: { scan_roots: scanRoots },
      });
      reset();
      onAdded(source);
    } catch (err) {
      setError(err instanceof Error ? err.message : JSON.stringify(err));
      setSubmitting(false);
    }
  }, [add, label, scanRoots, canSubmit, reset, onAdded]);

  return (
    <Dialog
      open={open}
      onClose={handleClose}
      title="Add local git source"
      description="Dayseam scans each root for `.git` directories and creates one repo row per discovery. Everything stays local."
      testId="add-local-git-dialog"
      footer={
        <>
          <DialogButton kind="secondary" onClick={handleClose} disabled={submitting}>
            Cancel
          </DialogButton>
          <DialogButton
            kind="primary"
            type="submit"
            disabled={!canSubmit}
            onClick={() => void handleSubmit()}
          >
            {submitting ? "Scanning…" : "Add and scan"}
          </DialogButton>
        </>
      }
    >
      <form
        className="flex flex-col gap-4"
        onSubmit={(e) => {
          e.preventDefault();
          void handleSubmit();
        }}
      >
        <label className="flex flex-col gap-1">
          <span className="text-xs font-medium text-neutral-700 dark:text-neutral-300">
            Label
          </span>
          <input
            ref={labelRef}
            type="text"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            autoFocus
            placeholder="Work repos"
            className="rounded border border-neutral-300 bg-white px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
          />
        </label>

        <label className="flex flex-col gap-1">
          <span className="text-xs font-medium text-neutral-700 dark:text-neutral-300">
            Scan roots (one absolute path per line)
          </span>
          <textarea
            rows={4}
            value={rootsRaw}
            onChange={(e) => setRootsRaw(e.target.value)}
            placeholder={"/Users/me/code\n/Users/me/work"}
            className="rounded border border-neutral-300 bg-white px-2 py-1.5 font-mono text-xs dark:border-neutral-700 dark:bg-neutral-900"
          />
          <span className="text-[11px] text-neutral-500 dark:text-neutral-400">
            {scanRoots.length} root{scanRoots.length === 1 ? "" : "s"} · each
            root is walked recursively for `.git` directories
          </span>
        </label>

        {error ? (
          <p
            role="alert"
            className="rounded border border-red-300 bg-red-50 px-3 py-2 text-xs text-red-800 dark:border-red-900 dark:bg-red-950/40 dark:text-red-200"
          >
            {error}
          </p>
        ) : null}
      </form>
    </Dialog>
  );
}
