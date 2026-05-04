/** Destructive footer block for every connector edit dialog — one
 *  consistent entry point to the same delete confirmation flow the
 *  sources strip used to expose via an inline ✕ affordance. */

interface SourceEditDeleteDangerZoneProps {
  onRequestDelete: () => void;
  disabled?: boolean;
}

export function SourceEditDeleteDangerZone({
  onRequestDelete,
  disabled = false,
}: SourceEditDeleteDangerZoneProps) {
  return (
    <div className="mt-2 border-t border-neutral-200 pt-3 dark:border-neutral-800">
      <p className="mb-2 text-[11px] text-neutral-500 dark:text-neutral-400">
        Permanently remove this source from Dayseam. You will be asked to
        confirm; folders on disk are never touched.
      </p>
      <button
        type="button"
        disabled={disabled}
        onClick={onRequestDelete}
        data-testid="source-edit-delete-trigger"
        className="w-full rounded border border-red-300 bg-red-50 px-3 py-1.5 text-xs font-medium text-red-800 hover:bg-red-100 disabled:cursor-not-allowed disabled:opacity-50 dark:border-red-800 dark:bg-red-950/50 dark:text-red-100 dark:hover:bg-red-900/60"
      >
        Delete source…
      </button>
    </div>
  );
}
