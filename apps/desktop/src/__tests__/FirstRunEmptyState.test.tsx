import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { Person } from "@dayseam/ipc-types";
import App from "../App";
import {
  registerInvokeHandler,
  registerOnboardingComplete,
  resetTauriMocks,
} from "./tauri-mock";

// `empty_state_visibility_matches_setup_status`
// `checklist_item_completes_on_dialog_close`
//
// Plan §Task 7 calls these out as the two invariants the first-run UX
// must uphold. We exercise both at the `<App />` level so the gate in
// `App.tsx`, `useSetupChecklist`, and `SetupSidebar` are all covered
// by one render tree.

const DEFAULT_PERSON: Person = {
  id: "11111111-1111-1111-1111-111111111111",
  display_name: "Me",
  is_self: true,
};

const NAMED_PERSON: Person = {
  id: "11111111-1111-1111-1111-111111111111",
  display_name: "Vedanth",
  is_self: true,
};

describe("First-run empty state", () => {
  beforeEach(() => {
    resetTauriMocks();
    localStorage.clear();
    document.documentElement.classList.remove("dark");
    document.documentElement.removeAttribute("data-theme");
  });

  afterEach(() => {
    resetTauriMocks();
    localStorage.clear();
  });

  describe("empty_state_visibility_matches_setup_status", () => {
    it("shows the first-run screen and hides the main layout when any step is incomplete", async () => {
      registerOnboardingComplete();
      // Flip one step off to force incomplete.
      registerInvokeHandler("sources_list", async () => []);

      render(<App />);

      await screen.findByTestId("first-run-empty-state");
      // Main-layout landmarks must be absent while the gate is up.
      expect(
        screen.queryByRole("region", { name: /report actions/i }),
      ).toBeNull();
      expect(
        screen.queryByRole("region", { name: /report preview/i }),
      ).toBeNull();
    });

    it("shows the main layout and hides the empty state once every step is done", async () => {
      registerOnboardingComplete();

      render(<App />);

      await waitFor(() =>
        expect(
          screen.getByRole("region", { name: /connected sources/i }),
        ).toBeInTheDocument(),
      );
      expect(screen.queryByTestId("first-run-empty-state")).toBeNull();
    });
  });

  describe("checklist_item_completes_on_dialog_close", () => {
    it("flips the name step on dialog-save without refetching the person", async () => {
      registerOnboardingComplete();
      // Start with the sentinel name so the name step is the only one
      // that's incomplete — the rest of the app is wired up.
      registerInvokeHandler("persons_get_self", async () => DEFAULT_PERSON);
      // `persons_update_self` returns the new `Person`; the dialog
      // pipes that into `setPerson` so the checklist flips without a
      // second `persons_get_self` round-trip.
      let updateCalls = 0;
      registerInvokeHandler("persons_update_self", async () => {
        updateCalls += 1;
        return NAMED_PERSON;
      });
      let getSelfCalls = 0;
      registerInvokeHandler("persons_get_self", async () => {
        getSelfCalls += 1;
        return DEFAULT_PERSON;
      });

      render(<App />);

      // Wait for the first-run screen to settle (initial fetch done).
      await screen.findByTestId("first-run-empty-state");
      const nameItem = await screen.findByTestId("setup-item-name");
      expect(nameItem).toHaveAttribute("data-done", "false");
      const firstGetCalls = getSelfCalls;

      // Open the name dialog via its action button.
      fireEvent.click(screen.getByTestId("setup-action-name"));
      const dialog = await screen.findByTestId("pick-name-dialog");
      const input = dialog.querySelector<HTMLInputElement>("#pick-name-input");
      expect(input).not.toBeNull();
      fireEvent.change(input!, { target: { value: "Vedanth" } });
      fireEvent.click(screen.getByRole("button", { name: /^save$/i }));

      // The dialog closes and, once it has, the checklist flips to
      // complete — which in turn lets the gate swap from the first-
      // run empty state to the main layout. We wait on the gate swap
      // (stronger assertion than re-reading the row, since `data-done`
      // unmounts once the first-run screen goes away).
      await waitFor(() =>
        expect(screen.queryByTestId("pick-name-dialog")).toBeNull(),
      );
      await waitFor(() =>
        expect(screen.queryByTestId("first-run-empty-state")).toBeNull(),
      );
      expect(
        screen.getByRole("region", { name: /connected sources/i }),
      ).toBeInTheDocument();
      expect(updateCalls).toBe(1);
      // No extra `persons_get_self` was fired — the dialog handed the
      // updated row back and the hook swapped state locally.
      expect(getSelfCalls).toBe(firstGetCalls);
    });
  });
});
