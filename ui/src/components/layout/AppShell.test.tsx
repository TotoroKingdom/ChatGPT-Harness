import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { i18n } from "@/i18n";
import { AppShell } from "./AppShell";
import { ControlReadView } from "@/components/status/ControlReadView";

vi.mock("@/app/providers/HarnessDataProvider", () => ({ useHarnessData: () => ({ client: { adapterKind: "real" }, components: { runtime: "ready" } }) }));
afterEach(cleanup);

it("switches shell copy without changing route identity or children", async () => {
  const navigate = vi.fn();
  const { container } = render(<AppShell page="overview" onNavigate={navigate}><input aria-label="identity fixture" defaultValue="request-123" /></AppShell>);
  const input = screen.getByDisplayValue("request-123");
  expect(screen.getByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
  await act(async () => { await i18n.changeLanguage("zh-CN"); });
  expect(screen.getByRole("heading", { name: "概览", level: 1 })).toBeInTheDocument();
  expect(container).toHaveTextContent("仅本地");
  expect(screen.getByDisplayValue("request-123")).toBe(input);
  expect(document.documentElement.lang).toBe("zh-CN");
  fireEvent.click(screen.getByRole("button", { name: /工具$/ }));
  expect(navigate).toHaveBeenCalledWith("tools");
});

it("localizes an existing control read error on live switch without a new request", async () => {
  render(<ControlReadView state={{ status: "error", error: { code: "RUNTIME_UNAVAILABLE" } }}>{() => null}</ControlReadView>);
  expect(screen.getByRole("alert")).toHaveTextContent("Runtime unavailable");
  await act(async () => { await i18n.changeLanguage("zh-CN"); });
  expect(screen.getByRole("alert")).not.toHaveTextContent("Runtime unavailable");
  expect(screen.getByRole("alert").textContent).toMatch(/[\u4e00-\u9fff]/);
});
