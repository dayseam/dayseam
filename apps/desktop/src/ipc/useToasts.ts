// Listens on the Tauri frontend event bus for `toast` broadcasts
// published by `dayseam-events::AppBus` and exposes them as a queue
// the `ToastHost` component renders.
//
// App-wide broadcasts take the `tauri::Manager::emit` path rather
// than the per-run `Channel<T>` path — see `ARCHITECTURE.md` §11.3 for
// why. Every open window receives every toast.

import { useEffect, useState, useCallback } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ToastEvent } from "@dayseam/ipc-types";

/** Event name the Rust side emits toasts under. */
export const TOAST_EVENT = "toast";

/** Alias kept for readability at call sites. */
export type QueuedToast = ToastEvent;

export function useToasts() {
  const [toasts, setToasts] = useState<QueuedToast[]>([]);

  const push = useCallback((event: ToastEvent) => {
    setToasts((prev) => {
      // Dedupe on id — AppBus already gives every toast a fresh
      // UUID, but a remount can cause the same emit to reach two
      // listeners briefly.
      if (prev.some((t) => t.id === event.id)) return prev;
      return [...prev, event];
    });
  }, []);

  const dismiss = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;
    listen<ToastEvent>(TOAST_EVENT, (evt) => {
      push(evt.payload);
    })
      .then((fn) => {
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch((err) => {
        // In a non-Tauri host (tests without the mock, storybook,
        // etc.) `listen` throws — swallow silently so the host page
        // still renders.
        console.warn("dayseam toast listener failed to attach", err);
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [push]);

  return { toasts, push, dismiss };
}
