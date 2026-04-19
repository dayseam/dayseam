import { describe, expect, it } from "vitest";
import type {
  Person,
  Sink,
  Source,
  SourceIdentity,
} from "@dayseam/ipc-types";
import {
  SELF_DEFAULT_DISPLAY_NAME,
  deriveSetupChecklist,
} from "../features/onboarding/state";

// Minimal fixtures — `deriveSetupChecklist` only looks at `.length`
// and `person.display_name`, so we can keep these terse.

const SOURCE: Source = {
  id: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
  kind: "LocalGit",
  label: "work",
  config: { LocalGit: { scan_roots: ["/Users/me/code"] } },
  secret_ref: null,
  created_at: "2026-04-17T12:00:00Z",
  last_sync_at: null,
  last_health: { ok: true, checked_at: null, last_error: null },
};

const IDENTITY: SourceIdentity = {
  id: "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
  person_id: "cccccccc-cccc-cccc-cccc-cccccccccccc",
  source_id: null,
  kind: "GitEmail",
  external_actor_id: "me@example.com",
};

const SINK: Sink = {
  id: "dddddddd-dddd-dddd-dddd-dddddddddddd",
  kind: "MarkdownFile",
  label: "notes",
  config: {
    MarkdownFile: {
      config_version: 1,
      dest_dirs: ["/Users/me/notes"],
      frontmatter: false,
    },
  },
  created_at: "2026-04-17T12:00:00Z",
  last_write_at: null,
};

function person(name: string): Person {
  return {
    id: "cccccccc-cccc-cccc-cccc-cccccccccccc",
    display_name: name,
    is_self: true,
  };
}

describe("deriveSetupChecklist", () => {
  it("marks every step as pending on a fresh install", () => {
    const { items, complete } = deriveSetupChecklist({
      person: null,
      sources: [],
      identities: [],
      sinks: [],
    });
    expect(complete).toBe(false);
    expect(items.map((i) => [i.id, i.done])).toEqual([
      ["name", false],
      ["source", false],
      ["identity", false],
      ["sink", false],
    ]);
  });

  it("keeps the name step pending while the self-person still carries the default sentinel", () => {
    const { items } = deriveSetupChecklist({
      person: person(SELF_DEFAULT_DISPLAY_NAME),
      sources: [SOURCE],
      identities: [IDENTITY],
      sinks: [SINK],
    });
    const name = items.find((i) => i.id === "name");
    expect(name?.done).toBe(false);
  });

  it("treats any other name — even whitespace-padded — as satisfying the name step", () => {
    const { items } = deriveSetupChecklist({
      person: person("  Vedanth  "),
      sources: [],
      identities: [],
      sinks: [],
    });
    expect(items.find((i) => i.id === "name")?.done).toBe(true);
  });

  it("reports complete only when all four inputs are populated", () => {
    const { complete } = deriveSetupChecklist({
      person: person("Vedanth"),
      sources: [SOURCE],
      identities: [IDENTITY],
      sinks: [SINK],
    });
    expect(complete).toBe(true);
  });

  it("returns items in a stable order so the UI doesn't reshuffle on refresh", () => {
    const { items } = deriveSetupChecklist({
      person: person("Vedanth"),
      sources: [SOURCE],
      identities: [],
      sinks: [SINK],
    });
    expect(items.map((i) => i.id)).toEqual(["name", "source", "identity", "sink"]);
  });
});
