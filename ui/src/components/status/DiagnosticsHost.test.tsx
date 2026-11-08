import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { i18n, initializeLanguage } from "@/i18n";
import { DiagnosticsHost, openConnectionDiagnostics } from "./DiagnosticsHost";

const data = vi.hoisted(() => ({ runDiagnostics: vi.fn() }));

vi.mock("@/app/providers/HarnessDataProvider", () => ({
  useHarnessData: () => ({
    runDiagnostics: data.runDiagnostics,
    lastDiagnostics: null,
    latestStartupAttempt: null,
  }),
}));

beforeEach(async () => {
  data.runDiagnostics.mockReset();
  await initializeLanguage("en");
});

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

describe("DiagnosticsHost", () => {
  it("renders a rejected diagnostics run as visible feedback and restores the action", async () => {
    data.runDiagnostics.mockRejectedValue({ code: "RUNTIME_UNAVAILABLE" });
    render(<DiagnosticsHost />);
    openConnectionDiagnostics();
    const run = await screen.findByRole("button", { name: "Run connection diagnostics" });
    fireEvent.click(run);
    expect(await screen.findByText("Connection diagnostics did not complete")).toBeInTheDocument();
    expect(screen.getByText("RUNTIME_UNAVAILABLE")).toBeInTheDocument();
    await waitFor(() => expect(run).toBeEnabled());
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });
});
