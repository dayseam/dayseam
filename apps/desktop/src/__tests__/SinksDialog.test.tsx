import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Sink } from "@dayseam/ipc-types";
import { SinksDialog } from "../features/sinks/SinksDialog";
import {
  registerInvokeHandler,
  resetTauriMocks,
  mockInvoke,
} from "./tauri-mock";

const SINK: Sink = {
  id: "sink-1",
  kind: "MarkdownFile",
  label: "Daily notes",
  config: {
    MarkdownFile: {
      config_version: 1,
      dest_dirs: ["/Users/me/vault/daily"],
      frontmatter: true,
    },
  },
  created_at: "2026-04-17T12:00:00Z",
  last_write_at: null,
};

describe("SinksDialog", () => {
  beforeEach(() => {
    resetTauriMocks();
  });
  afterEach(() => {
    resetTauriMocks();
  });

  it("renders configured sinks with their summary line", async () => {
    registerInvokeHandler("sinks_list", async () => [SINK]);
    render(<SinksDialog open onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText("Daily notes")).toBeInTheDocument(),
    );
    expect(
      screen.getByText(/markdown · \/Users\/me\/vault\/daily · frontmatter/i),
    ).toBeInTheDocument();
  });

  it("validates destination count and adds a new markdown sink", async () => {
    registerInvokeHandler("sinks_list", async () => []);
    registerInvokeHandler("sinks_add", async () => SINK);
    render(<SinksDialog open onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText(/no sinks configured yet/i)).toBeInTheDocument(),
    );
    fireEvent.change(screen.getByRole("textbox", { name: /label/i }), {
      target: { value: "Daily notes" },
    });
    fireEvent.change(screen.getByRole("textbox", { name: /destination directories/i }), {
      target: { value: "/Users/me/vault/daily" },
    });
    fireEvent.click(screen.getByRole("button", { name: /add sink/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "sinks_add",
        expect.objectContaining({
          kind: "MarkdownFile",
          label: "Daily notes",
          config: expect.objectContaining({
            MarkdownFile: expect.objectContaining({
              dest_dirs: ["/Users/me/vault/daily"],
              frontmatter: true,
            }),
          }),
        }),
      ),
    );
  });

  it("disables submit when more than two destination directories are provided", async () => {
    registerInvokeHandler("sinks_list", async () => []);
    render(<SinksDialog open onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText(/no sinks configured yet/i)).toBeInTheDocument(),
    );
    fireEvent.change(screen.getByRole("textbox", { name: /label/i }), {
      target: { value: "Notes" },
    });
    fireEvent.change(
      screen.getByRole("textbox", { name: /destination directories/i }),
      { target: { value: "/a\n/b\n/c" } },
    );
    expect(screen.getByRole("button", { name: /add sink/i })).toBeDisabled();
    expect(
      screen.getByText(/at most two destination directories/i),
    ).toBeInTheDocument();
  });
});
