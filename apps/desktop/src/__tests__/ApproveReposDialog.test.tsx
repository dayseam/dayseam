import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { LocalRepo, Source } from "@dayseam/ipc-types";
import { ApproveReposDialog } from "../features/sources/ApproveReposDialog";
import {
  registerInvokeHandler,
  resetTauriMocks,
  mockInvoke,
} from "./tauri-mock";

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

const REPO_PUBLIC: LocalRepo = {
  path: "/Users/me/code/public-app",
  label: "public-app",
  is_private: false,
  discovered_at: "2026-04-17T12:00:00Z",
};

const REPO_PRIVATE: LocalRepo = {
  path: "/Users/me/code/secret",
  label: "secret",
  is_private: true,
  discovered_at: "2026-04-17T12:00:00Z",
};

describe("ApproveReposDialog", () => {
  beforeEach(() => {
    resetTauriMocks();
  });
  afterEach(() => {
    resetTauriMocks();
  });

  it("lists discovered repos with their privacy state", async () => {
    registerInvokeHandler("local_repos_list", async () => [
      REPO_PUBLIC,
      REPO_PRIVATE,
    ]);
    render(<ApproveReposDialog source={SOURCE} onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText("public-app")).toBeInTheDocument(),
    );
    expect(screen.getByText("secret")).toBeInTheDocument();
    expect(screen.getByText(/1 marked private/i)).toBeInTheDocument();
  });

  it("flips is_private via `local_repos_set_private` when a checkbox toggles", async () => {
    registerInvokeHandler("local_repos_list", async () => [REPO_PUBLIC]);
    registerInvokeHandler("local_repos_set_private", async (args) => ({
      ...REPO_PUBLIC,
      is_private: args.isPrivate as boolean,
    }));
    render(<ApproveReposDialog source={SOURCE} onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText("public-app")).toBeInTheDocument(),
    );
    const checkbox = screen.getByRole("checkbox", { name: /private/i });
    fireEvent.click(checkbox);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "local_repos_set_private",
        expect.objectContaining({
          path: "/Users/me/code/public-app",
          isPrivate: true,
        }),
      ),
    );
  });
});
