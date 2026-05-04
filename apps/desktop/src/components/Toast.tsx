import type { ToastEvent, ToastSeverity } from "@dayseam/ipc-types";

const SEVERITY_CLASSES: Record<ToastSeverity, string> = {
  info: "border-sky-300 bg-sky-50 text-sky-900 dark:border-sky-700 dark:bg-sky-950/60 dark:text-sky-100",
  success:
    "border-emerald-300 bg-emerald-50 text-emerald-900 dark:border-emerald-700 dark:bg-emerald-950/60 dark:text-emerald-100",
  warning:
    "border-amber-300 bg-amber-50 text-amber-900 dark:border-amber-700 dark:bg-amber-950/60 dark:text-amber-100",
  error:
    "border-red-300 bg-red-50 text-red-900 dark:border-red-700 dark:bg-red-950/60 dark:text-red-100",
};

const SEVERITY_LABEL: Record<ToastSeverity, string> = {
  info: "Info",
  success: "Success",
  warning: "Warning",
  error: "Error",
};

export interface ToastProps {
  toast: ToastEvent;
  onDismiss: (id: string) => void;
  onCopy: (toast: ToastEvent) => void;
}

export function Toast({ toast, onDismiss, onCopy }: ToastProps) {
  const severityClass = SEVERITY_CLASSES[toast.severity];
  return (
    <div
      role={toast.severity === "error" ? "alert" : "status"}
      aria-live={toast.severity === "error" ? "assertive" : "polite"}
      data-testid={`toast-${toast.severity}`}
      className={`pointer-events-auto flex w-80 flex-col gap-1 rounded-md border px-3 py-2 text-sm shadow-xs ${severityClass}`}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex flex-col gap-0.5">
          <div className="flex items-center gap-2">
            <span
              aria-hidden="true"
              className="rounded bg-black/10 px-1.5 py-0.5 text-[10px] uppercase tracking-wide dark:bg-white/10"
            >
              {SEVERITY_LABEL[toast.severity]}
            </span>
            <span className="font-medium">{toast.title}</span>
          </div>
          {toast.body ? (
            <p className="text-xs opacity-80">{toast.body}</p>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <button
            type="button"
            onClick={() => onCopy(toast)}
            title="Copy toast details to clipboard"
            className="rounded px-1.5 py-0.5 text-xs opacity-70 transition hover:bg-black/5 hover:opacity-100 dark:hover:bg-white/10"
          >
            Copy
          </button>
          <button
            type="button"
            onClick={() => onDismiss(toast.id)}
            aria-label="Dismiss"
            title="Dismiss"
            className="rounded px-1.5 py-0.5 text-xs opacity-70 transition hover:bg-black/5 hover:opacity-100 dark:hover:bg-white/10"
          >
            ×
          </button>
        </div>
      </div>
    </div>
  );
}
