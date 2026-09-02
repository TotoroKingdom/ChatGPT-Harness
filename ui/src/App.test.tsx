import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import App from "@/App";

const pages = [
  ["Tools", "Tools"],
  ["Policies", "Policies"],
  ["Activity", "Activity"],
  ["Settings", "Settings"],
] as const;
const nav = (name: string) => screen.getByRole("button", { name: new RegExp(`${name}$`) });

afterEach(() => {
  cleanup();
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

  it("renders mock tools and policy decisions through the data provider", async () => {
    render(<App />);
    fireEvent.click(nav("Tools"));
    expect(await screen.findByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("Not exposed")).toBeInTheDocument();
    fireEvent.click(nav("Policies"));
    expect(await screen.findByText("Allow workspace reads")).toBeInTheDocument();
    expect(screen.getAllByText("DENY").length).toBeGreaterThan(0);
  });

  it("renders the disconnected product state from the mock scenario", async () => {
    window.history.replaceState({}, "", "/?mock=disconnected");
    render(<App />);
    expect(await screen.findByText("Runtime not connected")).toBeInTheDocument();
  });

  it("renders empty and error states from the mock scenarios", async () => {
    window.history.replaceState({}, "", "/?mock=empty");
    render(<App />);
    fireEvent.click(nav("Tools"));
    expect(await screen.findByText("No tools are exposed in this mock scenario.")).toBeInTheDocument();
    cleanup();
    window.history.replaceState({}, "", "/?mock=error");
    render(<App />);
    expect(await screen.findByText("Couldn’t load this view")).toBeInTheDocument();
  });
});

describe("M2 configuration and lifecycle experience", () => {
  it("walks through the four-step first-run wizard before showing the app shell", async () => {
    window.history.replaceState({}, "", "/?setup=first-run");
    render(<App />);

    expect(await screen.findByRole("heading", { name: "Connect your local workspace", level: 2 })).toBeInTheDocument();
    expect(screen.getByText("First run setup")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Workspace", level: 2 })).toBeInTheDocument();
    expect(screen.queryByText("Tunnel ID")).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Security", level: 2 })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Diagnostics", level: 2 })).not.toBeInTheDocument();
    expect(screen.queryByText("Save Changes")).not.toBeInTheDocument();
    expect(screen.queryByText("All changes saved")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Overview" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Choose Folder" }));
    expect(await screen.findByDisplayValue("D:\\example-project")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Continue/ }));
    expect(await screen.findByRole("heading", { name: "Connection", level: 2 })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Workspace", level: 2 })).not.toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("Enter tunnel ID"), { target: { value: "mock-tunnel" } });
    fireEvent.click(screen.getByRole("button", { name: "Choose File" }));
    expect(await screen.findByDisplayValue("D:\\example-project\\tunnel-client.exe")).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText("Enter API Key"), { target: { value: "test-api-key" } });
    fireEvent.click(screen.getByRole("button", { name: /Continue/ }));
    expect(await screen.findByRole("heading", { name: "Security", level: 2 })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Connection", level: 2 })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Continue/ }));
    expect(await screen.findByRole("heading", { name: "Diagnostics", level: 2 })).toBeInTheDocument();
    expect(screen.getAllByText("Available after Runtime integration").length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("button", { name: "Finish Setup" }));
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
    const tunnelId = await screen.findByDisplayValue("mock-tunnel");
    fireEvent.change(tunnelId, { target: { value: "updated-tunnel" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Changes" }));
    const restart = await screen.findByRole("button", { name: "Restart Harness" });
    expect(screen.getByText("Config Mismatch — Restart Required")).toBeInTheDocument();
    fireEvent.click(restart);
    await waitFor(() => {
      expect(screen.queryByText("Config Mismatch — Restart Required")).not.toBeInTheDocument();
    });
  });
});





