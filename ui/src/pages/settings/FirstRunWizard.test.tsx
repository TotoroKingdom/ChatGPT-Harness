import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { MockHarnessControlClient } from "@/services/harness/mock-client";
import type { HarnessControlClient } from "@/services/harness/client";
import { HarnessClientError } from "@/services/harness/errors";
import { HarnessDataProvider } from "@/app/providers/HarnessDataProvider";
import { DraftNavigationProvider } from "@/app/router/DraftNavigation";
import { FirstRunWizard } from "./FirstRunWizard";

const state = vi.hoisted(() => ({ client: undefined as unknown as MockHarnessControlClient }));
vi.mock("@/services/harness/factory", () => ({ createHarnessControlClient: () => state.client }));
beforeEach(async () => { state.client = new MockHarnessControlClient("ready"); await state.client.configReset(); state.client.setComponentStates({ runtime: "stopped", mcp: "stopped", tunnel: "stopped" }); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
function view(onDone = vi.fn()) {
  render(<HarnessDataProvider><DraftNavigationProvider><FirstRunWizard onDone={onDone} /></DraftNavigationProvider></HarnessDataProvider>);
  return onDone;
}
async function workspace() {
  fireEvent.click(await screen.findByRole("button", { name: "Choose Folder" }));
  await screen.findByRole("heading", { name: "Connection" });
  await waitFor(() => expect(screen.getByLabelText("Tunnel ID (required)")).toBeEnabled());
}
function connection() {
  fireEvent.change(screen.getByLabelText("Tunnel ID (required)"), { target: { value: "setup-tunnel" } });
  fireEvent.change(screen.getByLabelText("API Key (required)"), { target: { value: "test-only-credential" } });
  fireEvent.click(screen.getByRole("button", { name: "Apply and continue" }));
}
it("saves each scoped setup step and completes only after observed Harness readiness", async () => {
  const started = vi.spyOn(state.client, "harnessStart");
  const done = view(); await workspace();
  expect((await state.client.configGet())?.workspace.rootPath).toBe("D:\\example-project");
  expect(started).not.toHaveBeenCalled();
  connection(); await screen.findByRole("heading", { name: "Readiness" });
  expect(screen.getByText(/Configuration is prepared/)).toBeInTheDocument();
  expect(done).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  fireEvent.click(await screen.findByRole("button", { name: "Start Harness" }));
  await waitFor(() => expect(done).toHaveBeenCalledOnce(), { timeout: 5000 });
  expect(started).toHaveBeenCalledOnce();
});
it("a cancelled workspace picker writes nothing and stays at Workspace", async () => {
  vi.spyOn(state.client as HarnessControlClient, "chooseWorkspace").mockResolvedValue(null);
  const saved = vi.spyOn(state.client, "configSave"); view();
  fireEvent.click(await screen.findByRole("button", { name: "Choose Folder" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Choose Folder" })).toBeEnabled());
  expect(saved).not.toHaveBeenCalled();
  expect(screen.getByRole("heading", { name: "Workspace" })).toBeInTheDocument();
});
it("keeps non-sensitive Connection draft on partial failure, clears the key and requires re-entry", async () => {
  view(); await workspace();
  const saved = vi.spyOn(state.client, "configSave").mockImplementationOnce(async () => {
    vi.spyOn(state.client, "apiKeyStatus").mockResolvedValue(true);
    throw new HarnessClientError("error", "write failed", undefined, "CONFIG_WRITE_FAILED");
  });
  connection();
  await screen.findByText(/Connection save was not fully confirmed/);
  expect(screen.getByLabelText("Tunnel ID (required)")).toHaveValue("setup-tunnel");
  expect(screen.getByLabelText("API Key (required)")).toHaveValue("");
  fireEvent.click(screen.getByRole("button", { name: "Apply and continue" }));
  await screen.findByText("Enter an API Key for the connection.");
  expect(saved).toHaveBeenCalledOnce();
});
it("guards dirty Back and clears secret input on explicit discard", async () => {
  view(); await workspace();
  fireEvent.change(screen.getByLabelText("API Key (required)"), { target: { value: "test-only-credential" } });
  fireEvent.click(screen.getByRole("button", { name: "Back" }));
  expect(screen.getByRole("dialog", { name: "Discard changes?" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Stay editing" }));
  expect(screen.getByLabelText("API Key (required)")).toHaveValue("test-only-credential");
  fireEvent.click(screen.getByRole("button", { name: "Back" }));
  fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
  fireEvent.click(await screen.findByRole("button", { name: "Continue" }));
  expect(screen.getByLabelText("API Key (required)")).toHaveValue("");
});
it("presents read failures as Recovery and does not reset until confirmation", async () => {
  vi.spyOn(state.client, "configGet").mockRejectedValue(new HarnessClientError("error", "invalid", undefined, "CONFIG_INVALID"));
  const reset = vi.spyOn(state.client, "configReset"); view();
  await screen.findByText("The original configuration has not been overwritten.");
  fireEvent.click(screen.getByRole("button", { name: "Reset configuration" }));
  expect(screen.getByRole("dialog", { name: "Reset local configuration?" })).toBeInTheDocument();
  expect(reset).not.toHaveBeenCalled();
  expect(screen.getByText(/It does not stop an existing session/)).toBeInTheDocument();
});
it("does not claim the original file is safe after an unknown read failure", async () => {
  vi.spyOn(state.client, "configGet").mockRejectedValue(new HarnessClientError("error", "read unavailable", undefined, "CONFIG_READ_FAILED"));
  view();
  await screen.findByText("The current file state is unconfirmed. Inspect it before attempting a replacement.");
  expect(screen.queryByText("The original configuration has not been overwritten.")).not.toBeInTheDocument();
});
