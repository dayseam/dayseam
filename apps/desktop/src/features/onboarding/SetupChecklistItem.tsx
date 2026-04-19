// A single row of the first-run setup checklist.
//
// Deliberately dumb — the parent (`SetupSidebar`) decides which dialog
// to open and passes in an `onAction` callback. Keeping the item
// decoupled from the dialogs means we can render a static "progress"
// summary elsewhere in the app later without dragging the
// dialog-opening state along.

import type { SetupChecklistItem as ChecklistItem } from "./state";

interface SetupChecklistItemProps {
  item: ChecklistItem;
  /** Copy for the action button (e.g. "Pick a name"). */
  actionLabel: string;
  /** Opens the dialog that satisfies this step. */
  onAction: () => void;
  /** Disables the action button (e.g. while the checklist is loading). */
  disabled?: boolean;
}

export function SetupChecklistItemRow({
  item,
  actionLabel,
  onAction,
  disabled,
}: SetupChecklistItemProps) {
  return (
    <li
      data-testid={`setup-item-${item.id}`}
      data-done={item.done ? "true" : "false"}
      className="flex items-start gap-3 rounded-md border border-neutral-200 bg-white p-3 dark:border-neutral-800 dark:bg-neutral-950"
    >
      <Indicator done={item.done} />
      <div className="flex flex-1 flex-col gap-0.5">
        <span
          className={`text-sm font-medium ${
            item.done
              ? "text-neutral-500 line-through dark:text-neutral-500"
              : "text-neutral-900 dark:text-neutral-100"
          }`}
        >
          {item.title}
        </span>
        <span className="text-xs text-neutral-600 dark:text-neutral-400">
          {item.description}
        </span>
      </div>
      <button
        type="button"
        onClick={onAction}
        disabled={disabled}
        data-testid={`setup-action-${item.id}`}
        className="shrink-0 rounded border border-neutral-300 bg-white px-3 py-1 text-xs font-medium text-neutral-800 transition hover:bg-neutral-50 disabled:cursor-not-allowed disabled:opacity-50 dark:border-neutral-700 dark:bg-neutral-900 dark:text-neutral-100 dark:hover:bg-neutral-800"
      >
        {item.done ? "Edit" : actionLabel}
      </button>
    </li>
  );
}

function Indicator({ done }: { done: boolean }) {
  return (
    <span
      aria-hidden="true"
      className={`mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full text-[10px] font-bold ${
        done
          ? "bg-emerald-500 text-white dark:bg-emerald-400 dark:text-neutral-950"
          : "border border-neutral-300 bg-white text-neutral-400 dark:border-neutral-700 dark:bg-neutral-900"
      }`}
    >
      {done ? "\u2713" : ""}
    </span>
  );
}
