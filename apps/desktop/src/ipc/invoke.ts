// Single entry point for every Rust ↔ TS command call. Wraps
// `@tauri-apps/api/core::invoke` with the `Commands` catalogue defined
// in `@dayseam/ipc-types` so a typo in a command name, a missing
// argument, or a drifted return shape is a compile-time error rather
// than a runtime one.
//
// The typed helper is the only way Dayseam's TS code should call Tauri
// commands. Direct `invoke(…)` imports bypass the catalogue and defeat
// the capability/command/TS-type triple-write invariant documented in
// `ARCHITECTURE.md` §6.

import { invoke as tauriInvoke, Channel } from "@tauri-apps/api/core";
import type { CommandName, Commands } from "@dayseam/ipc-types";

export { Channel };

export async function invoke<K extends CommandName>(
  name: K,
  args: Commands[K]["args"],
): Promise<Commands[K]["result"]> {
  // The Tauri runtime accepts plain records; we type our `args` map as
  // the strict argument shape so callers get completion and type
  // checking on every field.
  return tauriInvoke(name, args as Record<string, unknown>);
}
