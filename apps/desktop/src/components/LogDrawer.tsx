import { useEffect, useMemo, useState } from "react";
import type { LogEntry, LogLevel } from "@dayseam/ipc-types";
import { useLogsTail } from "../ipc";

const LEVELS: readonly LogLevel[] = ["Debug", "Info", "Warn", "Error"];

const LEVEL_CLASSES: Record<LogLevel, string> = {
  Debug: "bg-neutral-200 text-neutral-700 dark:bg-neutral-800 dark:text-neutral-300",
  Info: "bg-sky-200 text-sky-800 dark:bg-sky-900 dark:text-sky-100",
  Warn: "bg-amber-200 text-amber-800 dark:bg-amber-900 dark:text-amber-100",
  Error: "bg-red-200 text-red-800 dark:bg-red-900 dark:text-red-100",
};

function formatTimestamp(ts: string): string {
  try {
    const d = new Date(ts);
    if (Number.isNaN(d.getTime())) return ts;
    return d.toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  } catch {
    return ts;
  }
}

export interface LogDrawerProps {
  open: boolean;
  onClose: () => void;
}

export function LogDrawer({ open, onClose }: LogDrawerProps) {
  const { entries, loading, error, refresh } = useLogsTail({ autoLoad: open });
  const [activeLevels, setActiveLevels] = useState<Set<LogLevel>>(
    () => new Set(LEVELS),
  );

  // Re-fetch on every open so the drawer always shows fresh entries,
  // not whatever was current when the component first mounted.
  useEffect(() => {
    if (open) void refresh();
  }, [open, refresh]);

  const visibleEntries = useMemo(
    () => entries.filter((e) => activeLevels.has(e.level)),
    [entries, activeLevels],
  );

  if (!open) return null;

  const toggleLevel = (level: LogLevel) => {
    setActiveLevels((prev) => {
      const next = new Set(prev);
      if (next.has(level)) next.delete(level);
      else next.add(level);
      return next;
    });
  };

  return (
    <aside
      role="dialog"
      aria-label="Log drawer"
      aria-modal="false"
      className="fixed inset-y-0 right-0 z-40 flex w-[420px] max-w-full flex-col border-l border-neutral-200 bg-white shadow-xl dark:border-neutral-800 dark:bg-neutral-950"
    >
      <header className="flex items-center justify-between border-b border-neutral-200 px-4 py-3 dark:border-neutral-800">
        <div className="flex flex-col gap-0.5">
          <h2 className="text-sm font-semibold text-neutral-900 dark:text-neutral-50">
            Activity log
          </h2>
          <p className="text-xs text-neutral-500 dark:text-neutral-400">
            Local-only; retained for troubleshooting.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => void refresh()}
            disabled={loading}
            className="rounded border border-neutral-300 px-2 py-1 text-xs text-neutral-700 hover:bg-neutral-50 disabled:opacity-50 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-900"
            title="Refresh"
          >
            {loading ? "Refreshing…" : "Refresh"}
          </button>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close log drawer"
            title="Close (⌘L)"
            className="rounded border border-neutral-300 px-2 py-1 text-xs text-neutral-700 hover:bg-neutral-50 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-900"
          >
            Close
          </button>
        </div>
      </header>

      <div
        role="group"
        aria-label="Log level filters"
        className="flex flex-wrap items-center gap-1 border-b border-neutral-200 px-4 py-2 dark:border-neutral-800"
      >
        {LEVELS.map((level) => (
          <button
            key={level}
            type="button"
            role="checkbox"
            aria-checked={activeLevels.has(level)}
            onClick={() => toggleLevel(level)}
            className={`rounded px-2 py-0.5 text-[11px] uppercase tracking-wide transition ${
              activeLevels.has(level)
                ? LEVEL_CLASSES[level]
                : "bg-transparent text-neutral-400 line-through dark:text-neutral-600"
            }`}
          >
            {level}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-3 font-mono text-xs">
        {error ? (
          <p className="text-red-600 dark:text-red-400">
            Failed to load logs: {error}
          </p>
        ) : visibleEntries.length === 0 ? (
          <p className="text-neutral-500 dark:text-neutral-400">
            {entries.length === 0
              ? "No entries yet."
              : "No entries match the current filters."}
          </p>
        ) : (
          <ul className="flex flex-col gap-1.5">
            {visibleEntries.map((entry, idx) => (
              <LogRow key={`${entry.timestamp}-${idx}`} entry={entry} />
            ))}
          </ul>
        )}
      </div>
    </aside>
  );
}

function LogRow({ entry }: { entry: LogEntry }) {
  return (
    <li className="flex items-start gap-2">
      <span className="shrink-0 text-neutral-500 dark:text-neutral-400">
        {formatTimestamp(entry.timestamp)}
      </span>
      <span
        className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] uppercase tracking-wide ${LEVEL_CLASSES[entry.level]}`}
      >
        {entry.level}
      </span>
      <span className="break-words text-neutral-800 dark:text-neutral-200">
        {entry.message}
      </span>
    </li>
  );
}
