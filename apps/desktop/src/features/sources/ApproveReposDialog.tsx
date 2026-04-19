// Dialog: after a local-git source is added, show every discovered
// `LocalRepo` so the user can flip `is_private` on the ones they
// don't want mined for content. Privacy defaults to **public** on
// the Rust side; this dialog is the only place where the user
// normally sees the full repo list, so it doubles as a review gate
// before the first sync.
//
// The dialog is deliberately non-blocking: closing it without
// toggling anything is fine, and the user can re-open it later via
// the sources sidebar once that affordance lands. Every toggle
// commits immediately (optimistic UI + `local_repos_set_private`
// under the hood) because batching adds a "discard changes?" prompt
// we don't need for a single boolean per row.

import { useState } from "react";
import type { LocalRepo, Source } from "@dayseam/ipc-types";
import { useLocalRepos } from "../../ipc";
import { Dialog, DialogButton } from "../../components/Dialog";

interface ApproveReposDialogProps {
  source: Source;
  onClose: () => void;
}

export function ApproveReposDialog({ source, onClose }: ApproveReposDialogProps) {
  const { repos, loading, error, setPrivate } = useLocalRepos(source.id);
  const [busyPath, setBusyPath] = useState<string | null>(null);
  const [rowError, setRowError] = useState<string | null>(null);

  const handleToggle = async (repo: LocalRepo, nextIsPrivate: boolean) => {
    setBusyPath(repo.path);
    setRowError(null);
    try {
      await setPrivate(repo.path, nextIsPrivate);
    } catch (err) {
      setRowError(err instanceof Error ? err.message : JSON.stringify(err));
    } finally {
      setBusyPath(null);
    }
  };

  const privateCount = repos.filter((r) => r.is_private).length;

  return (
    <Dialog
      open
      onClose={onClose}
      title={`Review ${source.label}`}
      description={`${repos.length} repo${repos.length === 1 ? "" : "s"} discovered. Toggle "private" on any repo whose content should be redacted in reports.`}
      size="lg"
      testId="approve-repos-dialog"
      footer={
        <>
          <span className="mr-auto text-xs text-neutral-500 dark:text-neutral-400">
            {privateCount} marked private
          </span>
          <DialogButton kind="primary" onClick={onClose}>
            Done
          </DialogButton>
        </>
      }
    >
      {loading && repos.length === 0 ? (
        <p className="text-xs text-neutral-500 dark:text-neutral-400">
          Loading repos…
        </p>
      ) : null}

      {error ? (
        <p
          role="alert"
          className="rounded border border-red-300 bg-red-50 px-3 py-2 text-xs text-red-800 dark:border-red-900 dark:bg-red-950/40 dark:text-red-200"
        >
          Failed to load repos: {error}
        </p>
      ) : null}

      {rowError ? (
        <p
          role="alert"
          className="mb-3 rounded border border-red-300 bg-red-50 px-3 py-2 text-xs text-red-800 dark:border-red-900 dark:bg-red-950/40 dark:text-red-200"
        >
          {rowError}
        </p>
      ) : null}

      {!loading && repos.length === 0 && !error ? (
        <p className="text-xs text-neutral-500 dark:text-neutral-400">
          No repositories found under the configured scan roots.
        </p>
      ) : null}

      {repos.length > 0 ? (
        <ul className="flex flex-col divide-y divide-neutral-200 dark:divide-neutral-800">
          {repos.map((repo) => (
            <li
              key={repo.path}
              className="flex items-center justify-between gap-3 py-2"
              data-testid={`approve-repo-row-${repo.label}`}
            >
              <div className="flex min-w-0 flex-col">
                <span className="truncate text-sm font-medium text-neutral-900 dark:text-neutral-100">
                  {repo.label}
                </span>
                <span
                  title={repo.path}
                  className="truncate font-mono text-[11px] text-neutral-500 dark:text-neutral-400"
                >
                  {repo.path}
                </span>
              </div>
              <label className="flex shrink-0 items-center gap-2 text-xs">
                <input
                  type="checkbox"
                  checked={repo.is_private}
                  disabled={busyPath === repo.path}
                  onChange={(e) => void handleToggle(repo, e.target.checked)}
                />
                <span className="text-neutral-700 dark:text-neutral-200">
                  Private
                </span>
              </label>
            </li>
          ))}
        </ul>
      ) : null}
    </Dialog>
  );
}
