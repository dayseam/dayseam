import { emit } from "@tauri-apps/api/event";
import type { ToastEvent, ToastSeverity } from "@dayseam/ipc-types";
import { TOAST_EVENT } from "./useToasts";

/** Broadcast a toast from the renderer so `ToastHost` picks it up.
 *  Fails silently when `emit` is unavailable (e.g. Vitest without a
 *  full Tauri shell). */
export async function emitFrontendToast(
  title: string,
  body: string | null,
  severity: ToastSeverity = "info",
): Promise<void> {
  const event: ToastEvent = {
    id: crypto.randomUUID(),
    severity,
    title,
    body,
    emitted_at: new Date().toISOString(),
  };
  try {
    await emit(TOAST_EVENT, event);
  } catch {
    // Non-Tauri hosts — ignore.
  }
}
