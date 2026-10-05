import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Dialog, Disclosure, Inspector, Row, Segmented, Switch } from "./primitives";

afterEach(cleanup);
it("keeps row inspection and direct controls separate", () => {
  const inspect = vi.fn(), change = vi.fn();
  render(<Row title="bash" onInspect={inspect} controls={<Switch checked={false} label="Enabled bash" onChange={change} />} />);
  fireEvent.click(screen.getByRole("switch"));
  expect(change).toHaveBeenCalledWith(true); expect(inspect).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "bash" })); expect(inspect).toHaveBeenCalledOnce();
});
it("provides roving keyboard selection without a Tab mutation", () => {
  const change = vi.fn();
  render(<Segmented label="Policy" value="ask" options={[{ value: "allow", label: "Allow" }, { value: "ask", label: "Ask" }, { value: "deny", label: "Deny" }]} onChange={change} />);
  const ask = screen.getByRole("radio", { name: "Ask" });
  expect(ask).toHaveAttribute("tabindex", "0");
  fireEvent.click(ask); expect(change).not.toHaveBeenCalled();
  fireEvent.keyDown(ask, { key: "Tab" }); expect(change).not.toHaveBeenCalled();
  fireEvent.keyDown(ask, { key: "ArrowRight" }); expect(change).toHaveBeenCalledWith("deny");
  expect(screen.getByRole("radio", { name: "Deny" })).toHaveFocus();
});
it("traps modal focus, delegates Esc and restores the trigger", () => {
  const close = vi.fn();
  const trigger = document.createElement("button"); document.body.append(trigger); trigger.focus();
  const { unmount } = render(<Dialog title="Confirm" onClose={close}><button>Last</button></Dialog>);
  const last = screen.getByRole("button", { name: "Last" }); last.focus(); fireEvent.keyDown(last, { key: "Tab" });
  expect(screen.getByRole("button", { name: "Close" })).toHaveFocus();
  fireEvent.keyDown(last, { key: "Escape" }); expect(close).toHaveBeenCalledOnce();
  unmount(); expect(trigger).toHaveFocus(); trigger.remove();
});
it("uses a nonmodal Inspector only when at least 600px of main remains", () => {
  Object.defineProperty(window, "innerWidth", { configurable: true, value: 1280 });
  const { rerender } = render(<Inspector title="Tool" onClose={() => undefined}>Details</Inspector>);
  expect(screen.getByRole("complementary")).not.toHaveAttribute("aria-modal");
  Object.defineProperty(window, "innerWidth", { configurable: true, value: 1243 }); fireEvent(window, new Event("resize"));
  expect(screen.getByRole("dialog")).toHaveAttribute("aria-modal", "true");
  Object.defineProperty(window, "innerWidth", { configurable: true, value: 980 }); fireEvent(window, new Event("resize"));
  rerender(<Inspector title="Tool" onClose={() => undefined}>Details</Inspector>);
  expect(screen.getByRole("dialog")).toHaveAttribute("aria-modal", "true");
});
it("restores focus to the latest inspected row when switching objects", () => {
  Object.defineProperty(window, "innerWidth", { configurable: true, value: 1280 });
  const first = document.createElement("button"), second = document.createElement("button");
  document.body.append(first, second); first.focus();
  const { rerender, unmount } = render(<Inspector title="First" onClose={() => undefined}>Details</Inspector>);
  second.focus(); rerender(<Inspector title="Second" onClose={() => undefined}>Details</Inspector>);
  unmount(); expect(second).toHaveFocus(); first.remove(); second.remove();
});
it("does not trap focus in evidence hidden by a closed disclosure", () => {
  render(<Dialog title="Confirm" onClose={() => undefined}><Disclosure title="Advanced"><Disclosure title="Nested"><button>Hidden</button></Disclosure></Disclosure></Dialog>);
  const summary = screen.getByText("Advanced"); summary.focus(); fireEvent.keyDown(summary, { key: "Tab" });
  expect(screen.getByRole("button", { name: "Close" })).toHaveFocus();
});
