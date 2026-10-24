import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "@/App";
import { MockHarnessControlClient } from "@/services/harness/mock-client";
import type { DesktopNavigationIntent } from "@/services/harness/client";

const pages = [
  ["Tools", "Tools"],
  ["Policies", "Policies"],
  ["Activity", "Activity"],
  ["Settings", "Settings"],
] as const;
const nav = (name: string) => screen.getByRole("button", { name: new RegExp(`${name}$`) });

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "", "/");
});

describe("M1 application shell", () => {
  it("navigates across all five primary pages", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
    for (const [navigationLabel, heading] of pages) {
      fireEvent.click(nav(navigationLabel));
      expect(await screen.findByRole("heading", { name: heading, level: 1 })).toBeInTheDocument();
    }
  });

  it("shows M2 desired and current effective tool state while preserving policies", async () => {
    render(<App />);
    fireEvent.click(nav("Tools"));
    expect(await screen.findByText("read_file")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Saved Enabled state for read_file" })).toHaveAttribute("aria-checked", "true");
    expect((await screen.findAllByText("Available")).length).toBeGreaterThan(0);
    fireEvent.click(nav("Policies"));
    expect((await screen.findAllByText("Allow workspace capabilities")).length).toBeGreaterThan(0);
    expect(screen.getAllByText("DENY").length).toBeGreaterThan(0);
  });

  it("renders the disconnected product state from the mock scenario", async () => {
    window.history.replaceState({}, "", "/?mock=disconnected");
    render(<App />);
    expect(await screen.findByText(/Runtime unavailable/)).toBeInTheDocument();
  });

  it("renders empty and unavailable observation states from the mock scenarios", async () => {
    window.history.replaceState({}, "", "/?mock=empty");
    render(<App />);
    fireEvent.click(nav("Tools"));
    expect((await screen.findAllByText("Runtime has not reported this tool.")).length).toBe(5);
    cleanup();
    window.history.replaceState({}, "", "/?mock=error");
    render(<App />);
    expect(await screen.findByText(/Runtime unavailable/)).toBeInTheDocument();
  });

  it("routes typed Host navigation intents to Overview and Approvals", async () => {
    let navigation: ((intent: DesktopNavigationIntent) => void) | undefined;
    vi.spyOn(MockHarnessControlClient.prototype, "desktopNavigationSubscribe").mockImplementation(async (listener) => {
      navigation = listener;
      return () => undefined;
    });
    vi.spyOn(MockHarnessControlClient.prototype, "desktopBackgroundNoticeSubscribe").mockResolvedValue(() => undefined);
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
    await waitFor(() => expect(navigation).toBeDefined());
    navigation?.({ page: "approvals", objectId: "approval_mock_01" });
    expect(await screen.findByRole("heading", { name: "Approvals", level: 1 })).toBeInTheDocument();
    navigation?.({ page: "overview" });
    expect(await screen.findByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
  });

  it("shows first-close education and Continue calls background command without Stop", async () => {
    let notice: (() => void) | undefined;
    vi.spyOn(MockHarnessControlClient.prototype, "desktopNavigationSubscribe").mockResolvedValue(() => undefined);
    vi.spyOn(MockHarnessControlClient.prototype, "desktopBackgroundNoticeSubscribe").mockImplementation(async (listener) => {
      notice = listener;
      return () => undefined;
    });
    const continueInBackground = vi.spyOn(MockHarnessControlClient.prototype, "desktopContinueInBackground");
    const harnessStop = vi.spyOn(MockHarnessControlClient.prototype, "harnessStop");
    render(<App />);
    await screen.findByRole("heading", { name: "Overview", level: 1 });
    await waitFor(() => expect(notice).toBeDefined());
    notice?.();
    expect(await screen.findByRole("heading", { name: "Keep ChatGPT-Harness available in the Tray?", level: 2 })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continue in background" }));
    await waitFor(() => expect(continueInBackground).toHaveBeenCalledOnce());
    expect(harnessStop).not.toHaveBeenCalled();
  });
});

describe("M2 configuration and lifecycle experience", () => {
  it("completes the first-run local runtime setup before showing the app shell", async () => {
    window.history.replaceState({}, "", "/?setup=first-run");
    render(<App />);

    expect(await screen.findByRole("heading", { name: "Workspace", level: 2 })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Start Harness" })).not.toBeInTheDocument();
    expect(screen.queryByText("Control-plane HTTP proxy (optional)")).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Security", level: 2 })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Overview" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Choose Folder" }));
    await screen.findByRole("heading", { name: "Connection" });
    fireEvent.change(screen.getByLabelText("Tunnel ID (required)"), { target: { value: "mock-tunnel" } });
    fireEvent.change(screen.getByLabelText("API Key (required)"), { target: { value: "mock-api-key" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply and continue" }));
    await screen.findByRole("heading", { name: "Readiness" });
    expect(screen.getByText(/Configuration is prepared/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Start Harness" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Start Harness" }));
    expect(await screen.findByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Settings$/ })).toBeInTheDocument();
  });
  it("keeps a configured API Key out of the Settings form", async () => {
    render(<App />);
    fireEvent.click(nav("Settings"));
    expect(await screen.findByText("Configured")).toBeInTheDocument();
    expect(screen.queryByPlaceholderText("Enter API Key")).not.toBeInTheDocument();
    expect(screen.queryByDisplayValue("tunnel-api-key")).not.toBeInTheDocument();
  });

  it("shows Config Mismatch in Settings and clears it after restart", async () => {
    render(<App />);
    fireEvent.click(nav("Settings"));
    await waitFor(() => expect(screen.getByRole("button", { name: "Configure" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Configure" }));
    const tunnelId = await screen.findByDisplayValue("mock-tunnel");
    fireEvent.change(tunnelId, { target: { value: "updated-tunnel" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    const restart = await screen.findByRole("button", { name: "Restart Harness" });
    expect(screen.getByText("Config Mismatch — Restart Required")).toBeInTheDocument();
    fireEvent.click(restart);
    await waitFor(() => {
      expect(screen.queryByText("Config Mismatch — Restart Required")).not.toBeInTheDocument();
    });
    expect(await screen.findByText("Harness restarted with the saved configuration.")).toBeInTheDocument();
  });

  it("validates and saves the control-plane-only proxy from Settings", async () => {
    render(<App />);
    fireEvent.click(nav("Settings"));
    await waitFor(() => expect(screen.getByRole("button", { name: "Configure" })).toBeEnabled());
    fireEvent.click(screen.getByRole("button", { name: "Configure" }));
    const proxy = await screen.findByLabelText("Control-plane HTTP proxy (optional)");
    fireEvent.change(proxy, { target: { value: "socks5://127.0.0.1:7897" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(await screen.findByText("Proxy URL must use http:// or https://.")).toBeInTheDocument();

    fireEvent.change(proxy, { target: { value: "http://127.0.0.1:7897" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(await screen.findByText("Config Mismatch — Restart Required")).toBeInTheDocument();
  });
});
