import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Person, Source, SourceIdentity } from "@dayseam/ipc-types";
import { IdentityManagerDialog } from "../features/identities/IdentityManagerDialog";
import {
  registerInvokeHandler,
  resetTauriMocks,
  mockInvoke,
} from "./tauri-mock";

const SELF: Person = {
  id: "person-1",
  display_name: "Ada",
  is_self: true,
};

const SOURCE: Source = {
  id: "src-1",
  kind: "LocalGit",
  label: "Work",
  config: { LocalGit: { scan_roots: ["/Users/me/code"] } },
  secret_ref: null,
  created_at: "2026-04-17T12:00:00Z",
  last_sync_at: null,
  last_health: { ok: true, checked_at: null, last_error: null },
};

const IDENTITY: SourceIdentity = {
  id: "id-1",
  person_id: "person-1",
  source_id: null,
  kind: "GitEmail",
  external_actor_id: "ada@example.com",
};

describe("IdentityManagerDialog", () => {
  beforeEach(() => {
    resetTauriMocks();
    registerInvokeHandler("sources_list", async () => [SOURCE]);
    registerInvokeHandler("persons_get_self", async () => SELF);
    registerInvokeHandler(
      "identities_list_for",
      async () => [IDENTITY],
    );
  });
  afterEach(() => {
    resetTauriMocks();
  });

  it("loads the self-person and existing identity mappings", async () => {
    render(<IdentityManagerDialog open onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText(/add mapping for ada/i)).toBeInTheDocument(),
    );
    await waitFor(() =>
      expect(screen.getByText("ada@example.com")).toBeInTheDocument(),
    );
  });

  it("upserts a new identity when the form is submitted", async () => {
    registerInvokeHandler("identities_upsert", async (args) => args.identity as SourceIdentity);
    render(<IdentityManagerDialog open onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText(/add mapping for ada/i)).toBeInTheDocument(),
    );
    fireEvent.change(screen.getByRole("textbox", { name: /identity value/i }), {
      target: { value: "ada@work.example" },
    });
    fireEvent.click(screen.getByRole("button", { name: /^add$/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "identities_upsert",
        expect.objectContaining({
          identity: expect.objectContaining({
            external_actor_id: "ada@work.example",
            kind: "GitEmail",
            person_id: "person-1",
          }),
        }),
      ),
    );
  });

  it("deletes an identity when its Remove button is clicked", async () => {
    registerInvokeHandler("identities_delete", async () => null);
    render(<IdentityManagerDialog open onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText("ada@example.com")).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: /delete mapping ada@example\.com/i }),
    );
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "identities_delete",
        expect.objectContaining({ id: "id-1" }),
      ),
    );
  });

  it("degrades to an error banner when persons_get_self fails", async () => {
    registerInvokeHandler("persons_get_self", async () => {
      throw new Error("db locked");
    });
    render(<IdentityManagerDialog open onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText(/could not load the self-person/i)).toBeInTheDocument(),
    );
  });
});
