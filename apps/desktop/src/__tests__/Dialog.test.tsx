import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Dialog, DialogButton } from "../components/Dialog";

describe("Dialog primitive", () => {
  it("mounts nothing when open is false", () => {
    render(
      <Dialog open={false} onClose={() => {}} title="Title" testId="d">
        body
      </Dialog>,
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("renders the title, description, and body when open", () => {
    render(
      <Dialog
        open
        onClose={() => {}}
        title="Add source"
        description="Provide a scan root."
        testId="d"
      >
        <p>inner</p>
      </Dialog>,
    );
    expect(screen.getByRole("dialog", { name: /add source/i })).toBeInTheDocument();
    expect(screen.getByText(/provide a scan root/i)).toBeInTheDocument();
    expect(screen.getByText(/inner/i)).toBeInTheDocument();
  });

  it("closes on Escape", () => {
    const onClose = vi.fn();
    render(
      <Dialog open onClose={onClose} title="T" testId="d">
        body
      </Dialog>,
    );
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("closes on backdrop click, not on content click", () => {
    const onClose = vi.fn();
    render(
      <Dialog open onClose={onClose} title="T" testId="d">
        <button type="button">inside</button>
      </Dialog>,
    );
    // Click content first — must not close.
    fireEvent.mouseDown(screen.getByRole("button", { name: /inside/i }));
    expect(onClose).not.toHaveBeenCalled();

    // Click backdrop — must close.
    fireEvent.mouseDown(screen.getByTestId("d-backdrop"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("restores focus to the opener when it unmounts", () => {
    function Harness() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button
            type="button"
            data-testid="opener"
            onClick={() => setOpen(true)}
          >
            open
          </button>
          <Dialog open={open} onClose={() => setOpen(false)} title="T" testId="d">
            body
          </Dialog>
        </>
      );
    }
    render(<Harness />);
    const opener = screen.getByTestId("opener");
    opener.focus();
    fireEvent.click(opener);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(document.activeElement).toBe(opener);
  });
});

describe("DialogButton", () => {
  it("forwards clicks and respects the disabled prop", () => {
    const onClick = vi.fn();
    const { rerender } = render(
      <DialogButton kind="primary" onClick={onClick}>
        Go
      </DialogButton>,
    );
    fireEvent.click(screen.getByRole("button", { name: /go/i }));
    expect(onClick).toHaveBeenCalledTimes(1);

    rerender(
      <DialogButton kind="primary" onClick={onClick} disabled>
        Go
      </DialogButton>,
    );
    fireEvent.click(screen.getByRole("button", { name: /go/i }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
