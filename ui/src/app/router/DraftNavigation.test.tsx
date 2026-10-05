import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { useState } from "react";
import { useAppRouter } from "./useAppRouter";
import { DraftNavigationProvider, useDraftGuard, useDraftNavigation } from "./DraftNavigation";

function NavigationProbe() {
  const { page, navigate } = useAppRouter();
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [discarded, setDiscarded] = useState(0);

  useDraftGuard({
    dirty,
    saving,
    onDiscard: () => {
      setDiscarded((value) => value + 1);
      setDirty(false);
    },
  });

  return (
    <>
      <output data-testid="page">{page}</output>
      <output data-testid="dirty">{String(dirty)}</output>
      <output data-testid="discarded">{discarded}</output>
      <button type="button" onClick={() => setDirty(true)}>Make dirty</button>
      <button type="button" onClick={() => setSaving(true)}>Start saving</button>
      <button type="button" onClick={() => { setSaving(false); setDirty(false); }}>Finish saving</button>
      <button type="button" onClick={() => navigate("settings")}>Go to Settings</button>
    </>
  );
}

function renderNavigationProbe() {
  render(
    <DraftNavigationProvider>
      <NavigationProbe />
    </DraftNavigationProvider>,
  );
}

function ReentrantNavigationProbe() {
  const navigation = useDraftNavigation();
  const { page, navigate } = useAppRouter();
  const [dirty, setDirty] = useState(true);
  const [discarded, setDiscarded] = useState(0);

  useDraftGuard({
    dirty,
    saving: false,
    onDiscard: () => {
      setDiscarded((value) => value + 1);
      setDirty(false);
    },
  });

  return (
    <>
      <output data-testid="reentrant-page">{page}</output>
      <output data-testid="reentrant-discarded">{discarded}</output>
      <button type="button" onClick={() => navigation.requestLeave(() => navigate("approvals"))}>Return through router</button>
    </>
  );
}

function FinishDraftProbe() {
  const navigation = useDraftNavigation();
  const { page, navigate } = useAppRouter();
  const [saving, setSaving] = useState(false);

  useDraftGuard({ dirty: false, saving });

  return (
    <>
      <output data-testid="finish-page">{page}</output>
      <output data-testid="finish-saving">{String(saving)}</output>
      <button type="button" onClick={() => setSaving(true)}>Start save</button>
      <button type="button" onClick={() => { navigation.finishDraft(); navigate("settings"); }}>Finish save and navigate</button>
    </>
  );
}

afterEach(() => cleanup());

describe("DraftNavigationProvider", () => {
  it("guards dirty navigation and completes the original navigation only after discard", () => {
    renderNavigationProbe();
    fireEvent.click(screen.getByRole("button", { name: "Make dirty" }));
    fireEvent.click(screen.getByRole("button", { name: "Go to Settings" }));

    expect(screen.getByRole("dialog", { name: "Discard changes?" })).toBeInTheDocument();
    expect(screen.getByTestId("page")).toHaveTextContent("overview");

    fireEvent.click(screen.getByRole("button", { name: "Stay editing" }));
    expect(screen.queryByRole("dialog", { name: "Discard changes?" })).not.toBeInTheDocument();
    expect(screen.getByTestId("page")).toHaveTextContent("overview");

    fireEvent.click(screen.getByRole("button", { name: "Go to Settings" }));
    fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect(screen.getByTestId("page")).toHaveTextContent("settings");
    expect(screen.getByTestId("dirty")).toHaveTextContent("false");
    expect(screen.getByTestId("discarded")).toHaveTextContent("1");
  });

  it("treats Escape on a dirty leave confirmation as staying in the draft", () => {
    renderNavigationProbe();
    fireEvent.click(screen.getByRole("button", { name: "Make dirty" }));
    fireEvent.click(screen.getByRole("button", { name: "Go to Settings" }));

    const dialog = screen.getByRole("dialog", { name: "Discard changes?" });
    fireEvent.keyDown(dialog, { key: "Escape" });

    expect(screen.queryByRole("dialog", { name: "Discard changes?" })).not.toBeInTheDocument();
    expect(screen.getByTestId("page")).toHaveTextContent("overview");
    expect(screen.getByTestId("dirty")).toHaveTextContent("true");
    expect(screen.getByTestId("discarded")).toHaveTextContent("0");
  });

  it("blocks navigation while saving and allows it after saving finishes", () => {
    renderNavigationProbe();
    fireEvent.click(screen.getByRole("button", { name: "Start saving" }));
    fireEvent.click(screen.getByRole("button", { name: "Go to Settings" }));

    const dialog = screen.getByRole("dialog", { name: /Saving/ });
    expect(screen.getByText("Wait for the current save to finish.")).toBeInTheDocument();
    expect(screen.getByTestId("page")).toHaveTextContent("overview");
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: /Saving/ })).not.toBeInTheDocument();
    expect(screen.getByTestId("page")).toHaveTextContent("overview");

    fireEvent.click(screen.getByRole("button", { name: "Finish saving" }));
    fireEvent.click(screen.getByRole("button", { name: "Go to Settings" }));
    expect(screen.getByTestId("page")).toHaveTextContent("settings");
  });

  it("does not re-open a leave confirmation when the discarded callback routes through the router", () => {
    render(
      <DraftNavigationProvider>
        <ReentrantNavigationProbe />
      </DraftNavigationProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Return through router" }));
    expect(screen.getByRole("dialog", { name: "Discard changes?" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect(screen.queryByRole("dialog", { name: "Discard changes?" })).not.toBeInTheDocument();
    expect(screen.getByTestId("reentrant-page")).toHaveTextContent("approvals");
    expect(screen.getByTestId("reentrant-discarded")).toHaveTextContent("1");
  });

  it("finishDraft clears a previous saving guard before immediate navigation", () => {
    render(
      <DraftNavigationProvider>
        <FinishDraftProbe />
      </DraftNavigationProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Start save" }));
    expect(screen.getByTestId("finish-saving")).toHaveTextContent("true");
    fireEvent.click(screen.getByRole("button", { name: "Finish save and navigate" }));

    expect(screen.getByTestId("finish-page")).toHaveTextContent("settings");
    expect(screen.queryByRole("dialog", { name: /Saving/ })).not.toBeInTheDocument();
  });
});
