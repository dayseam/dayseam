// Tracks the set of update versions the user has clicked "Skip this
// version" on. Persisted to `localStorage` (not the `tauri` store)
// because (a) the value is strictly UI state — losing it just
// re-prompts on next version, no data loss — and (b) keeping it in
// the webview storage means `useUpdater` can decide
// synchronously-on-mount whether to surface the banner, without an
// extra IPC round-trip that would force a "checking…" flash for
// returning users who already skipped the current release.
//
// Contract: a skipped version is remembered *by its exact string*.
// If the user skips 0.6.1 and 0.6.2 ships, the banner re-appears —
// that is the desired behaviour. "Skip this version" is a per-
// release decline, never a global opt-out.

const STORAGE_KEY = "dayseam.updater.skippedVersions";

function readSkipped(): Set<string> {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return new Set();
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return new Set();
    return new Set(parsed.filter((v): v is string => typeof v === "string"));
  } catch {
    // Unparseable or access denied (private mode, SSR): treat as
    // empty so the banner falls back to showing — better to nag
    // once than to silently drop an update prompt.
    return new Set();
  }
}

function writeSkipped(versions: Set<string>): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify([...versions]));
  } catch {
    // Quota / private-mode failures are non-fatal; next session
    // simply re-prompts, which is a safe default for an updater UI.
  }
}

export function isSkipped(version: string): boolean {
  return readSkipped().has(version);
}

export function skipVersion(version: string): void {
  const current = readSkipped();
  current.add(version);
  writeSkipped(current);
}

/** Test-only: clear the skip list. Exposed rather than
 *  `resetSkippedVersions` so the intent at the call site is
 *  obvious — it exists only because RTL tests share a
 *  single `localStorage` across the suite and a dangling skip
 *  entry from one test leaks into the next. */
export function __clearSkippedVersionsForTests(): void {
  try {
    window.localStorage.removeItem(STORAGE_KEY);
  } catch {
    // no-op
  }
}
