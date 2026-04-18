import { useEffect } from "react";
import type { ToastEvent } from "@dayseam/ipc-types";
import { useToasts } from "../ipc";
import { Toast } from "./Toast";

/** Time in ms before a non-error toast auto-dismisses. Errors persist. */
const AUTO_DISMISS_MS = 4000;

function copyToastToClipboard(toast: ToastEvent) {
  const payload = JSON.stringify(
    {
      id: toast.id,
      severity: toast.severity,
      title: toast.title,
      body: toast.body ?? null,
      emitted_at: toast.emitted_at,
    },
    null,
    2,
  );
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    void navigator.clipboard.writeText(payload);
  }
}

export function ToastHost() {
  const { toasts, dismiss } = useToasts();

  useEffect(() => {
    // Every non-error toast auto-dismisses after AUTO_DISMISS_MS so
    // the stack doesn't grow without bound. We set a timer per toast
    // the first time it appears; clearing on unmount prevents
    // dangling timers from dismissing the next render's toasts.
    const timers: ReturnType<typeof setTimeout>[] = [];
    toasts.forEach((toast) => {
      if (toast.severity === "error") return;
      const id = toast.id;
      const timer = setTimeout(() => dismiss(id), AUTO_DISMISS_MS);
      timers.push(timer);
    });
    return () => {
      timers.forEach(clearTimeout);
    };
  }, [toasts, dismiss]);

  if (toasts.length === 0) return null;

  return (
    <div
      aria-label="Notifications"
      className="pointer-events-none fixed bottom-4 right-4 z-50 flex flex-col gap-2"
    >
      {toasts.map((toast) => (
        <Toast
          key={toast.id}
          toast={toast}
          onDismiss={dismiss}
          onCopy={copyToastToClipboard}
        />
      ))}
    </div>
  );
}
