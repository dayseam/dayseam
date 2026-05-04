import type { SourceKind } from "@dayseam/ipc-types";
import { MARKS } from "./connectorMarks";

/**
 * Return the accent hex pair for `kind`. Exposed so non-SVG call
 * sites (e.g. a chip that wants a faint coloured border, or the
 * identity row's coloured dot) can share the single source of truth
 * for what "the GitHub colour" is without poking at the internals
 * of {@link ConnectorLogo}.
 */
export function connectorAccent(kind: SourceKind): {
  light: string;
  dark: string;
} {
  return MARKS[kind].accent;
}
