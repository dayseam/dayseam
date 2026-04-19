// Sources row — the configured-sources strip directly under the
// action bar. This replaces the Phase-1 static `SOURCE_PLACEHOLDERS`
// chips with live rows fed by `useSources()`.
//
// Each row renders a dashed-border chip plus a health dot (green =
// `ok`, amber = never checked, red = last probe failed) and a repo
// count for `LocalGit` sources. Hovering a chip reveals a "Rescan"
// affordance that fires `sources_healthcheck(id)`.
//
// Phase 2 intentionally leaves the chip non-clickable for selection
// — the generate-report view (PR-B2) owns source selection via its
// own multi-select. Keeping this row read-only avoids accidentally
// teaching users two different selection gestures.

import { useCallback, useState } from "react";
import type { Source, SourceHealth } from "@dayseam/ipc-types";
import { useSources } from "../../ipc";
import { AddLocalGitSourceDialog } from "./AddLocalGitSourceDialog";
import { ApproveReposDialog } from "./ApproveReposDialog";

function healthDotClass(health: SourceHealth): string {
  if (!health.checked_at) return "bg-neutral-300 dark:bg-neutral-600";
  return health.ok
    ? "bg-emerald-500 dark:bg-emerald-400"
    : "bg-red-500 dark:bg-red-400";
}

function healthTitle(health: SourceHealth): string {
  if (!health.checked_at) return "Not yet probed";
  if (health.ok) return `Healthy — last checked ${formatWhen(health.checked_at)}`;
  // Every `DayseamError` variant carries a `code` in its `data` blob;
  // the discriminated-union shape means we read it through the nested
  // `.data` rather than a flat `.code`.
  const code = health.last_error?.data.code ?? "unknown";
  return `Error (${code}) at ${formatWhen(health.checked_at)}`;
}

function formatWhen(ts: string): string {
  try {
    const d = new Date(ts);
    if (Number.isNaN(d.getTime())) return ts;
    return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  } catch {
    return ts;
  }
}

function repoCount(source: Source): number | null {
  if ("LocalGit" in source.config) {
    return source.config.LocalGit.scan_roots.length;
  }
  return null;
}

export function SourcesSidebar() {
  const { sources, loading, error, refresh, healthcheck } = useSources();
  const [addOpen, setAddOpen] = useState(false);
  // When `AddLocalGitSourceDialog` resolves successfully it hands the
  // newly-created source here so we can open ApproveReposDialog on
  // top; keeping the two dialogs coordinated at the parent avoids
  // callback choreography across siblings.
  const [approving, setApproving] = useState<Source | null>(null);

  const handleHealthcheck = useCallback(
    (id: string) => {
      void healthcheck(id);
    },
    [healthcheck],
  );

  return (
    <section
      aria-label="Connected sources"
      className="flex flex-wrap items-center gap-2 border-b border-neutral-200 px-6 py-3 dark:border-neutral-800"
    >
      <span className="text-xs uppercase tracking-wide text-neutral-500 dark:text-neutral-400">
        Sources
      </span>

      {loading && sources.length === 0 ? (
        <span className="text-xs text-neutral-400 dark:text-neutral-500">
          Loading…
        </span>
      ) : null}

      {error ? (
        <span
          role="alert"
          className="text-xs text-red-600 dark:text-red-400"
          title={error}
        >
          Failed to load sources
        </span>
      ) : null}

      {sources.map((source) => {
        const count = repoCount(source);
        return (
          <span
            key={source.id}
            title={healthTitle(source.last_health)}
            className="group inline-flex items-center gap-1.5 rounded border border-neutral-300 px-2 py-0.5 text-xs text-neutral-700 dark:border-neutral-700 dark:text-neutral-200"
            data-testid={`source-chip-${source.id}`}
          >
            <span
              aria-hidden="true"
              className={`h-1.5 w-1.5 rounded-full ${healthDotClass(source.last_health)}`}
            />
            <span>{source.label}</span>
            {count !== null ? (
              <span className="text-neutral-500 dark:text-neutral-400">
                · {count} root{count === 1 ? "" : "s"}
              </span>
            ) : null}
            <button
              type="button"
              onClick={() => handleHealthcheck(source.id)}
              className="rounded px-1 text-[11px] text-neutral-500 opacity-0 transition group-hover:opacity-100 hover:bg-neutral-100 dark:text-neutral-400 dark:hover:bg-neutral-800"
              aria-label={`Rescan ${source.label}`}
              title="Rescan"
            >
              ↻
            </button>
          </span>
        );
      })}

      {sources.length === 0 && !loading && !error ? (
        <span className="text-xs text-neutral-400 dark:text-neutral-500">
          No sources connected
        </span>
      ) : null}

      <button
        type="button"
        onClick={() => setAddOpen(true)}
        className="ml-auto rounded border border-neutral-300 px-2 py-0.5 text-xs text-neutral-700 hover:bg-neutral-50 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-900"
      >
        Add local git source
      </button>

      <AddLocalGitSourceDialog
        open={addOpen}
        onClose={() => setAddOpen(false)}
        onAdded={(source) => {
          setAddOpen(false);
          setApproving(source);
          // Refresh in the background so the chip appears even if
          // the user dismisses the approve dialog without approving.
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
    </section>
  );
}
