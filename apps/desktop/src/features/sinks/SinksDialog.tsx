// Dialog: manage configured sinks. v0.1 has exactly one sink kind
// (`MarkdownFile`) whose only knobs are a label, one or two
// destination directories, and a boolean "wrap output in YAML
// frontmatter" flag. List + add covers Phase 2's needs; a full
// edit/delete surface waits for PR-B2's save-report flow where sink
// selection becomes user-facing.

import { useCallback, useMemo, useState } from "react";
import type { Sink } from "@dayseam/ipc-types";
import { useSinks } from "../../ipc";
import { Dialog, DialogButton } from "../../components/Dialog";

interface SinksDialogProps {
  open: boolean;
  onClose: () => void;
}

const CURRENT_MARKDOWN_CONFIG_VERSION = 1;

function parseDestDirs(raw: string): string[] {
  return raw
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

function sinkSummary(sink: Sink): string {
  if ("MarkdownFile" in sink.config) {
    const cfg = sink.config.MarkdownFile;
    const dirs = cfg.dest_dirs.join(", ");
    return `Markdown · ${dirs}${cfg.frontmatter ? " · frontmatter" : ""}`;
  }
  return "Unknown sink kind";
}

export function SinksDialog({ open, onClose }: SinksDialogProps) {
  const { sinks, loading, error, add } = useSinks();

  const [label, setLabel] = useState("");
  const [destRaw, setDestRaw] = useState("");
  const [frontmatter, setFrontmatter] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const destDirs = useMemo(() => parseDestDirs(destRaw), [destRaw]);
  const canSubmit =
    label.trim().length > 0 &&
    destDirs.length >= 1 &&
    destDirs.length <= 2 &&
    !submitting;

  const handleAdd = useCallback(async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setFormError(null);
    try {
      await add("MarkdownFile", label.trim(), {
        MarkdownFile: {
          config_version: CURRENT_MARKDOWN_CONFIG_VERSION,
          dest_dirs: destDirs,
          frontmatter,
        },
      });
      setLabel("");
      setDestRaw("");
    } catch (err) {
      setFormError(err instanceof Error ? err.message : JSON.stringify(err));
    } finally {
      setSubmitting(false);
    }
  }, [canSubmit, add, label, destDirs, frontmatter]);

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="Sinks"
      description="Where saved reports land. Each sink is a directory Dayseam writes markdown into; Obsidian vaults are just directories on disk."
      size="lg"
      testId="sinks-dialog"
      footer={
        <DialogButton kind="primary" onClick={onClose}>
          Done
        </DialogButton>
      }
    >
      <form
        className="mb-4 flex flex-col gap-2 rounded border border-neutral-200 bg-neutral-50 p-3 dark:border-neutral-800 dark:bg-neutral-900/50"
        onSubmit={(e) => {
          e.preventDefault();
          void handleAdd();
        }}
      >
        <span className="text-xs font-medium text-neutral-700 dark:text-neutral-300">
          Add markdown sink
        </span>
        <label className="flex flex-col gap-1">
          <span className="text-[11px] text-neutral-600 dark:text-neutral-400">
            Label
          </span>
          <input
            type="text"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder="Daily notes"
            className="rounded border border-neutral-300 bg-white px-2 py-1 text-sm dark:border-neutral-700 dark:bg-neutral-900"
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-[11px] text-neutral-600 dark:text-neutral-400">
            Destination directories (one or two absolute paths)
          </span>
          <textarea
            rows={2}
            value={destRaw}
            onChange={(e) => setDestRaw(e.target.value)}
            placeholder={"/Users/me/vault/daily"}
            className="rounded border border-neutral-300 bg-white px-2 py-1 font-mono text-xs dark:border-neutral-700 dark:bg-neutral-900"
          />
          {destDirs.length > 2 ? (
            <span className="text-[11px] text-amber-700 dark:text-amber-400">
              Markdown sinks accept at most two destination directories.
            </span>
          ) : null}
        </label>
        <label className="flex items-center gap-2 text-xs">
          <input
            type="checkbox"
            checked={frontmatter}
            onChange={(e) => setFrontmatter(e.target.checked)}
          />
          <span className="text-neutral-700 dark:text-neutral-200">
            Wrap output with YAML frontmatter (recommended for Obsidian
            Dataview)
          </span>
        </label>
        <div className="flex items-center justify-end">
          <DialogButton kind="secondary" type="submit" disabled={!canSubmit}>
            {submitting ? "Adding…" : "Add sink"}
          </DialogButton>
        </div>
        {formError ? (
          <p role="alert" className="text-xs text-red-600 dark:text-red-400">
            {formError}
          </p>
        ) : null}
      </form>

      {loading && sinks.length === 0 ? (
        <p className="text-xs text-neutral-500 dark:text-neutral-400">
          Loading sinks…
        </p>
      ) : null}

      {error ? (
        <p
          role="alert"
          className="rounded border border-red-300 bg-red-50 px-3 py-2 text-xs text-red-800 dark:border-red-900 dark:bg-red-950/40 dark:text-red-200"
        >
          Failed to load sinks: {error}
        </p>
      ) : null}

      {sinks.length === 0 && !loading && !error ? (
        <p className="text-xs text-neutral-500 dark:text-neutral-400">
          No sinks configured yet.
        </p>
      ) : null}

      {sinks.length > 0 ? (
        <ul className="flex flex-col divide-y divide-neutral-200 dark:divide-neutral-800">
          {sinks.map((sink) => (
            <li
              key={sink.id}
              className="flex items-center justify-between gap-3 py-2"
              data-testid={`sink-row-${sink.id}`}
            >
              <div className="flex min-w-0 flex-col">
                <span className="truncate text-sm font-medium text-neutral-900 dark:text-neutral-100">
                  {sink.label}
                </span>
                <span className="truncate font-mono text-[11px] text-neutral-500 dark:text-neutral-400">
                  {sinkSummary(sink)}
                </span>
              </div>
            </li>
          ))}
        </ul>
      ) : null}
    </Dialog>
  );
}
