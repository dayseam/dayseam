import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Source, SourceHealth } from "@dayseam/ipc-types";
import { SourcesSidebar } from "../features/sources/SourcesSidebar";
import {
  registerInvokeHandler,
  resetTauriMocks,
  mockInvoke,
} from "./tauri-mock";

const HEALTHY: SourceHealth = {
  ok: true,
  checked_at: "2026-04-17T12:00:00Z",
  last_error: null,
};

const SOURCE: Source = {
  id: "src-1",
  kind: "LocalGit",
  label: "Work repos",
  config: { LocalGit: { scan_roots: ["/Users/me/code", "/Users/me/work"] } },
  secret_ref: null,
  created_at: "2026-04-10T12:00:00Z",
  last_sync_at: null,
  last_health: HEALTHY,
};

describe("SourcesSidebar", () => {
  beforeEach(() => {
    resetTauriMocks();
  });
  afterEach(() => {
    resetTauriMocks();
  });

  it("renders the empty state when no sources are configured", async () => {
    registerInvokeHandler("sources_list", async () => []);
    render(<SourcesSidebar />);
    await waitFor(() =>
      expect(screen.getByText(/no sources connected/i)).toBeInTheDocument(),
    );
    expect(
      screen.getByRole("button", { name: /add local git source/i }),
    ).toBeInTheDocument();
  });

  it("renders a configured source with its root count and a healthy dot", async () => {
    registerInvokeHandler("sources_list", async () => [SOURCE]);
    render(<SourcesSidebar />);
    await waitFor(() =>
      expect(screen.getByText("Work repos")).toBeInTheDocument(),
    );
    // "· 2 roots" — scan_roots has length 2.
    expect(screen.getByText(/· 2 roots/i)).toBeInTheDocument();
    expect(screen.getByTestId("source-chip-src-1")).toHaveAttribute(
      "title",
      expect.stringMatching(/healthy/i),
    );
  });

  it("invokes `sources_healthcheck` when the rescan control is clicked", async () => {
    registerInvokeHandler("sources_list", async () => [SOURCE]);
    registerInvokeHandler("sources_healthcheck", async () => HEALTHY);
    render(<SourcesSidebar />);
    await waitFor(() =>
      expect(screen.getByText("Work repos")).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("button", { name: /rescan work repos/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "sources_healthcheck",
        expect.objectContaining({ id: "src-1" }),
      ),
    );
  });

  it("opens the add-source dialog when the add button is clicked", async () => {
    registerInvokeHandler("sources_list", async () => []);
    render(<SourcesSidebar />);
    await waitFor(() =>
      expect(screen.getByText(/no sources connected/i)).toBeInTheDocument(),
    );
    fireEvent.click(
      screen.getByRole("button", { name: /add local git source/i }),
    );
    expect(
      screen.getByRole("dialog", { name: /add local git source/i }),
    ).toBeInTheDocument();
  });
});
